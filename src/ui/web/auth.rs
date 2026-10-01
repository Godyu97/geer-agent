use axum::http::{HeaderMap, StatusCode, uri::Authority};
use std::{collections::VecDeque, sync::Mutex};
use uuid::Uuid;

const COOKIE: &str = "geer_agent_auth";
pub(super) struct Auth {
    token: String,
    credentials: Mutex<VecDeque<String>>,
}

impl Auth {
    pub fn new(token: String) -> Self {
        Self {
            token,
            credentials: Mutex::new(VecDeque::new()),
        }
    }
    pub fn login(&self, token: &str) -> Option<String> {
        if token != self.token {
            return None;
        }
        let credential = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let mut credentials = self.credentials.lock().expect("浏览器凭证锁损坏");
        if credentials.len() >= 256 {
            credentials.pop_front();
        }
        credentials.push_back(credential.clone());
        Some(format!(
            "{COOKIE}={credential}; HttpOnly; SameSite=Strict; Path=/"
        ))
    }
    pub fn credential(&self, headers: &HeaderMap) -> Result<String, StatusCode> {
        let cookie = headers
            .get("cookie")
            .and_then(|value| value.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;
        let value = cookie
            .split(';')
            .filter_map(|part| part.trim().split_once('='))
            .find_map(|(key, value)| (key == COOKIE).then_some(value))
            .ok_or(StatusCode::UNAUTHORIZED)?;
        self.valid(value)
            .then(|| value.to_owned())
            .ok_or(StatusCode::UNAUTHORIZED)
    }
    pub fn valid(&self, credential: &str) -> bool {
        self.credentials
            .lock()
            .expect("浏览器凭证锁损坏")
            .iter()
            .any(|value| value == credential)
    }
    pub fn logout(&self, credential: &str) {
        self.credentials
            .lock()
            .expect("浏览器凭证锁损坏")
            .retain(|value| value != credential);
    }
    pub fn clear_cookie() -> &'static str {
        "geer_agent_auth=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0"
    }
}

pub(super) fn host(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get("host")?.to_str().ok()?;
    let authority = value.parse::<Authority>().ok()?;
    let name = authority.host();
    let valid_name = name
        .trim_matches(['[', ']'])
        .parse::<std::net::IpAddr>()
        .is_ok()
        || (!name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.')));
    (valid_name && !value.contains('@') && !value.contains('%')).then_some(value)
}

pub(super) fn same_origin(headers: &HeaderMap) -> Result<(), StatusCode> {
    let host = host(headers).ok_or(StatusCode::FORBIDDEN)?;
    let origin = headers
        .get("origin")
        .and_then(|value| value.to_str().ok())
        .ok_or(StatusCode::FORBIDDEN)?;
    (origin == format!("http://{host}") || origin == format!("https://{host}"))
        .then_some(())
        .ok_or(StatusCode::FORBIDDEN)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credentials_are_process_local_and_writes_require_origin() {
        let auth = Auth::new("secret".into());
        assert!(auth.login("wrong").is_none());
        let cookie = auth.login("secret").unwrap();
        assert!(cookie.contains("HttpOnly; SameSite=Strict; Path=/"));
        let mut headers = HeaderMap::new();
        headers.insert("cookie", cookie.parse().unwrap());
        let credential = auth.credential(&headers).unwrap();
        assert_eq!(
            Auth::new("secret".into()).credential(&headers),
            Err(StatusCode::UNAUTHORIZED)
        );
        headers.insert("host", "127.0.0.1:9928".parse().unwrap());
        assert!(same_origin(&headers).is_err());
        headers.insert("origin", "http://attacker.example".parse().unwrap());
        assert!(same_origin(&headers).is_err());
        headers.insert("origin", "http://127.0.0.1:9928".parse().unwrap());
        assert!(same_origin(&headers).is_ok());
        auth.logout(&credential);
        assert!(auth.credential(&headers).is_err());
    }

    #[test]
    fn host_rejects_csp_separators_and_accepts_local_ipv6() {
        let mut headers = HeaderMap::new();
        for value in [
            "example.com;script-src",
            "user@example.com",
            "example.com%3B",
        ] {
            headers.insert("host", value.parse().unwrap());
            assert!(host(&headers).is_none());
        }
        for value in ["localhost:9928", "192.168.1.2:9928", "[::1]:9928"] {
            headers.insert("host", value.parse().unwrap());
            assert_eq!(host(&headers), Some(value));
        }
    }
}
