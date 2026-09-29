import { describe, expect, it } from "vitest";
import {
  applyEvent,
  initialState,
  safeExternalHref,
  shouldSubmit,
} from "./model";
import type { Snapshot } from "./model";

const snapshot: Snapshot = {
  status: {
    model: "test",
    session_id: "one",
    context_tokens: 2,
    context_window_tokens: 10,
    turn_tokens: 0,
    total_tokens: 0,
    usage_complete: true,
  },
  sessions: [],
  transcript: [],
  unsaved_ids: [],
};

describe("GUI message ordering", () => {
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
