//! HTTP 宿主只处理认证、资源与传输；Agent 仍独占共用工作线程。
mod auth;

use axum::{
    Json, Router,
    extract::{
        DefaultBodyLimit, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode, Uri, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use futures_util::{SinkExt, StreamExt};
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use std::{error::Error, io, sync::Arc, time::Duration};
use tokio::{
    net::TcpListener,
    sync::{Semaphore, mpsc, oneshot},
    time::{Instant, timeout},
};

use crate::{
    config::web::WebConfig,
    ui::app::{AppEvent, AppRuntime, EventSink, Host},
};
use auth::{Auth, same_origin};

static ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/ui/frontend/dist/web");
const CONNECTIONS: usize = 16;
const EVENTS: usize = 256;
const MESSAGE_LIMIT: usize = 512 * 1024;

#[derive(Clone)]
struct Service {
    app: Arc<AppRuntime>,
    auth: Arc<Auth>,
    connections: Arc<Semaphore>,
}

pub(super) fn run() -> Result<(), Box<dyn Error>> {
    let config = WebConfig::load()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let listener = TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, config.port))
            .await
            .map_err(|error| {
                io::Error::other(format!("Web UI 无法监听 0.0.0.0:{}：{error}", config.port))
            })?;
        let (finish, finished) = oneshot::channel();
        let finish = std::sync::Mutex::new(Some(finish));
        let (app, ready) = AppRuntime::start(Host::Web, None, move |result| {
            if let Some(finish) = finish.lock().expect("关闭结果锁损坏").take() {
                let _ = finish.send(result);
            }
        });
        let startup = tokio::task::spawn_blocking(move || ready.recv()).await?;
        startup
            .map_err(|_| io::Error::other("图形工作线程启动中断。"))?
            .map_err(io::Error::other)?;
        if config.generated {
            println!("Web UI 临时访问口令：{}", config.token);
        }
        println!(
            "Web UI 监听 0.0.0.0:{}，本机访问 http://127.0.0.1:{}",
            config.port, config.port
        );
        let service = Service {
            app: Arc::clone(&app),
            auth: Arc::new(Auth::new(config.token)),
            connections: Arc::new(Semaphore::new(CONNECTIONS)),
        };
        let (stopping, stopped) = oneshot::channel();
        let mut server = tokio::spawn(async move {
            axum::serve(listener, router(service))
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
        });
        let (server_done, server_error) = tokio::select! {
            result = &mut server => (true, result.map_err(io::Error::other)?.err()),
            result = shutdown_signal() => {
                app.request_close();
                let _ = stopping.send(());
                (false, result.err())
            }
        };
        // 网络错误也要解除授权并保存；信号关闭则等待正在运行的 Agent 自然收尾。
        app.request_close();
        let saved = finished
            .await
            .map_err(|_| io::Error::other("关闭结果未返回。"))?;
        if !server_done {
            // 慢速 HTTP 上传不能无限阻止进程退出；Agent 的保存已在上面等待完成。
            if timeout(Duration::from_secs(5), &mut server).await.is_err() {
                server.abort();
            }
        }
        saved.map_err(|message| io::Error::other(format!("Web UI 退出时保存失败：{message}")))?;
        if let Some(error) = server_error {
            return Err(error.into());
        }
        Ok(())
    })
}

async fn shutdown_signal() -> io::Result<()> {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! { result = tokio::signal::ctrl_c() => result, _ = term.recv() => Ok(()) }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}

fn router(service: Service) -> Router {
    Router::new()
        .route("/api/auth", get(authenticated))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/ws", get(upgrade))
        .route("/api/{*path}", get(|| async { StatusCode::NOT_FOUND }))
        .fallback(get(asset))
        .layer(DefaultBodyLimit::max(4096))
        .layer(middleware::from_fn(security_headers))
        .with_state(service)
}

async fn security_headers(
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let source = auth::host(&headers)
        .map(|host| format!("ws://{host} wss://{host}"))
        .unwrap_or_default();
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_SECURITY_POLICY, format!("default-src 'self'; connect-src 'self' {source}; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self'; frame-ancestors 'none'; base-uri 'none'").parse().expect("合法 Host 生成 CSP"));
    headers.insert(
        header::CACHE_CONTROL,
        "no-store".parse().expect("固定响应头"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        "nosniff".parse().expect("固定响应头"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        "no-referrer".parse().expect("固定响应头"),
    );
    response
}

async fn authenticated(
    State(service): State<Service>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    service.auth.credential(&headers)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct Login {
    token: String,
}
async fn login(
    State(service): State<Service>,
    headers: HeaderMap,
    Json(login): Json<Login>,
) -> Result<Response, StatusCode> {
    same_origin(&headers)?;
    let cookie = service
        .auth
        .login(&login.token)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    Ok(([(header::SET_COOKIE, cookie)], StatusCode::NO_CONTENT).into_response())
}
async fn logout(
    State(service): State<Service>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    same_origin(&headers)?;
    let credential = service.auth.credential(&headers)?;
    service.auth.logout(&credential);
    Ok((
        [(header::SET_COOKIE, Auth::clear_cookie())],
        StatusCode::NO_CONTENT,
    )
        .into_response())
}

async fn asset(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    let Some(file) = ASSETS.get_file(path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    };
    ([(header::CONTENT_TYPE, content_type)], file.contents()).into_response()
}

async fn upgrade(
    State(service): State<Service>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    same_origin(&headers)?;
    let credential = service.auth.credential(&headers)?;
    let permit = Arc::clone(&service.connections)
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    Ok(ws
        .max_message_size(MESSAGE_LIMIT)
        .max_frame_size(MESSAGE_LIMIT)
        .on_upgrade(move |socket| async move {
            let _permit = permit;
            connection(socket, service, credential).await;
        }))
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Command {
    Submit {
        request_id: i64,
        line: String,
        revision: u64,
    },
    Authorize {
        request_id: i64,
        id: u64,
        allowed: bool,
    },
}
#[derive(Serialize)]
struct Reply {
    r#type: &'static str,
    request_id: i64,
    error: Option<String>,
}

async fn connection(socket: WebSocket, service: Service, credential: String) {
    let (events, mut incoming) = mpsc::channel(EVENTS);
    let sink: EventSink = Arc::new(move |event| events.try_send(event).is_ok());
    let client = match service.app.connect(sink) {
        Ok(client) => client,
        Err(_) => return,
    };
    let (mut outgoing, mut messages) = socket.split();
    let mut heartbeat = tokio::time::interval(Duration::from_secs(15));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_response = Instant::now();
    loop {
        tokio::select! {
            event = incoming.recv() => {
                let Some(event) = event else { break; };
                if !service.auth.valid(&credential) { break; }
                let closing_service = matches!(event, AppEvent::Closing) && service.app.is_closing();
                let text = match serde_json::to_string(&event) { Ok(text) => text, Err(_) => break };
                if !send(&mut outgoing, Message::Text(text.into())).await { break; }
                // /exit 的 Closing 只属于页面；全局关闭立即断开传输并继续在工作线程保存。
                if matches!(event, AppEvent::ClientExited) || closing_service { break; }
            }
            message = messages.next() => {
                if !service.auth.valid(&credential) { break; }
                match message {
                    Some(Ok(Message::Text(text))) => {
                        last_response = Instant::now();
                        let command = match serde_json::from_str::<Command>(&text) { Ok(command) => command, Err(_) => break };
                        let (request_id, result) = match command {
                            Command::Submit { request_id, line, revision } => (request_id, service.app.submit(&client, line, Some(revision)).map(|_| ())),
                            Command::Authorize { request_id, id, allowed } => (request_id, service.app.authorize(id, allowed)),
                        };
                        let reply = Reply { r#type: "reply", request_id, error: result.err() };
                        let Ok(reply) = serde_json::to_string(&reply) else { break; };
                        if !send(&mut outgoing, Message::Text(reply.into())).await { break; }
                    }
                    Some(Ok(Message::Ping(data))) => { last_response = Instant::now(); if !send(&mut outgoing, Message::Pong(data)).await { break; } }
                    Some(Ok(Message::Pong(_))) => last_response = Instant::now(),
                    _ => break,
                }
            }
            _ = heartbeat.tick() => {
                if last_response.elapsed() >= Duration::from_secs(45) || !service.auth.valid(&credential) { break; }
                if !send(&mut outgoing, Message::Ping(Vec::new().into())).await { break; }
            }
        }
    }
    service.app.disconnect(&client);
    let _ = timeout(Duration::from_secs(2), outgoing.close()).await;
}

async fn send(
    sink: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: Message,
) -> bool {
    matches!(
        timeout(Duration::from_secs(5), sink.send(message)).await,
        Ok(Ok(()))
    )
}
