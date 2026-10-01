import type { Entry, Event, Snapshot, Connection, DeletePreview, DeleteReport } from "./protocol";
export type { Entry, Event, Snapshot, Connection, Usage, Status, SessionEntry, DeletePreview, DeleteReport } from "./protocol";

export type Turn = { start: number; entries: Entry[] };

export function groupTranscript(entries: Entry[]): Turn[] {
  const turns: Turn[] = [];
  entries.forEach((entry, index) => {
    if (entry.role === "user" || turns.length === 0)
      turns.push({ start: index, entries: [] });
    turns[turns.length - 1].entries.push(entry);
  });
  return turns;
}

export type Action =
  | Event
  | { type: "queued"; pending: Pending }
  | { type: "submit_failed"; request_id: number; message: string }
  | { type: "authorization_cleared"; id: number }
  | { type: "close_dismissed" }
  | { type: "delete_dismissed" };

export type Pending = { id: number; line: string; local?: boolean };
export type ViewState = {
  connection: Connection;
  deleteRevision: number | null;
  snapshot: Snapshot | null;
  pending: Pending | null;
  live: string;
  liveTools: string[];
  liveUsage: number;
  authorization: { id: number; prompt: string } | null;
  notice: string | null;
  error: string | null;
  diagnostics: string[];
  startupError: string | null;
  closing: boolean;
  closeFailed: {
    report: string;
    unsaved_ids: string[];
    can_retry: boolean;
  } | null;
  deleteConfirmation: DeletePreview | null;
  deleteReport: DeleteReport | null;
};

export const initialState: ViewState = {
  connection: "connected",
  deleteRevision: null,
  snapshot: null,
  pending: null,
  live: "",
  liveTools: [],
  liveUsage: 0,
  authorization: null,
  notice: null,
  error: null,
  diagnostics: [],
  startupError: null,
  closing: false,
  closeFailed: null,
  deleteConfirmation: null,
  deleteReport: null,
};

export function applyEvent(state: ViewState, event: Action): ViewState {
  switch (event.type) {
    case "sync": {
      const run = event.state.running;
      return { ...state, snapshot: event.state.snapshot, pending: run ? { id: run.request_id, line: run.line, local: false } : null, live: run?.text ?? "", liveTools: run?.tools ?? [], liveUsage: run?.usage ?? 0, authorization: event.state.authorization, diagnostics: event.state.diagnostics, startupError: event.state.startup_error, closing: event.state.closing, notice: event.state.notice, error: event.state.error, deleteReport: event.state.delete_report, deleteConfirmation: null, deleteRevision: null };
    }
    case "connection":
      return { ...state, connection: event.status, authorization: event.status === "connected" ? state.authorization : null, deleteConfirmation: null };
    case "client_exited":
      return { ...state, connection: "exited", closing: false, pending: null, closeFailed: null, authorization: null };
    case "delete_confirmation":
      return { ...state, deleteConfirmation: event.preview, deleteRevision: event.revision };
    case "queued":
      return {
        ...state,
        pending: event.pending,
        live: "",
        liveTools: [],
        liveUsage: 0,
        authorization: null,
        notice: null,
        error: null,
      };
    case "submit_failed":
      return state.pending?.id === event.request_id && state.pending.local !== false
        ? {
            ...state,
            pending: null,
            live: "",
            liveTools: [],
            error: event.message,
          }
        : { ...state, error: event.message };
    case "authorization_cleared":
    case "authorization_resolved":
      // 后端收到回复后可能立即发出下一项授权；只清除已回复的那一项，否则工作线程会一直等待。
      return state.authorization?.id === event.id
        ? { ...state, authorization: null }
        : state;
    case "close_dismissed":
      return { ...state, closeFailed: null };
    case "delete_dismissed":
      return { ...state, deleteConfirmation: null };
    case "snapshot":
      if (event.snapshot.revision === undefined && event.request_id !== null && state.pending?.id !== event.request_id)
        return state;
      return {
        ...state,
        snapshot: event.snapshot,
        pending: event.snapshot.revision === undefined && event.request_id === null ? state.pending : null,
        live: event.snapshot.revision === undefined && event.request_id === null ? state.live : "",
        liveTools: event.snapshot.revision === undefined && event.request_id === null ? state.liveTools : [],
        liveUsage: event.snapshot.revision === undefined && event.request_id === null ? state.liveUsage : 0,
        authorization:
          state.authorization?.id === event.snapshot.authorization_id
            ? state.authorization
            : null,
        notice: event.notice,
        error: event.error,
        deleteConfirmation: event.delete_confirmation ?? null,
        deleteReport: event.delete_report ?? null,
      };
    case "started":
      return event.line === undefined ? state : { ...state, pending: { id: event.request_id, line: event.line, local: false }, live: "", liveTools: [], liveUsage: 0, notice: null, error: null, deleteConfirmation: null };
    case "delta":
      return state.pending?.id === event.request_id
        ? { ...state, live: state.live + event.text }
        : state;
    case "usage":
      return state.pending?.id === event.request_id && event.usage
        ? {
            ...state,
            liveUsage: state.liveUsage + event.usage.input + event.usage.output,
          }
        : state;
    case "tool_progress":
      return state.pending?.id === event.request_id
        ? {
            ...state,
            live:
              state.live && !state.live.endsWith("\n\n")
                ? state.live + "\n\n"
                : state.live,
            liveTools: [...state.liveTools, event.name],
          }
        : state;
    case "authorization":
      return {
        ...state,
        authorization: { id: event.id, prompt: event.prompt },
      };
    case "diagnostic":
      return {
        ...state,
        diagnostics: [...state.diagnostics, event.message].slice(-20),
      };
    case "startup_error":
      return { ...state, startupError: event.message, pending: null };
    case "closing":
      return { ...state, closing: true, authorization: null, deleteConfirmation: null };
    case "close_failed":
      return {
        ...state,
        closing: false,
        closeFailed: {
          report: event.report,
          unsaved_ids: event.unsaved_ids,
          can_retry: event.can_retry,
        },
      };
  }
}

export function shouldSubmit(
  event: Pick<KeyboardEvent, "key" | "shiftKey" | "isComposing" | "keyCode">,
): boolean {
  return (
    event.key === "Enter" &&
    !event.shiftKey &&
    !event.isComposing &&
    event.keyCode !== 229
  );
}

export function safeExternalHref(href: string | undefined): string | undefined {
  return href && /^https?:\/\//i.test(href) ? href : undefined;
}
