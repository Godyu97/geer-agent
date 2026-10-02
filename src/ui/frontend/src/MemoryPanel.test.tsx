// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import App from "./App";
import { searchMemories } from "./MemoryPanel";
import { applyEvent, initialState } from "./model";
import type { HostAdapter } from "./host";
import type { Event, MemoryEntry, Snapshot } from "./protocol";

afterEach(cleanup);
function entry(id: number, content: string): MemoryEntry {
  return { id: `11111111-1111-4111-8111-${String(id).padStart(12, "0")}`, content, created_at_ms: id, updated_at_ms: id };
}
const entries = [entry(1, "中文 Rust\n完整第二行"), entry(2, "个人 preference")];
const snapshot: Snapshot = {
  revision: 5,
  status: { model: "test", session_id: "one", session_title: "one", workspace: "/server", context_tokens: 0, context_window_tokens: 100, turn_tokens: 0, total_tokens: 0, usage_complete: true, instructions_loaded: true, memory: { state: "ready", count: 2, error: null } },
  sessions: [], all_sessions: [], memories: entries, transcript: [], unsaved_ids: [],
};
function adapter() {
  let notify: (event: Event) => void = () => {};
  const host: HostAdapter = { kind: "web", connect(callback) { notify = callback; return () => {}; }, submit: vi.fn().mockResolvedValue(undefined), authorize: vi.fn(), copy: vi.fn(), close: vi.fn(), forceClose: vi.fn() };
  return { host, send(event: Event) { act(() => notify(event)); } };
}
function done(request: number | null, revision: number, memories: MemoryEntry[], notice: string | null = null, error: string | null = null): Event {
  return { type: "snapshot", request_id: request, snapshot: { ...snapshot, revision, memories, status: { ...snapshot.status, memory: { state: "ready", count: memories.length, error: null } } }, notice, error };
}

it("matches server keyword scoring, stable ties, Unicode content and the ten result bound", () => {
  const values = [entry(3, "中文 rust"), entry(1, "中文 only"), entry(2, "RUST only"), entry(4, "absent")];
  expect(searchMemories(values, "RUST 中文").map((value) => value.id)).toEqual([values[0].id, values[1].id, values[2].id]);
  expect(searchMemories(values, "missing")).toEqual([]);
  expect(searchMemories(Array.from({ length: 12 }, (_, index) => entry(index, "rust")), "rust")).toHaveLength(10);
  expect(searchMemories(values, " ")).toBe(values);
});

it("manages full memory text, preserves failed edit drafts and confirms global clear with its preview revision", async () => {
  const client = adapter(); render(<App host={client.host} />); client.send(done(null, 5, entries));
  const composer = screen.getByLabelText("消息") as HTMLTextAreaElement;
  fireEvent.change(composer, { target: { value: "未发送聊天草稿" } });
  fireEvent.click(screen.getByRole("button", { name: "记忆" }));
  const panel = screen.getByRole("dialog", { name: "记忆管理" });
  expect(within(panel).getByText(entries[0].id)).toBeTruthy();
  expect(within(panel).getByText(/完整第二行/)).toBeTruthy();
  fireEvent.change(within(panel).getByLabelText("搜索记忆"), { target: { value: "RUST" } });
  expect(within(panel).queryByText(entries[1].id)).toBeNull();
  fireEvent.change(within(panel).getByLabelText("搜索记忆"), { target: { value: "" } });
  fireEvent.click(within(panel).getByRole("button", { name: "新增记忆" }));
  fireEvent.change(within(panel).getByLabelText("新增记忆正文"), { target: { value: "新增事实\n第二段" } });
  fireEvent.click(within(panel).getByRole("button", { name: "保存记忆" }));
  await waitFor(() => expect(client.host.submit).toHaveBeenLastCalledWith({ requestId: 1, line: "/memory add 新增事实\n第二段", revision: 5 }));
  const added = [...entries, entry(3, "新增事实\n第二段")];
  client.send(done(1, 6, added, `已添加记忆：${added[2].id}`));
  expect(within(panel).queryByLabelText("新增记忆正文")).toBeNull();
  const first = within(panel).getByText(entries[0].id).closest("article")!;
  fireEvent.click(within(first).getByRole("button", { name: "编辑记忆" }));
  const editor = within(panel).getByLabelText("编辑记忆正文") as HTMLTextAreaElement;
  expect(editor.value).toBe(entries[0].content);
  fireEvent.change(editor, { target: { value: "修改后的多行\n事实" } });
  fireEvent.click(within(panel).getByRole("button", { name: "保存记忆" }));
  await waitFor(() => expect(client.host.submit).toHaveBeenLastCalledWith({ requestId: 2, line: `/memory edit ${entries[0].id} 修改后的多行\n事实`, revision: 6 }));
  client.send(done(2, 7, added, null, "记忆操作失败：数据库失败"));
  expect(editor.value).toBe("修改后的多行\n事实");
  expect(within(panel).getByRole("alert").textContent).toContain("数据库失败");
  fireEvent.click(within(panel).getByRole("button", { name: "取消编辑" }));
  fireEvent.click(within(first).getByRole("button", { name: "删除记忆" }));
  await waitFor(() => expect(client.host.submit).toHaveBeenLastCalledWith({ requestId: 3, line: `/memory delete ${entries[0].id}`, revision: 7 }));
  client.send(done(3, 8, added));
  client.send({ type: "memory_confirmation", revision: 8, preview: { action: { kind: "delete", id: entries[0].id }, entries: [entries[0]], count: 1 } });
  let confirm = screen.getByRole("dialog", { name: "记忆操作确认" });
  expect(document.activeElement).toBe(within(confirm).getByRole("button", { name: "取消" }));
  fireEvent.keyDown(confirm, { key: "Escape" });
  expect(screen.queryByRole("dialog", { name: "记忆操作确认" })).toBeNull();
  expect(client.host.submit).toHaveBeenCalledTimes(3);
  fireEvent.click(within(panel).getByRole("button", { name: "清空记忆" }));
  await waitFor(() => expect(client.host.submit).toHaveBeenLastCalledWith({ requestId: 4, line: "/memory clear", revision: 8 }));
  client.send(done(4, 9, added));
  client.send({ type: "memory_confirmation", revision: 9, preview: { action: { kind: "clear" }, entries: [], count: 3 } });
  confirm = screen.getByRole("dialog", { name: "记忆操作确认" });
  expect(within(confirm).getByText(/所有 workspace/)).toBeTruthy();
  fireEvent.click(within(confirm).getByRole("button", { name: "确认清空" }));
  await waitFor(() => expect(client.host.submit).toHaveBeenLastCalledWith({ requestId: 5, line: "/memory clear --yes", revision: 9 }));
  client.send(done(5, 10, [], "已清空 3 条全局长期记忆。会话与 Trace 保留。"));
  expect(within(panel).getByText("暂无长期记忆。")).toBeTruthy();
  expect(composer.value).toBe("未发送聊天草稿");
  fireEvent.click(within(panel).getByRole("button", { name: "关闭记忆面板" }));
  expect(composer.value).toBe("未发送聊天草稿");
});

it("distinguishes disabled and failed storage and prevents busy or disconnected writes without losing a draft", () => {
  const client = adapter(); render(<App host={client.host} />); client.send(done(null, 5, entries));
  fireEvent.click(screen.getByRole("button", { name: "记忆" }));
  const panel = screen.getByRole("dialog", { name: "记忆管理" });
  fireEvent.click(within(panel).getByRole("button", { name: "新增记忆" }));
  const draft = within(panel).getByLabelText("新增记忆正文") as HTMLTextAreaElement;
  fireEvent.change(draft, { target: { value: "保留草稿" } });
  client.send({ type: "started", request_id: 40, line: "other device" });
  expect((within(panel).getByRole("button", { name: "保存记忆" }) as HTMLButtonElement).disabled).toBe(true);
  expect(draft.value).toBe("保留草稿");
  client.send(done(40, 6, entries));
  client.send({ type: "connection", status: "disconnected" });
  expect(draft.disabled).toBe(true);
  client.send({ type: "connection", status: "connected" });
  client.send({ type: "snapshot", request_id: null, notice: null, error: null, snapshot: { ...snapshot, revision: 7, memories: [], status: { ...snapshot.status, memory: { state: "unavailable", count: null, error: "连接失败" } } } });
  expect(within(panel).getByRole("alert").textContent).toContain("连接失败");
  expect(within(panel).queryByText("暂无长期记忆。")).toBeNull();
  expect(draft.value).toBe("保留草稿");
});

it("drops expired or foreign previews after state changes, reconnects and remote operations", () => {
  let state = applyEvent(initialState, done(null, 5, entries));
  const preview = { action: { kind: "clear" as const }, entries: [], count: 2 };
  expect(applyEvent(state, { type: "memory_confirmation", preview, revision: 4 })).toBe(state);
  state = applyEvent(state, { type: "memory_confirmation", preview, revision: 5 });
  expect(state.memoryConfirmation).toEqual(preview);
  state = applyEvent(state, { type: "started", request_id: 3, line: "/memory add other" });
  expect(state.memoryConfirmation).toBeNull();
  state = applyEvent(state, done(3, 6, entries));
  state = applyEvent(state, { type: "memory_confirmation", preview, revision: 6 });
  state = applyEvent(state, { type: "connection", status: "disconnected" });
  expect(state.memoryConfirmation).toBeNull();
});
