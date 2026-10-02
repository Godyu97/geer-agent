import { describe, expect, it } from "vitest";
import {
  applyEvent,
  groupTranscript,
  initialState,
  safeExternalHref,
  shouldSubmit,
} from "./model";
import type { Snapshot } from "./model";

const snapshot: Snapshot = {
  status: {
    model: "test",
    session_id: "one",
    session_title: "新会话",
    workspace: "/tmp/one",
    context_tokens: 2,
    context_window_tokens: 10,
    turn_tokens: 0,
    total_tokens: 0,
    usage_complete: true,
    instructions_loaded: false, memory: { state: "ready", count: 0, error: null },
  },
  sessions: [],
  all_sessions: [], memories: [],
  transcript: [],
  unsaved_ids: [],
};

describe("GUI message ordering", () => {
  it("isolates live tools from text and discards stale tool events", () => {
    const pending = applyEvent(initialState, {
      type: "queued",
      pending: { id: 2, line: "检查" },
    });
    expect(
      applyEvent(pending, {
        type: "tool_progress",
        request_id: 1,
        name: "old",
      }),
    ).toBe(pending);
    const tool = applyEvent(pending, {
      type: "tool_progress",
      request_id: 2,
      name: "read",
    });
    expect(tool.live).toBe("");
    expect(tool.liveTools).toEqual(["read"]);
    const live = applyEvent(tool, {
      type: "delta",
      request_id: 2,
      text: "可见回答",
    });
    const done = applyEvent(live, {
      type: "snapshot",
      request_id: 2,
      notice: null,
      error: null,
      snapshot: {
        ...snapshot,
        transcript: [{ role: "assistant", text: "可见回答" }],
      },
    });
    expect(done.liveTools).toEqual([]);
    expect(done.live).toBe("");
    expect(done.snapshot?.transcript[0].text).toBe("可见回答");
    expect(
      applyEvent(done, { type: "tool_progress", request_id: 2, name: "late" }),
    ).toBe(done);
  });
  it("separates prose before and after a tool without inserting progress text", () => {
    const pending = {
      ...initialState,
      pending: { id: 1, line: "检查" },
      live: "先检查。",
    };
    const first = applyEvent(pending, {
      type: "tool_progress",
      request_id: 1,
      name: "read",
    });
    const second = applyEvent(first, {
      type: "tool_progress",
      request_id: 1,
      name: "rg",
    });
    const done = applyEvent(second, {
      type: "delta",
      request_id: 1,
      text: "检查完成。",
    });
    expect(done.live).toBe("先检查。\n\n检查完成。");
  });
  it("keeps only deltas and snapshots for the current operation", () => {
    const pending = { ...initialState, pending: { id: 2, line: "hello" } };
    const stale = applyEvent(pending, {
      type: "delta",
      request_id: 1,
      text: "old",
    });
    expect(stale).toBe(pending);
    const live = applyEvent(stale, {
      type: "delta",
      request_id: 2,
      text: "new",
    });
    expect(live.live).toBe("new");
    expect(
      applyEvent(live, {
        type: "snapshot",
        request_id: 1,
        snapshot,
        notice: null,
        error: null,
      }),
    ).toBe(live);
    const done = applyEvent(live, {
      type: "snapshot",
      request_id: 2,
      snapshot,
      notice: null,
      error: null,
    });
    expect(done.pending).toBeNull();
    expect(done.live).toBe("");
  });
  it("keeps the next authorization that arrives before the previous reply returns", () => {
    const first = applyEvent(initialState, {
      type: "authorization",
      id: 1,
      prompt: "ls",
    });
    const next = applyEvent(first, {
      type: "authorization",
      id: 2,
      prompt: "glob",
    });
    const cleared = applyEvent(next, { type: "authorization_cleared", id: 1 });
    expect(cleared.authorization).toEqual({ id: 2, prompt: "glob" });
    expect(
      applyEvent(cleared, { type: "authorization_cleared", id: 2 })
        .authorization,
    ).toBeNull();
  });
  it("clears cancelled authorization on reconnect snapshot without losing a queued operation", () => {
    const stale = {
      ...initialState,
      pending: { id: 2, line: "检查" },
      live: "旧输出",
      liveTools: ["read"],
      liveUsage: 12,
      authorization: { id: 7, prompt: "write" },
    };
    const connected = applyEvent(stale, {
      type: "snapshot",
      request_id: null,
      snapshot: { ...snapshot, authorization_id: null },
      notice: null,
      error: null,
    });
    expect(connected.authorization).toBeNull();
    expect(connected.pending).toEqual(stale.pending);
    expect(connected.live).toBe(stale.live);
    expect(connected.liveTools).toEqual(stale.liveTools);
    expect(connected.liveUsage).toBe(stale.liveUsage);
    const done = applyEvent(connected, {
      type: "snapshot",
      request_id: 2,
      snapshot: { ...snapshot, authorization_id: null },
      notice: null,
      error: null,
    });
    expect(done.pending).toBeNull();
    expect(done.live).toBe("");
    expect(done.liveTools).toEqual([]);
    expect(done.liveUsage).toBe(0);
  });
  it("clears old authorization on queue and keeps valid authorization across snapshots", () => {
    const stale = { ...initialState, authorization: { id: 7, prompt: "old" } };
    const queued = applyEvent(stale, {
      type: "queued",
      pending: { id: 2, line: "检查" },
    });
    expect(queued.authorization).toBeNull();
    const authorized = applyEvent(queued, { type: "authorization", id: 8, prompt: "read" });
    const completed = {
      type: "snapshot" as const,
      request_id: 2,
      snapshot: { ...snapshot, authorization_id: null },
      notice: null,
      error: null,
    };
    expect(applyEvent(authorized, { ...completed, request_id: 1 })).toBe(authorized);
    expect(applyEvent(authorized, {
      ...completed,
      request_id: null,
      snapshot: { ...snapshot, authorization_id: 8 },
    }).authorization).toEqual({ id: 8, prompt: "read" });
    expect(applyEvent(authorized, completed).authorization).toBeNull();
  });
  it("clears authorization on close and exposes failed saves", () => {
    const authorization = applyEvent(initialState, {
      type: "authorization",
      id: 4,
      prompt: "write file",
    });
    const closing = applyEvent(authorization, { type: "closing" });
    expect(closing.authorization).toBeNull();
    const failed = applyEvent(closing, {
      type: "close_failed",
      report: "failed",
      unsaved_ids: ["s"],
      can_retry: true,
    });
    expect(failed.closing).toBe(false);
    expect(failed.closeFailed?.unsaved_ids).toEqual(["s"]);
  });
});

it("keeps interleaved tools and system notices within their user turn", () => {
  const entries = [
    { role: "system" as const, text: "恢复记录" },
    { role: "user" as const, text: "第一轮" },
    { role: "tool" as const, text: "read" },
    { role: "assistant" as const, text: "继续检查" },
    { role: "tool" as const, text: "rg" },
    { role: "system" as const, text: "压缩完成" },
    { role: "assistant" as const, text: "完成" },
    { role: "user" as const, text: "第二轮" },
  ];
  const turns = groupTranscript(entries);
  expect(turns.map((turn) => turn.start)).toEqual([0, 1, 7]);
  expect(turns[1].entries).toEqual(entries.slice(1, 7));
  expect(groupTranscript([])).toEqual([]);
});

describe("composer keys", () => {
  it("sends on Enter but not while the Chinese IME is composing", () => {
    const key = {
      key: "Enter",
      shiftKey: false,
      isComposing: false,
      keyCode: 13,
    };
    expect(shouldSubmit(key)).toBe(true);
    expect(shouldSubmit({ ...key, isComposing: true })).toBe(false);
    expect(shouldSubmit({ ...key, keyCode: 229 })).toBe(false);
    expect(shouldSubmit({ ...key, shiftKey: true })).toBe(false);
  });
});

it("limits rendered links to external HTTP(S) destinations", () => {
  expect(safeExternalHref("https://example.com/docs")).toBe(
    "https://example.com/docs",
  );
  expect(safeExternalHref("javascript:alert(1)")).toBeUndefined();
  expect(safeExternalHref("file:///etc/passwd")).toBeUndefined();
  expect(safeExternalHref("/local/path")).toBeUndefined();
});
