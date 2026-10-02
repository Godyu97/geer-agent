// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import App from "./App";
import { applyEvent, initialState } from "./model";
import type { Event, Snapshot } from "./protocol";
import type { HostAdapter } from "./host";
import { WebHost } from "./hosts/web";
import WebGate from "./WebGate";

afterEach(() => { cleanup(); vi.useRealTimers(); vi.unstubAllGlobals(); });
const snapshot: Snapshot = {
  revision: 2,
  status: { model: "test", session_id: "one", session_title: "one", workspace: "/server", context_tokens: 0, context_window_tokens: 100, turn_tokens: 0, total_tokens: 0, usage_complete: true, instructions_loaded: false, memory: { state: "ready", count: 0, error: null } },
  sessions: [], all_sessions: [], memories: [], transcript: [], unsaved_ids: [], authorization_id: null,
};
function sync(extra: Partial<Extract<Event, { type: "sync" }>["state"]> = {}): Event {
  return { type: "sync", state: { snapshot, running: null, authorization: null, diagnostics: [], startup_error: null, closing: false, notice: null, error: null, delete_report: null, ...extra } };
}
function fakeHost() {
  let notify: (event: Event) => void = () => {};
  const host: HostAdapter = { kind: "web", connect(callback) { notify = callback; return () => {}; }, submit: vi.fn().mockResolvedValue(undefined), authorize: vi.fn().mockResolvedValue(undefined), copy: vi.fn().mockRejectedValue(new Error("HTTP clipboard")), close: vi.fn().mockResolvedValue(undefined), forceClose: vi.fn().mockResolvedValue(undefined) };
  return { host, send(event: Event) { act(() => notify(event)); } };
}

it("restores global live state and keeps another browser's operation after a rejected local submit", () => {
  let state = applyEvent(initialState, sync({ running: { request_id: 7, line: "other device", text: "partial", tools: ["bash"], usage: 10 }, authorization: { id: 8, prompt: "confirm" } }));
  expect(state.pending?.line).toBe("other device");
  expect(state.live).toBe("partial");
  expect(state.liveTools).toEqual(["bash"]);
  expect(state.liveUsage).toBe(10);
  state = applyEvent(state, { type: "submit_failed", request_id: 7, message: "busy" });
  expect(state.pending?.id).toBe(7);
  state = applyEvent(state, { type: "authorization_resolved", id: 7 });
  expect(state.authorization?.id).toBe(8);
  state = applyEvent(state, { type: "authorization_resolved", id: 8 });
  expect(state.authorization).toBeNull();
  state = applyEvent(state, { type: "snapshot", request_id: 7, snapshot: { ...snapshot, revision: 3 }, error: null, notice: null });
  expect(state.pending).toBeNull();
  expect(state.live).toBe("");
});

it("keeps per-session drafts across remote switches and clears deleted drafts", () => {
  const adapter = fakeHost(); render(<App host={adapter.host} />); adapter.send(sync());
  const input = screen.getByLabelText("消息") as HTMLTextAreaElement;
  fireEvent.change(input, { target: { value: "draft one" } });
  adapter.send(sync({ snapshot: { ...snapshot, status: { ...snapshot.status, session_id: "two" } } }));
  expect(input.value).toBe("");
  fireEvent.change(input, { target: { value: "draft two" } });
  adapter.send(sync()); expect(input.value).toBe("draft one");
  adapter.send(sync({ snapshot: { ...snapshot, status: { ...snapshot.status, session_id: "two" } }, delete_report: { items: [{ id: "one", state: "deleted", error: null }], new_session_id: "two" } }));
  expect(input.value).toBe("draft two");
  adapter.send(sync()); expect(input.value).toBe("");
});

it("opens phone panels, uses server workspace paths, and offers manual code copying", async () => {
  const adapter = fakeHost(); render(<App host={adapter.host} />);
  adapter.send(sync({ snapshot: { ...snapshot, transcript: [{ role: "assistant", text: "```rust\nlet n = 1;\n```" }] } }));
  fireEvent.click(screen.getByRole("button", { name: "会话面板" }));
  expect(screen.getByRole("button", { name: "会话面板" }).getAttribute("aria-expanded")).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "更改" }));
  expect(screen.queryByRole("button", { name: "浏览" })).toBeNull();
  expect(screen.getByText("服务器目录路径")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "状态面板" }));
  expect(screen.getByRole("button", { name: "状态面板" }).getAttribute("aria-expanded")).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "复制代码" }));
  await waitFor(() => expect(screen.getByRole("dialog", { name: "手动复制" })).toBeTruthy());
  expect((screen.getByLabelText("待复制代码") as HTMLTextAreaElement).value).toContain("let n = 1;");
});

it("passes the snapshot revision through the shared host and disables disconnected submissions", async () => {
  const adapter = fakeHost(); render(<App host={adapter.host} />); adapter.send(sync());
  fireEvent.change(screen.getByLabelText("消息"), { target: { value: "hello" } });
  fireEvent.click(screen.getByRole("button", { name: "发送 ↗" }));
  await waitFor(() => expect(adapter.host.submit).toHaveBeenCalledWith({ requestId: 1, line: "hello", revision: 2 }));
  adapter.send(sync()); adapter.send({ type: "connection", status: "disconnected" });
  expect((screen.getByLabelText("消息") as HTMLTextAreaElement).disabled).toBe(true);
});

class FakeSocket {
  static OPEN = 1;
  static instances: FakeSocket[] = [];
  readyState = 1;
  sent: string[] = [];
  onmessage: ((event: { data: string }) => void) | null = null;
  onclose: (() => void) | null = null;
  constructor(readonly url: string) { FakeSocket.instances.push(this); }
  send(text: string) { this.sent.push(text); }
  close() { this.readyState = 3; this.onclose?.(); }
  emit(event: object) { this.onmessage?.({ data: JSON.stringify(event) }); }
}
it("reconnects with a fresh sync and never resends a command whose reply was lost", async () => {
  vi.useFakeTimers(); FakeSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeSocket); vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ status: 204 }));
  const host = new WebHost(); const received: Event[] = []; const stop = host.connect((event) => received.push(event));
  const first = FakeSocket.instances[0]; first.emit(sync());
  const reply = host.submit({ requestId: 1, line: "hello", revision: 2 }).catch((error: Error) => error.message);
  first.close(); expect(await reply).toContain("操作结果");
  await vi.advanceTimersByTimeAsync(1500);
  const second = FakeSocket.instances[1]; expect(second.sent).toEqual([]);
  second.emit(sync({ running: { request_id: 9, line: "hello", text: "continued", tools: [], usage: 0 } }));
  expect(received.at(-1)?.type).toBe("sync");
  stop();
});

it("authenticates without putting the token in a URL or browser storage", async () => {
  const adapter = fakeHost();
  const fetch = vi.fn().mockResolvedValueOnce({ ok: false }).mockResolvedValueOnce({ ok: true });
  vi.stubGlobal("fetch", fetch);
  render(<WebGate host={adapter.host} />);
  await waitFor(() => expect((screen.getByLabelText("访问口令") as HTMLInputElement).disabled).toBe(false));
  fireEvent.change(screen.getByLabelText("访问口令"), { target: { value: "private token" } });
  fireEvent.click(screen.getByRole("button", { name: "连接工作区" }));
  await waitFor(() => expect(fetch).toHaveBeenCalledWith("/api/login", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ token: "private token" }) }));
  expect(localStorage.length).toBe(0);
  await waitFor(() => expect(screen.queryByLabelText("访问口令")).toBeNull());
});
