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

const snapshot: Snapshot = {
  status: {
    model: "test-model",
    session_id: "session-one",
    context_tokens: 2,
    context_window_tokens: 100,
    turn_tokens: 0,
    total_tokens: 0,
    usage_complete: true,
  },
  sessions: [],
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
