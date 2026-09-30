import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from "react";
import type { KeyboardEvent, ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Channel, invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open } from "@tauri-apps/plugin-dialog";
import {
  applyEvent,
  groupTranscript,
  initialState,
  safeExternalHref,
  shouldSubmit,
} from "./model";
import type { Entry, Event, Pending } from "./model";
import "./style.css";

function plainText(node: ReactNode): string {
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(plainText).join("");
  if (node && typeof node === "object" && "props" in node) {
    return plainText((node.props as { children?: ReactNode }).children);
  }
  return "";
}

function Message({
  entry,
  onCopyError,
}: {
  entry: Entry;
  onCopyError: (message: string) => void;
}) {
  const label = {
    user: "李火旺🔥",
    assistant: "Geer",
    tool: "工具",
    system: "系统",
  }[entry.role];
  return (
    <article className={`message message-${entry.role}`}>
      <div className="message-label">{label}</div>
      <div className="message-body">
        {entry.role === "assistant" ? (
          <ReactMarkdown
            remarkPlugins={[remarkGfm]}
            components={{
              a: ({ children, href }) =>
                safeExternalHref(href) ? (
                  <a href={href} target="_blank" rel="noopener noreferrer">
                    {children}
                  </a>
                ) : (
                  <span>{children}</span>
                ),
              pre: ({ children }) => (
                <div className="code-wrap">
                  <button
                    className="copy"
                    onClick={() =>
                      void writeText(plainText(children)).catch((error) =>
                        onCopyError(String(error)),
                      )
                    }
                  >
                    复制代码
                  </button>
                  <pre>{children}</pre>
                </div>
              ),
            }}
          >
            {entry.text}
          </ReactMarkdown>
        ) : (
          <pre className="plain">{entry.text}</pre>
        )}
      </div>
    </article>
  );
}

function shortId(id: string): string {
  return id.slice(0, 8);
}

function ToolGroup({
  entries,
  pending = false,
}: {
  entries: Entry[];
  pending?: boolean;
}) {
  const [open, setOpen] = useState(false);
  return (
    <details
      className="tool-group"
      onToggle={(event) => setOpen(event.currentTarget.open)}
    >
      <summary>
        <span>工具调用 · {entries.length} 次</span>
        <span className="tool-group-state">
          {pending ? "进行中" : "查看详情"}
        </span>
      </summary>
      {open && (
        <div className="tool-group-content">
          {entries.map((entry, index) => (
            <pre className="tool-detail plain" key={index}>
              {entry.text}
            </pre>
          ))}
        </div>
      )}
    </details>
  );
}

function ConversationTurn({
  entries,
  onCopyError,
}: {
  entries: Entry[];
  onCopyError: (message: string) => void;
}) {
  const tools = entries.filter((entry) => entry.role === "tool");
  const firstTool = entries.findIndex((entry) => entry.role === "tool");
  return (
    <section className="conversation-turn" aria-label="对话轮次">
      {entries.map((entry, index) =>
        entry.role === "tool" ? (
          index === firstTool ? (
            <ToolGroup key="tools" entries={tools} />
          ) : null
        ) : (
          <Message key={index} entry={entry} onCopyError={onCopyError} />
        ),
      )}
    </section>
  );
}

export default function App() {
  const [state, dispatch] = useReducer(applyEvent, initialState);
  const [input, setInput] = useState("");
  const [visibleCount, setVisibleCount] = useState(120);
  const [localError, setLocalError] = useState<string | null>(null);
  const [workspaceEditing, setWorkspaceEditing] = useState(false);
  const [workspaceInput, setWorkspaceInput] = useState("");
  const [sessionScope, setSessionScope] = useState<"current" | "all">(
    "current",
  );
  const [managingSessions, setManagingSessions] = useState(false);
  const [selectedSessions, setSelectedSessions] = useState<Set<string>>(new Set());
  const nextRequest = useRef(1);
  const current = useRef(state);
  current.current = state;
  const stream = useRef<HTMLDivElement>(null);
  const followBottom = useRef(true);
  const lastSession = useRef<string | null>(null);
  const lastWorkspace = useRef<string | null>(null);
  const prependHeight = useRef<number | null>(null);

  useEffect(() => {
    const channel = new Channel<Event>();
    channel.onmessage = (event) => dispatch(event);
    void invoke("gui_connect", { onEvent: channel }).catch((error) => {
      dispatch({ type: "startup_error", message: String(error) });
    });
  }, []);

  const sessionId = state.snapshot?.status.session_id ?? null;
  const workspace = state.snapshot?.status.workspace ?? null;
  const displayedSessions = useMemo(
    () => sessionScope === "all"
      ? (state.snapshot?.all_sessions ?? [])
      : (state.snapshot?.sessions ?? []),
    [sessionScope, state.snapshot],
  );
  useEffect(() => {
    setSelectedSessions(new Set());
  }, [sessionScope, workspace, managingSessions]);
  useEffect(() => {
    setSelectedSessions((selected) => new Set(
      [...selected].filter((id) => displayedSessions.some((entry) => entry.id === id)),
    ));
  }, [displayedSessions]);
  useEffect(() => {
    if (state.deleteReport?.new_session_id === sessionId) setInput("");
    else setSelectedSessions(new Set());
  }, [sessionId]);
  useEffect(() => {
    if (sessionId !== lastSession.current) {
      followBottom.current = true;
      lastSession.current = sessionId;
      setVisibleCount(120);
    }
  }, [sessionId]);
  useEffect(() => {
    if (workspace && workspace !== lastWorkspace.current) {
      lastWorkspace.current = workspace;
      setWorkspaceInput(workspace);
      setWorkspaceEditing(false);
      setLocalError(null);
    }
  }, [workspace]);
  useEffect(() => {
    if (
      workspaceEditing &&
      !state.pending &&
      state.notice?.startsWith("Workspace 未改变")
    ) {
      setWorkspaceInput(workspace ?? "");
      setWorkspaceEditing(false);
      setLocalError(null);
    }
  }, [state.notice, state.pending, workspace, workspaceEditing]);
  useLayoutEffect(() => {
    if (prependHeight.current !== null && stream.current) {
      stream.current.scrollTop +=
        stream.current.scrollHeight - prependHeight.current;
      prependHeight.current = null;
    }
  }, [visibleCount]);
  useEffect(() => {
    if (followBottom.current && stream.current)
      stream.current.scrollTop = stream.current.scrollHeight;
  }, [state.snapshot?.transcript, state.live, state.liveTools, sessionId]);

  async function submit(line: string, clearComposer = true) {
    if (
      !line.trim() ||
      !current.current.snapshot ||
      current.current.pending ||
      current.current.closing ||
      current.current.startupError ||
      current.current.authorization
    )
      return;
    const pending: Pending = { id: nextRequest.current++, line: line.trim() };
    current.current = {
      ...current.current,
      pending,
      live: "",
      liveTools: [],
      liveUsage: 0,
    };
    dispatch({ type: "queued", pending });
    if (/^\/(?:open|resume)\s/.test(pending.line)) {
      setSelectedSessions(new Set());
    }
    setLocalError(null);
    try {
      await invoke("gui_submit", { requestId: pending.id, line: pending.line });
      if (clearComposer) setInput("");
    } catch (error) {
      current.current = { ...current.current, pending: null };
      dispatch({
        type: "submit_failed",
        request_id: pending.id,
        message: String(error),
      });
      setLocalError(String(error));
    }
  }

  async function authorize(allowed: boolean) {
    const request = current.current.authorization;
    if (!request) return;
    try {
      await invoke("gui_authorize", { id: request.id, allowed });
    } catch (error) {
      setLocalError(String(error));
    }
    dispatch({ type: "authorization_cleared" });
  }

  async function browseWorkspace() {
    setLocalError(null);
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        defaultPath: workspaceInput || status?.workspace,
      });
      if (typeof selected === "string") setWorkspaceInput(selected);
    } catch (error) {
      setLocalError(String(error));
    }
  }

  async function retryClose() {
    dispatch({ type: "closing" });
    try {
      await getCurrentWindow().close();
    } catch (error) {
      setLocalError(String(error));
    }
  }

  const transcript = state.snapshot?.transcript ?? [];
  const turns = useMemo(() => groupTranscript(transcript), [transcript]);
  const visible = turns.slice(-visibleCount);
  const status = state.snapshot?.status;
  const operationBusy =
    !state.snapshot ||
    !!state.pending ||
    state.closing ||
    !!state.startupError ||
    !!state.authorization;
  const disabled = operationBusy || !!state.deleteConfirmation;
  const contextPercent = status
    ? Math.min(
        100,
        Math.round(
          (status.context_tokens / Math.max(1, status.context_window_tokens)) *
            100,
        ),
      )
    : 0;
  const isChat = state.pending && !state.pending.line.startsWith("/");
  const retryDeleteIds = state.deleteReport?.items
    .filter((item) => item.state === "failed" || item.state === "cleanup_pending")
    .map((item) => item.id) ?? [];
  function toggleSession(id: string) {
    setSelectedSessions((selected) => {
      const next = new Set(selected);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">
            g<span>·</span>
          </div>
          <div>
            <strong>geer-agent</strong>
            <small>Desktop workspace</small>
          </div>
        </div>
        <div className="workspace-card">
          <div className="workspace-heading">
            <span>WORKSPACE</span>
            {!workspaceEditing && (
              <button
                disabled={disabled}
                onClick={() => {
                  setWorkspaceInput(status?.workspace ?? "");
                  setLocalError(null);
                  setWorkspaceEditing(true);
                }}
              >
                更改
              </button>
            )}
          </div>
          {workspaceEditing ? (
            <div className="workspace-editor">
              <input
                aria-label="Workspace 路径"
                value={workspaceInput}
                disabled={disabled}
                onChange={(event) => setWorkspaceInput(event.target.value)}
              />
              <div className="workspace-actions">
                <button disabled={disabled} onClick={() => void browseWorkspace()}>
                  浏览
                </button>
                <button
                  className="workspace-confirm"
                  disabled={disabled || !workspaceInput.trim()}
                  onClick={() =>
                    void submit(`/workspace ${workspaceInput.trim()}`, false)
                  }
                >
                  切换并新建会话
                </button>
              </div>
              {(localError || state.error) && (
                <small className="workspace-error">
                  {localError || state.error}
                </small>
              )}
              <button
                className="workspace-cancel"
                disabled={disabled}
                onClick={() => {
                  setWorkspaceInput(status?.workspace ?? "");
                  setLocalError(null);
                  setWorkspaceEditing(false);
                }}
              >
                取消
              </button>
            </div>
          ) : (
            <strong className="workspace-path" title={status?.workspace}>
              {status?.workspace ?? "正在读取…"}
            </strong>
          )}
        </div>
        <button
          className="new-session"
          disabled={disabled}
          onClick={() => void submit("/new")}
        >
          ＋ 新建会话
        </button>
        <div className="section-title session-title">
          <span>会话 {displayedSessions.length}</span>
          <span className="scope-toggle" role="group" aria-label="会话范围">
            <button
              disabled={disabled}
              className={sessionScope === "current" ? "active" : ""}
              onClick={() => setSessionScope("current")}
            >
              当前
            </button>
            <button
              disabled={disabled}
              className={sessionScope === "all" ? "active" : ""}
              onClick={() => setSessionScope("all")}
            >
              全部
            </button>
          </span>
        </div>
        <div className="session-management">
          <button
            disabled={disabled}
            onClick={() => setManagingSessions(!managingSessions)}
          >
            {managingSessions ? "完成管理" : "管理会话"}
          </button>
          {managingSessions && <>
            <label className="select-all-sessions">
              <input
                type="checkbox"
                aria-label="全选当前列表"
                disabled={disabled || displayedSessions.length === 0}
                checked={displayedSessions.length > 0 && selectedSessions.size === displayedSessions.length}
                ref={(node) => {
                  if (node) node.indeterminate = selectedSessions.size > 0
                    && selectedSessions.size < displayedSessions.length;
                }}
                onChange={() => setSelectedSessions(
                  selectedSessions.size === displayedSessions.length
                    ? new Set()
                    : new Set(displayedSessions.map((entry) => entry.id)),
                )}
              />
              全选
            </label>
            <span>已选 {selectedSessions.size}</span>
            <button
              className="danger-text"
              disabled={disabled || selectedSessions.size === 0}
              onClick={() => {
                const ids = displayedSessions
                  .filter((entry) => selectedSessions.has(entry.id))
                  .map((entry) => entry.id);
                void submit(`/delete ${ids.join(" ")}`, false);
              }}
            >
              删除所选
            </button>
          </>}
        </div>
        {retryDeleteIds.length > 0 && (
          <button
            className="retry-delete"
            disabled={disabled}
            onClick={() => void submit(`/delete ${retryDeleteIds.join(" ")}`, false)}
          >
            重试删除 / 清理（{retryDeleteIds.length}）
          </button>
        )}
        <div className="session-list">
          {displayedSessions.map((session) => {
            const details = <>
              <span className="session-detail">
                <strong>{session.title}</strong>
                <small>
                  {shortId(session.id)} · {new Date(session.updated_at_ms).toLocaleString()} ·{" "}
                  {session.status}
                </small>
                {sessionScope === "all" && (
                  <small title={session.workspace}>{session.workspace}</small>
                )}
              </span>
              {session.uncertain_tools && <span title="工具状态未确认">!</span>}
            </>;
            const className = `session-item ${session.active ? "active" : ""} ${selectedSessions.has(session.id) ? "selected" : ""}`;
            return managingSessions ? (
              <label key={session.id} className={className} title={session.id}>
                <input
                  type="checkbox"
                  aria-label={`选择 ${session.title} ${session.id}`}
                  checked={selectedSessions.has(session.id)}
                  disabled={disabled}
                  onChange={() => toggleSession(session.id)}
                />
                {details}
              </label>
            ) : (
              <button
                key={session.id}
                disabled={disabled}
                className={className}
                title={session.id}
                onClick={() => void submit(`/open ${session.id}`)}
              >
                <span className="session-dot" />{details}
              </button>
            );
          })}
        </div>
        <div className="sidebar-footer">
          <span className="connection-dot" />
          {state.startupError
            ? "启动失败"
            : state.snapshot
              ? "本地运行"
              : "正在连接"}
        </div>
      </aside>

      <main className="main-panel">
        <header className="topbar">
          <div>
            <strong title={status?.session_id}>{status?.session_title ?? "对话"}</strong>
            <span className="session-id" title={status?.session_id}>
              {status ? shortId(status.session_id) : "初始化中"}
            </span>
          </div>
          <div className="top-actions">
            <button disabled={disabled} onClick={() => void submit("/compact")}>
              压缩
            </button>
            <button disabled={disabled} onClick={() => void submit("/save")}>
              保存
            </button>
            <button disabled={disabled} onClick={() => void submit("/help")}>
              帮助
            </button>
          </div>
        </header>
        {state.startupError ? (
          <div className="startup-error">
            <h2>无法启动 Geer</h2>
            <p>{state.startupError}</p>
            <p>请检查程序选用的 .env 和模型配置，然后重新启动。</p>
          </div>
        ) : (
          <>
            <div
              className="feed"
              ref={stream}
              onScroll={(event) => {
                const node = event.currentTarget;
                followBottom.current =
                  node.scrollHeight - node.scrollTop - node.clientHeight < 64;
              }}
            >
              {transcript.length === 0 && !state.pending && (
                <div className="empty">
                  <div className="empty-symbol">✦</div>
                  <h1>从这里开始</h1>
                  <p>向 Geer 提问，或输入 /help 查看已有命令。</p>
                </div>
              )}
              {turns.length > visibleCount && (
                <button
                  className="older"
                  onClick={() => {
                    prependHeight.current =
                      stream.current?.scrollHeight ?? null;
                    followBottom.current = false;
                    setVisibleCount((count) => count + 120);
                  }}
                >
                  显示更早记录 · 还有 {turns.length - visibleCount} 轮
                </button>
              )}
              {visible.map((turn) => (
                <ConversationTurn
                  key={`${status?.session_id}-${turn.start}`}
                  entries={turn.entries}
                  onCopyError={setLocalError}
                />
              ))}
              {isChat && (
                <>
                  <Message
                    entry={{ role: "user", text: state.pending!.line }}
                    onCopyError={setLocalError}
                  />
                  {state.liveTools.length > 0 && (
                    <ToolGroup
                      key={`${sessionId}-pending-${state.pending!.id}`}
                      entries={state.liveTools.map((name) => ({
                        role: "tool",
                        text: `调用 ${name}`,
                      }))}
                      pending
                    />
                  )}
                  {state.live && (
                    <Message
                      entry={{ role: "assistant", text: state.live }}
                      onCopyError={setLocalError}
                    />
                  )}
                </>
              )}
              {state.pending && (
                <div className="working">
                  <span className="pulse" />
                  {state.closing ? "等待当前操作完成并保存…" : "Geer 正在工作…"}
                  {state.liveUsage > 0
                    ? ` · 已报告 ${state.liveUsage} tokens`
                    : state.live
                      ? ` · 临时估算约 ${Math.max(1, Math.ceil(state.live.length / 4))} tokens`
                      : ""}
                </div>
              )}
              {state.notice && (
                <div className="notice">
                  <pre>{state.notice}</pre>
                </div>
              )}
              {!workspaceEditing && (localError || state.error) && (
                <div className="error-banner">{localError || state.error}</div>
              )}
              {state.diagnostics.length > 0 && (
                <details className="diagnostics">
                  <summary>运行诊断 ({state.diagnostics.length})</summary>
                  {state.diagnostics.map((message, index) => (
                    <pre key={index}>{message}</pre>
                  ))}
                </details>
              )}
            </div>
            <div className="composer">
              <div className="composer-inner">
                <label className="composer-identity" htmlFor="message-input">
                  李火旺🔥
                </label>
                <textarea
                  id="message-input"
                  aria-label="消息"
                  placeholder="发送消息给 Geer…"
                  value={input}
                  disabled={disabled}
                  rows={3}
                  onChange={(event) => setInput(event.target.value)}
                  onKeyDown={(event: KeyboardEvent<HTMLTextAreaElement>) => {
                    if (shouldSubmit(event.nativeEvent)) {
                      event.preventDefault();
                      void submit(input);
                    }
                  }}
                />
                <div className="composer-bottom">
                  <span>Enter 发送 · Shift+Enter 换行</span>
                  <button
                    disabled={disabled || !input.trim()}
                    onClick={() => void submit(input)}
                  >
                    发送 ↗
                  </button>
                </div>
              </div>
            </div>
          </>
        )}
      </main>

      <aside className="info-panel">
        <div className="section-title">运行状态</div>
        <div className="info-card">
          <small>模型</small>
          <strong>{status?.model ?? "—"}</strong>
        </div>
        <div className="info-card">
          <small>上下文估算</small>
          <strong>
            {status
              ? `${status.context_tokens.toLocaleString()} / ${status.context_window_tokens.toLocaleString()}`
              : "—"}
          </strong>
          <div className="meter">
            <span style={{ width: `${contextPercent}%` }} />
          </div>
          <small>{contextPercent}%</small>
        </div>
        <div className="info-card">
          <small>Token 用量</small>
          <div className="metric">
            <span>本轮</span>
            <strong>{status?.turn_tokens.toLocaleString() ?? "—"}</strong>
          </div>
          <div className="metric">
            <span>累计</span>
            <strong>{status?.total_tokens.toLocaleString() ?? "—"}</strong>
          </div>
          {status && !status.usage_complete && (
            <small className="warning">部分用量不可用</small>
          )}
        </div>
        <div className="info-card">
          <small>待保存会话</small>
          <strong>{state.snapshot?.unsaved_ids.length ?? 0}</strong>
          {state.snapshot?.unsaved_ids.map((id) => (
            <small key={id}>{shortId(id)}</small>
          ))}
        </div>
      </aside>

      {state.authorization && (
        <div className="modal-backdrop">
          <div
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label="工具授权"
          >
            <div className="modal-icon">!</div>
            <h2>允许工具操作？</h2>
            <p>请确认以下操作范围。允许后，本会话中同类工具将不再重复询问。</p>
            <pre>{state.authorization.prompt}</pre>
            <div className="modal-actions">
              <button
                className="secondary"
                autoFocus
                onClick={() => void authorize(false)}
              >
                拒绝
              </button>
              <button className="primary" onClick={() => void authorize(true)}>
                本会话允许
              </button>
            </div>
          </div>
        </div>
      )}
      {state.deleteConfirmation && (
        <div className="modal-backdrop">
          <div
            className="modal delete-modal"
            role="dialog"
            aria-modal="true"
            aria-label="删除会话确认"
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                dispatch({ type: "delete_dismissed" });
              }
              if (event.key === "Tab") {
                event.preventDefault();
                const buttons = Array.from(
                  event.currentTarget.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"),
                );
                const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
                buttons[(index + (event.shiftKey ? buttons.length - 1 : 1)) % buttons.length]?.focus();
              }
            }}
          >
            <h2>删除 {state.deleteConfirmation.targets.length} 个会话？</h2>
            <p>会话和消息将被永久删除，无法恢复。保留 Trace 日志。</p>
            {state.deleteConfirmation.targets.some((target) => target.active) &&
              <p className="delete-active-warning">包含当前会话：删除成功后将在原 workspace 新建空会话。</p>}
            <ul className="delete-targets">
              {state.deleteConfirmation.targets.map((target) => <li key={target.id}>
                <strong>{target.title}{target.active ? "（当前）" : ""}</strong>
                <code>{target.id}</code>
              </li>)}
            </ul>
            <div className="modal-actions">
              <button
                className="secondary"
                autoFocus
                onClick={() => dispatch({ type: "delete_dismissed" })}
              >
                取消
              </button>
              <button
                className="danger"
                disabled={operationBusy}
                onClick={() => {
                  const ids = current.current.deleteConfirmation?.targets.map((target) => target.id);
                  if (!ids) return;
                  dispatch({ type: "delete_dismissed" });
                  void submit(`/delete --yes ${ids.join(" ")}`, false);
                }}
              >
                确认删除
              </button>
            </div>
          </div>
        </div>
      )}
      {state.closeFailed && (
        <div className="modal-backdrop">
          <div
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label="保存失败"
          >
            <div className="modal-icon">!</div>
            <h2>部分会话未保存</h2>
            <p>
              {state.closeFailed.can_retry
                ? "以下会话仍在内存中。可以重试保存，返回继续使用，或明确退出。"
                : "以下会话仅保存在内存中，当前无法写入数据库。请返回继续使用或明确退出。"}
            </p>
            <pre>{state.closeFailed.report}</pre>
            <div className="modal-actions">
              <button
                className="secondary"
                onClick={() => dispatch({ type: "close_dismissed" })}
              >
                返回
              </button>
              <button
                className="secondary"
                onClick={() => void invoke("gui_force_close")}
              >
                仍然退出
              </button>
              {state.closeFailed.can_retry && (
                <button className="primary" onClick={() => void retryClose()}>
                  重试保存
                </button>
              )}
            </div>
          </div>
        </div>
      )}
      {state.closing && (
        <div className="closing-overlay">正在等待当前操作完成并保存会话…</div>
      )}
    </div>
  );
}
