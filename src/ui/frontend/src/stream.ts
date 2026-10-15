import type { HostAdapter } from "./host";
import type { Event } from "./protocol";

// 高频正文短暂合并，授权、工具、快照等边界立即送达，不能为刷新节奏打乱事件顺序。
export function connectStream(host: HostAdapter, notify: (event: Event) => void): () => void {
  let active = true;
  let buffered: Extract<Event, { type: "delta" }> | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  function flush() {
    clearTimeout(timer);
    timer = undefined;
    const event = buffered;
    buffered = null;
    if (active && event) notify(event);
  }
  const disconnect = host.connect((event) => {
    if (!active) return;
    if (event.type !== "delta") {
      flush();
      notify(event);
      return;
    }
    if (buffered && buffered.request_id !== event.request_id) flush();
    buffered = buffered ? { ...buffered, text: buffered.text + event.text } : event;
    timer ??= setTimeout(flush, 40);
  });
  return () => {
    active = false;
    clearTimeout(timer);
    buffered = null;
    disconnect();
  };
}
