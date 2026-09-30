// @vitest-environment jsdom
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import App from "./App";
import type { Entry, Event, Snapshot } from "./model";

const mock = vi.hoisted(() => ({
  channels: [] as Array<{ onmessage: (event: Event) => void }>,
  invoke: vi.fn(),
  close: vi.fn(),
  dialogOpen: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage: (event: Event) => void = () => {};
    constructor() {
      mock.channels.push(this);
    }
  },
  invoke: mock.invoke,
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ close: mock.close }),
}));
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({ writeText: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: mock.dialogOpen }));

const snapshot: Snapshot = {
  status: {
    model: "test-model",
    session_id: "session-one",
    session_title: "新会话",
    workspace: "/tmp/workspace-one",
    context_tokens: 2,
    context_window_tokens: 100,
    turn_tokens: 0,
    total_tokens: 0,
    usage_complete: true,
  },
  sessions: [],
  all_sessions: [],
  transcript: [],
  unsaved_ids: [],
};

function send(event: Event) {
  act(() => {
    mock.channels.at(-1)!.onmessage(event);
  });
}

beforeEach(() => {
  mock.channels.length = 0;
  mock.invoke.mockReset().mockResolvedValue(undefined);
  mock.close.mockReset().mockResolvedValue(undefined);
  mock.dialogOpen.mockReset().mockResolvedValue(null);
});
afterEach(cleanup);

function showHistory(transcript: Entry[], sessionId = "session-one") {
  send({
    type: "snapshot",
    request_id: null,
    notice: null,
    error: null,
    snapshot: {
      ...snapshot,
      status: { ...snapshot.status, session_id: sessionId },
      transcript,
    },
  });
}

it("keeps streamed Markdown visible when the completed history arrives", async () => {
  render(<App />);
  showHistory([]);
  const input = screen.getByRole("textbox", { name: "消息" });
  expect(input.getAttribute("placeholder")).toBe("发送消息给 Geer…");
  fireEvent.change(input, { target: { value: "检查代码" } });
  fireEvent.click(screen.getByRole("button", { name: /发送/ }));
  await waitFor(() =>
    expect(mock.invoke).toHaveBeenCalledWith("gui_submit", {
      requestId: 1,
      line: "检查代码",
    }),
  );
  send({ type: "delta", request_id: 1, text: "**修复完成**" });
  send({ type: "tool_progress", request_id: 1, name: "read" });
  send({ type: "tool_progress", request_id: 1, name: "rg" });
  expect(screen.getByText("修复完成").tagName).toBe("STRONG");
  const group = screen.getByText("工具调用 · 2 次").closest("details")!;
  expect(group.open).toBe(false);
  expect(screen.queryByText("调用 read")).toBeNull();
  expect(screen.queryByText(/\[调用工具/)).toBeNull();
  expect(screen.getByText("Geer")).toBeTruthy();
  expect(screen.getAllByText("李火旺🔥")).toHaveLength(2);
  send({
    type: "snapshot",
    request_id: 1,
    notice: null,
    error: null,
    snapshot: {
      ...snapshot,
      transcript: [
        { role: "user", text: "检查代码" },
        { role: "tool", text: "read\n文件内容" },
        { role: "tool", text: "rg\n匹配结果" },
        { role: "assistant", text: "**修复完成**" },
      ],
    },
  });
  expect(screen.getAllByText("修复完成")).toHaveLength(1);
  expect(screen.getByText("修复完成").closest("details")).toBeNull();
  expect(screen.getAllByText("工具调用 · 2 次")).toHaveLength(1);
  expect(screen.queryByText("进行中")).toBeNull();
  expect(screen.queryByText("Geer 正在工作…")).toBeNull();
});

it("folds all tools in one turn, keeps prose outside, and resets on session switch", async () => {
  render(<App />);
  const history: Entry[] = [
    { role: "user", text: "第一轮" },
    { role: "assistant", text: "先检查文件" },
    { role: "tool", text: "read\n第一项结果" },
    { role: "assistant", text: "再检查引用" },
    { role: "tool", text: "rg\n第二项结果" },
    { role: "assistant", text: "第一轮完成" },
    { role: "user", text: "第二轮" },
    { role: "tool", text: "ls\n第三项结果" },
    { role: "assistant", text: "第二轮完成" },
  ];
  showHistory(history);
  const first = screen.getByText("工具调用 · 2 次").closest("details")!;
  const second = screen.getByText("工具调用 · 1 次").closest("details")!;
  expect(first.open).toBe(false);
  expect(second.open).toBe(false);
  for (const prose of ["先检查文件", "再检查引用", "第一轮完成", "第二轮完成"])
    expect(screen.getByText(prose).closest("details")).toBeNull();
  fireEvent.click(within(first).getByText("工具调用 · 2 次"));
  await waitFor(() =>
    expect(within(first).getByText(/第一项结果/)).toBeTruthy(),
  );
  expect(within(first).getByText(/第二项结果/)).toBeTruthy();
  expect(within(first).queryByText(/第三项结果/)).toBeNull();
  showHistory([...history, { role: "user", text: "第三轮" }]);
  expect(screen.getByText("工具调用 · 2 次").closest("details")).toBe(first);
  expect(first.open).toBe(true);
  fireEvent.click(within(first).getByText("工具调用 · 2 次"));
  await waitFor(() => expect(screen.queryByText(/第一项结果/)).toBeNull());
  showHistory(history, "session-two");
  expect(screen.getByText("工具调用 · 2 次").closest("details")!.open).toBe(
    false,
  );
  showHistory([
    { role: "user", text: "纯对话" },
    { role: "assistant", text: "没有工具" },
  ]);
  expect(screen.queryByText(/工具调用/)).toBeNull();
});

it("loads older history by whole turn without splitting a large tool group", async () => {
  render(<App />);
  const history: Entry[] = [
    { role: "user", text: "更早的提问" },
    { role: "assistant", text: "更早的回答" },
    ...Array.from({ length: 119 }, (_, index) => ({
      role: "user" as const,
      text: `提问-${index}`,
    })),
    { role: "user", text: "工具很多的提问" },
    ...Array.from({ length: 130 }, (_, index) => ({
      role: "tool" as const,
      text: `read\n结果-${index}`,
    })),
    { role: "assistant", text: "全部完成" },
  ];
  showHistory(history);
  expect(screen.queryByText("更早的提问")).toBeNull();
  expect(screen.getByText("工具很多的提问")).toBeTruthy();
  const group = screen.getByText("工具调用 · 130 次").closest("details")!;
  fireEvent.click(within(group).getByText("工具调用 · 130 次"));
  await waitFor(() =>
    expect(within(group).getAllByText(/read/)).toHaveLength(130),
  );
  fireEvent.click(screen.getByRole("button", { name: /显示更早记录/ }));
  expect(screen.getByText("更早的回答")).toBeTruthy();
  expect(screen.getByText("工具调用 · 130 次").closest("details")).toBe(group);
  expect(group.open).toBe(true);
});

it("keeps Chinese IME Enter in the composer and submits after composition", async () => {
  render(<App />);
  send({
    type: "snapshot",
    request_id: null,
    snapshot,
    notice: null,
    error: null,
  });
  const input = screen.getByRole("textbox", { name: "消息" });
  fireEvent.change(input, { target: { value: "你好" } });
  fireEvent.keyDown(input, { key: "Enter", isComposing: true, keyCode: 229 });
  expect(
    mock.invoke.mock.calls.filter(([name]) => name === "gui_submit"),
  ).toHaveLength(0);
  fireEvent.keyDown(input, { key: "Enter", keyCode: 13 });
  await waitFor(() =>
    expect(mock.invoke).toHaveBeenCalledWith("gui_submit", {
      requestId: 1,
      line: "你好",
    }),
  );
});

it("uses explicit denial and reveals older messages on demand", async () => {
  render(<App />);
  const history = Array.from({ length: 121 }, (_, index) => ({
    role: "user" as const,
    text: `message-${index}`,
  }));
  send({
    type: "snapshot",
    request_id: null,
    snapshot: { ...snapshot, transcript: history },
    notice: null,
    error: null,
  });
  expect(screen.queryByText("message-0")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: /显示更早记录/ }));
  expect(screen.getByText("message-0")).toBeTruthy();
  send({ type: "authorization", id: 7, prompt: "write /tmp/a" });
  fireEvent.click(screen.getByRole("button", { name: "拒绝" }));
  await waitFor(() =>
    expect(mock.invoke).toHaveBeenCalledWith("gui_authorize", {
      id: 7,
      allowed: false,
    }),
  );
});

it("shows unsaved sessions and retries window close only after user selection", async () => {
  render(<App />);
  send({
    type: "snapshot",
    request_id: null,
    snapshot,
    notice: null,
    error: null,
  });
  send({
    type: "close_failed",
    report: "session-one [待补写]",
    unsaved_ids: ["session-one"],
    can_retry: true,
  });
  expect(screen.getByRole("dialog", { name: "保存失败" })).toBeTruthy();
  expect(mock.close).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "返回" }));
  expect(screen.queryByRole("dialog", { name: "保存失败" })).toBeNull();
  send({
    type: "close_failed",
    report: "session-one [待补写]",
    unsaved_ids: ["session-one"],
    can_retry: true,
  });
  fireEvent.click(screen.getByRole("button", { name: "重试保存" }));
  await waitFor(() => expect(mock.close).toHaveBeenCalledOnce());
});

it("allows sending again after reconnect cancels an outstanding authorization", async () => {
  render(<App />);
  showHistory([]);
  const input = screen.getByRole("textbox", { name: "消息" });
  fireEvent.change(input, { target: { value: "先检查" } });
  fireEvent.click(screen.getByRole("button", { name: /发送/ }));
  send({ type: "authorization", id: 7, prompt: "write /tmp/a" });
  send({
    type: "snapshot",
    request_id: 1,
    snapshot: { ...snapshot, authorization_id: 7 },
    notice: null,
    error: null,
  });
  expect(screen.getByRole("button", { name: /发送/ }).hasAttribute("disabled")).toBe(true);
  expect(screen.getByRole("button", { name: "本会话允许" })).toBeTruthy();
  send({
    type: "snapshot",
    request_id: null,
    snapshot: { ...snapshot, authorization_id: null },
    notice: null,
    error: null,
  });
  expect(screen.queryByRole("button", { name: "本会话允许" })).toBeNull();
  fireEvent.change(input, { target: { value: "重连后继续" } });
  fireEvent.click(screen.getByRole("button", { name: /发送/ }));
  await waitFor(() => expect(mock.invoke).toHaveBeenCalledWith("gui_submit", {
    requestId: 2,
    line: "重连后继续",
  }));
});

it("edits workspace without losing the chat draft and switches session scope", async () => {
  render(<App />);
  const currentSession = {
    id: "11111111-current",
    title: "测试会话",
    updated_at_ms: 1,
    model: "test-model",
    status: "已保存",
    active: true,
    uncertain_tools: false,
    workspace: "/tmp/workspace-one",
  };
  const otherSession = {
    ...currentSession,
    id: "22222222-other",
    active: false,
    workspace: "/tmp/workspace-two",
  };
  send({
    type: "snapshot",
    request_id: null,
    notice: null,
    error: null,
    snapshot: {
      ...snapshot,
      sessions: [currentSession],
      all_sessions: [currentSession, otherSession],
    },
  });
  const message = screen.getByRole("textbox", { name: "消息" });
  fireEvent.change(message, { target: { value: "保留这段草稿" } });
  fireEvent.click(screen.getByRole("button", { name: "更改" }));
  const path = screen.getByRole("textbox", { name: "Workspace 路径" });
  expect((path as HTMLInputElement).value).toBe("/tmp/workspace-one");
  mock.dialogOpen.mockResolvedValueOnce("/picked/workspace");
  fireEvent.click(screen.getByRole("button", { name: "浏览" }));
  await waitFor(() =>
    expect(mock.dialogOpen).toHaveBeenCalledWith({
      directory: true,
      multiple: false,
      defaultPath: "/tmp/workspace-one",
    }),
  );
  expect((path as HTMLInputElement).value).toBe("/picked/workspace");
  mock.dialogOpen.mockResolvedValueOnce(null);
  fireEvent.click(screen.getByRole("button", { name: "浏览" }));
  await waitFor(() => expect(mock.dialogOpen).toHaveBeenCalledTimes(2));
  expect((path as HTMLInputElement).value).toBe("/picked/workspace");
  expect(
    mock.invoke.mock.calls.filter(([name]) => name === "gui_submit"),
  ).toHaveLength(0);
  fireEvent.change(path, { target: { value: "/missing workspace" } });
  fireEvent.click(
    screen.getByRole("button", { name: "切换并新建会话" }),
  );
  await waitFor(() =>
    expect(mock.invoke).toHaveBeenCalledWith("gui_submit", {
      requestId: 1,
      line: "/workspace /missing workspace",
    }),
  );
  expect((message as HTMLTextAreaElement).value).toBe("保留这段草稿");
  expect(
    (screen.getByRole("button", { name: "浏览" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  send({
    type: "snapshot",
    request_id: 1,
    notice: null,
    error: "Workspace 切换失败：目录不存在",
    snapshot: {
      ...snapshot,
      sessions: [currentSession],
      all_sessions: [currentSession, otherSession],
    },
  });
  expect((path as HTMLInputElement).value).toBe("/missing workspace");
  expect(screen.getByText(/目录不存在/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "全部" }));
  expect(screen.getByTitle("22222222-other")).toBeTruthy();
  expect(screen.getByText("/tmp/workspace-two")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(screen.queryByRole("textbox", { name: "Workspace 路径" })).toBeNull();
  expect((message as HTMLTextAreaElement).value).toBe("保留这段草稿");
  expect(document.querySelector(".workspace-path")?.textContent).toBe(
    "/tmp/workspace-one",
  );
  send({
    type: "snapshot",
    request_id: null,
    notice: "已切换 workspace：/tmp/workspace-two",
    error: null,
    snapshot: {
      ...snapshot,
      status: {
        ...snapshot.status,
        workspace: "/tmp/workspace-two",
        session_id: "33333333-new",
      },
      sessions: [{ ...otherSession, id: "33333333-new", active: true }],
      all_sessions: [
        { ...otherSession, id: "33333333-new", active: true },
        currentSession,
      ],
    },
  });
  expect(document.querySelector(".workspace-path")?.textContent).toBe(
    "/tmp/workspace-two",
  );
});

it("does not offer a futile retry when persistence is unavailable", () => {
  render(<App />);
  send({
    type: "snapshot",
    request_id: null,
    snapshot,
    notice: null,
    error: null,
  });
  send({
    type: "close_failed",
    report: "仅内存",
    unsaved_ids: ["session-one"],
    can_retry: false,
  });
  expect(screen.getByRole("dialog", { name: "保存失败" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "重试保存" })).toBeNull();
});

const historyA = {
  id: "11111111-1111-4111-8111-111111111111",
  title: "首条消息生成的标题",
  updated_at_ms: 1,
  model: "test-model",
  status: "已保存",
  active: true,
  uncertain_tools: false,
  workspace: "/tmp/workspace-one",
};
const historyB = { ...historyA, id: "22222222-2222-4222-8222-222222222222", active: false };
const historyC = { ...historyB, id: "33333333-3333-4333-8333-333333333333", workspace: "/tmp/workspace-two" };
const historySnapshot: Snapshot = {
  ...snapshot,
  status: { ...snapshot.status, session_id: historyA.id, session_title: historyA.title },
  sessions: [historyA, historyB],
  all_sessions: [historyA, historyB, historyC],
};
function showSessions(next = historySnapshot) {
  send({ type: "snapshot", request_id: null, snapshot: next, notice: null, error: null });
}
function lastSubmit(): { requestId: number; line: string } {
  return mock.invoke.mock.calls.filter(([command]) => command === "gui_submit").at(-1)![1];
}
function selected(id: string) {
  return screen.getByRole("checkbox", { name: new RegExp(`选择 .*${id}`) }) as HTMLInputElement;
}
function completePreview(targets = [historyB]) {
  send({ type: "snapshot", request_id: lastSubmit().requestId, snapshot: historySnapshot,
    notice: null, error: null, delete_confirmation: { targets } });
}

it("selects sessions by UUID and clears selection on scope, management and open commands", async () => {
  render(<App />);
  showSessions();
  expect(screen.getAllByText(historyA.title).length).toBeGreaterThan(1);
  fireEvent.click(screen.getByRole("button", { name: "管理会话" }));
  fireEvent.click(selected(historyB.id));
  expect(selected(historyA.id).checked).toBe(false);
  expect(selected(historyB.id).checked).toBe(true);
  expect(mock.invoke.mock.calls.some(([command]) => command === "gui_submit")).toBe(false);
  showSessions({ ...historySnapshot, sessions: [historyB, historyA] });
  expect(selected(historyB.id).checked).toBe(true);
  fireEvent.click(screen.getByRole("checkbox", { name: "全选当前列表" }));
  expect(screen.getByText("已选 2")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "全部" }));
  expect(screen.getByText("已选 0")).toBeTruthy();
  fireEvent.click(screen.getByRole("checkbox", { name: "全选当前列表" }));
  expect(screen.getByText("已选 3")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "完成管理" }));
  fireEvent.click(screen.getByRole("button", { name: "管理会话" }));
  expect(screen.getByText("已选 0")).toBeTruthy();
  fireEvent.click(selected(historyB.id));
  fireEvent.change(screen.getByRole("textbox", { name: "消息" }), {
    target: { value: `/open ${historyA.id}` },
  });
  fireEvent.click(screen.getByRole("button", { name: "发送 ↗" }));
  await waitFor(() => expect(lastSubmit().line).toBe(`/open ${historyA.id}`));
  send({ type: "snapshot", request_id: lastSubmit().requestId,
    snapshot: historySnapshot, notice: "当前会话未改变", error: null });
  expect(screen.getByText("已选 0")).toBeTruthy();
});

it("previews deletion once, defaults to cancel, preserves drafts and blocks busy actions", async () => {
  render(<App />);
  showSessions();
  const input = screen.getByRole("textbox", { name: "消息" }) as HTMLTextAreaElement;
  fireEvent.change(input, { target: { value: "保留未发送的草稿" } });
  fireEvent.click(screen.getByRole("button", { name: "管理会话" }));
  fireEvent.click(selected(historyB.id));
  fireEvent.click(screen.getByRole("button", { name: "删除所选" }));
  await waitFor(() => expect(lastSubmit().line).toBe(`/delete ${historyB.id}`));
  expect(selected(historyA.id).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "全部" }) as HTMLButtonElement).disabled).toBe(true);
  completePreview();
  const dialog = screen.getByRole("dialog", { name: "删除会话确认" });
  expect(within(dialog).getByText(historyB.id)).toBeTruthy();
  expect(within(dialog).getByText(/保留 Trace 日志/)).toBeTruthy();
  expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "取消" }));
  fireEvent.keyDown(dialog, { key: "Escape" });
  expect(screen.queryByRole("dialog", { name: "删除会话确认" })).toBeNull();
  expect(input.value).toBe("保留未发送的草稿");
  expect(mock.invoke.mock.calls.filter(([command]) => command === "gui_submit")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: "删除所选" }));
  await waitFor(() => expect(lastSubmit().requestId).toBe(2));
  completePreview();
  fireEvent.click(screen.getByRole("button", { name: "确认删除" }));
  await waitFor(() => expect(lastSubmit().line).toBe(`/delete --yes ${historyB.id}`));
  send({ type: "snapshot", request_id: lastSubmit().requestId,
    snapshot: { ...historySnapshot, sessions: [historyA], all_sessions: [historyA, historyC] },
    notice: "已删除，Trace 日志保留", error: null,
    delete_report: { items: [{ id: historyB.id, state: "deleted", error: null }], new_session_id: null } });
  expect(input.value).toBe("保留未发送的草稿");
  expect(screen.queryByRole("checkbox", { name: new RegExp(historyB.id) })).toBeNull();
  expect(screen.getByText("已选 0")).toBeTruthy();
});

it("replaces a deleted current session, retains failed selections and retries residual cleanup", async () => {
  render(<App />);
  showSessions();
  const input = screen.getByRole("textbox", { name: "消息" }) as HTMLTextAreaElement;
  fireEvent.change(input, { target: { value: "当前会话草稿" } });
  fireEvent.click(screen.getByRole("button", { name: "管理会话" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "全选当前列表" }));
  fireEvent.click(screen.getByRole("button", { name: "删除所选" }));
  await waitFor(() => expect(lastSubmit().line).toBe(`/delete ${historyA.id} ${historyB.id}`));
  completePreview([historyA, historyB]);
  expect(screen.getByText(/包含当前会话/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "确认删除" }));
  await waitFor(() => expect(lastSubmit().line).toBe(`/delete --yes ${historyA.id} ${historyB.id}`));
  const replacement = { ...historyA, id: "44444444-4444-4444-8444-444444444444", title: "新会话" };
  const next = { ...historySnapshot,
    status: { ...historySnapshot.status, session_id: replacement.id, session_title: "新会话" },
    sessions: [replacement, historyB], all_sessions: [replacement, historyB, historyC] };
  send({ type: "snapshot", request_id: lastSubmit().requestId, snapshot: next,
    notice: "部分删除失败：存储故障；旧会话已删除，消息清理待重试。", error: null,
    delete_report: { items: [
      { id: historyA.id, state: "cleanup_pending", error: "清理失败" },
      { id: historyB.id, state: "failed", error: "存储故障" },
    ], new_session_id: replacement.id } });
  expect(input.value).toBe("");
  expect(selected(historyB.id).checked).toBe(true);
  expect(selected(replacement.id).checked).toBe(false);
  expect(screen.getByText(/部分删除失败/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: /重试删除 \/ 清理/ }));
  await waitFor(() => expect(lastSubmit().line).toBe(`/delete ${historyA.id} ${historyB.id}`));
});
