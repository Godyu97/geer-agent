import { afterEach, expect, it, vi } from "vitest";
import type { HostAdapter } from "./host";
import type { Event } from "./protocol";
import { connectStream } from "./stream";

afterEach(() => vi.useRealTimers());

function connection() {
  vi.useFakeTimers();
  let receive: (event: Event) => void = () => {};
  const disconnected = vi.fn();
  const notify = vi.fn();
  const host: HostAdapter = {
    kind: "web", connect(callback) { receive = callback; return disconnected; },
    submit: vi.fn(), authorize: vi.fn(), copy: vi.fn(), close: vi.fn(), forceClose: vi.fn(),
  };
  const dispose = connectStream(host, notify);
  return { send: (event: Event) => receive(event), notify, dispose, disconnected };
}

it("coalesces a burst of tokens without dropping text and starts a fresh window", () => {
  const client = connection();
  for (let i = 0; i < 100; i++) client.send({ type: "delta", request_id: 1, text: `${i}中` });
  expect(client.notify).not.toHaveBeenCalled();
  vi.advanceTimersByTime(40);
  expect(client.notify).toHaveBeenCalledExactlyOnceWith({ type: "delta", request_id: 1, text: Array.from({ length: 100 }, (_, i) => `${i}中`).join("") });
  client.send({ type: "delta", request_id: 1, text: "下一段" });
  vi.advanceTimersByTime(40);
  expect(client.notify).toHaveBeenCalledTimes(2);
  expect(client.notify).toHaveBeenLastCalledWith({ type: "delta", request_id: 1, text: "下一段" });
  client.dispose();
});

it("flushes text before tool, authorization and completion boundaries without delaying them", () => {
  const client = connection();
  const events: Event[] = [
    { type: "delta", request_id: 1, text: "先读取" },
    { type: "tool_progress", request_id: 1, name: "read" },
    { type: "delta", request_id: 1, text: "需要确认" },
    { type: "authorization", id: 9, prompt: "允许读取？" },
    { type: "delta", request_id: 1, text: "完成" },
    { type: "snapshot", request_id: 1, snapshot: { revision: 2 } as Extract<Event, { type: "snapshot" }>["snapshot"], notice: null, error: null },
  ];
  events.forEach(client.send);
  expect(client.notify.mock.calls.map(([event]) => event)).toEqual(events);
  vi.advanceTimersByTime(80);
  expect(client.notify).toHaveBeenCalledTimes(events.length);
  client.dispose();
});

it("does not combine different requests or dispatch after disposal", () => {
  const client = connection();
  client.send({ type: "delta", request_id: 1, text: "旧请求" });
  client.send({ type: "delta", request_id: 2, text: "新请求" });
  expect(client.notify).toHaveBeenCalledExactlyOnceWith({ type: "delta", request_id: 1, text: "旧请求" });
  client.dispose();
  vi.advanceTimersByTime(80);
  client.send({ type: "authorization", id: 9, prompt: "已断开" });
  expect(client.notify).toHaveBeenCalledTimes(1);
  expect(client.disconnected).toHaveBeenCalledOnce();
  expect(vi.getTimerCount()).toBe(0);
});
