export type Entry = {
  role: "user" | "assistant" | "tool" | "system";
  text: string;
};

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
  instructions_loaded: boolean;
  memory: MemoryStatus;
};
export type MemoryStatus = { state: "ready" | "disabled" | "unavailable"; count: number | null; error: string | null };
export type MemoryEntry = { id: string; content: string; created_at_ms: number; updated_at_ms: number };
export type MemoryAction = { kind: "delete"; id: string } | { kind: "clear" };
export type MemoryPreview = { action: MemoryAction; entries: MemoryEntry[]; count: number };
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
  revision?: number;
  status: Status;
  sessions: SessionEntry[];
  all_sessions: SessionEntry[];
  memories: MemoryEntry[];
  transcript: Entry[];
  unsaved_ids: string[];
  authorization_id?: number | null;
};
export type Running = {
  request_id: number;
  line: string;
  text: string;
  tools: string[];
  usage: number;
};
export type ServerView = {
  snapshot: Snapshot | null;
  running: Running | null;
  authorization: { id: number; prompt: string } | null;
  diagnostics: string[];
  startup_error: string | null;
  closing: boolean;
  notice: string | null;
  error: string | null;
  delete_report: DeleteReport | null;
};
export type Event =
  | { type: "sync"; state: ServerView }
  | { type: "connection"; status: Connection }
  | { type: "client_exited" }
  | { type: "authorization_resolved"; id: number }
  | { type: "delete_confirmation"; preview: DeletePreview; revision: number }
  | { type: "memory_confirmation"; preview: MemoryPreview; revision: number }
  | {
      type: "snapshot";
      request_id: number | null;
      snapshot: Snapshot;
      notice: string | null;
      error: string | null;
      delete_confirmation?: DeletePreview | null;
      delete_report?: DeleteReport | null;
    }
  | { type: "started"; request_id: number; line?: string; operation_id?: string; client_id?: string }
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

export type Connection = "connected" | "connecting" | "disconnected" | "unauthenticated" | "exited";
