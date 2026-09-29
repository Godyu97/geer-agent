// @vitest-environment jsdom
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import App from "./App";
import type { Event, Snapshot } from "./model";

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
