export type Entry = {
  role: "user" | "assistant" | "tool" | "system";
  text: string;
};
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

export type Usage = { input: number; output: number };
export type Status = {
  model: string;
  session_id: string;
  session_title: string;
  workspace: string;
  context_tokens: number;
  context_window_tokens: number;
  turn_tokens: number;
  total_tokens: number;
  usage_complete: boolean;
};
export type SessionEntry = {
  id: string;
  title: string;
  updated_at_ms: number;
  model: string;
  status: string;
  active: boolean;
  uncertain_tools: boolean;
  workspace: string;
};
export type DeletePreview = {
  targets: { id: string; title: string; active: boolean }[];
};
export type DeleteReport = {
  items: {
    id: string;
    state: "deleted" | "absent" | "failed" | "cleanup_pending";
    error: string | null;
  }[];
  new_session_id: string | null;
};
export type Snapshot = {
  status: Status;
  sessions: SessionEntry[];
  all_sessions: SessionEntry[];
  transcript: Entry[];
  unsaved_ids: string[];
  authorization_id?: number | null;
};
export type Event =
  | {
      type: "snapshot";
      request_id: number | null;
      snapshot: Snapshot;
      notice: string | null;
      error: string | null;
      delete_confirmation?: DeletePreview | null;
      delete_report?: DeleteReport | null;
    }
  | { type: "started"; request_id: number }
  | { type: "delta"; request_id: number; text: string }
  | { type: "tool_progress"; request_id: number; name: string }
  | { type: "usage"; request_id: number; usage: Usage | null }
  | { type: "authorization"; id: number; prompt: string }
  | { type: "diagnostic"; message: string }
  | { type: "startup_error"; message: string }
  | { type: "closing" }
  | {
      type: "close_failed";
      report: string;
      unsaved_ids: string[];
      can_retry: boolean;
    };

export type Action =
  | Event
  | { type: "queued"; pending: Pending }
  | { type: "submit_failed"; request_id: number; message: string }
  | { type: "authorization_cleared"; id: number }
  | { type: "close_dismissed" }
  | { type: "delete_dismissed" };

export type Pending = { id: number; line: string };
export type ViewState = {
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
      return state.pending?.id === event.request_id
        ? {
            ...state,
            pending: null,
            live: "",
            liveTools: [],
            error: event.message,
          }
        : state;
    case "authorization_cleared":
      // 后端收到回复后可能立即发出下一项授权；只清除已回复的那一项，否则工作线程会一直等待。
      return state.authorization?.id === event.id
        ? { ...state, authorization: null }
        : state;
    case "close_dismissed":
      return { ...state, closeFailed: null };
    case "delete_dismissed":
      return { ...state, deleteConfirmation: null };
    case "snapshot":
      if (event.request_id !== null && state.pending?.id !== event.request_id)
        return state;
      return {
        ...state,
        snapshot: event.snapshot,
        pending: event.request_id === null ? state.pending : null,
        live: event.request_id === null ? state.live : "",
        liveTools: event.request_id === null ? state.liveTools : [],
        liveUsage: event.request_id === null ? state.liveUsage : 0,
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
      return state;
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
