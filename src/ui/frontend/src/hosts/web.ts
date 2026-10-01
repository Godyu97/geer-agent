import type { HostAdapter } from "../host";
import type { Event } from "../model";

type Reply = { type: "reply"; request_id: number; error: string | null };
export class WebHost implements HostAdapter {
  readonly kind = "web";
  private socket: WebSocket | null = null;
  private notify: ((event: Event) => void) | null = null;
  private replies = new Map<number, { resolve: () => void; reject: (reason: Error) => void; timer: ReturnType<typeof setTimeout> }>();
  private nextControl = -1;
  private exited = false;

  connect(onEvent: (event: Event) => void) {
    this.notify = onEvent;
    this.exited = false;
    let disposed = false;
    let retry: ReturnType<typeof setTimeout> | undefined;
    const open = () => {
      if (disposed || this.exited) return;
      onEvent({ type: "connection", status: "connecting" });
      const socket = new WebSocket(`${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}/api/ws`);
      this.socket = socket;
      socket.onmessage = ({ data }) => {
        if (disposed || this.socket !== socket) return;
        const event = JSON.parse(String(data)) as Event | Reply;
        if (event.type === "reply") {
          const reply = this.replies.get(event.request_id);
          if (!reply) return;
          clearTimeout(reply.timer);
          this.replies.delete(event.request_id);
          if (event.error) reply.reject(new Error(event.error));
          else reply.resolve();
          return;
        }
        if (event.type === "sync") onEvent({ type: "connection", status: "connected" });
        if (event.type === "client_exited") this.exited = true;
        onEvent(event);
      };
      socket.onclose = () => {
        if (this.socket !== socket) return;
        this.socket = null;
        this.rejectPending("连接已断开，操作结果请以重新同步的状态为准。");
        if (disposed || this.exited) return;
        onEvent({ type: "connection", status: "disconnected" });
        void fetch("/api/auth", { cache: "no-store" }).then((response) => {
          if (disposed || this.exited) return;
          if (response.status === 401) onEvent({ type: "connection", status: "unauthenticated" });
          else retry = setTimeout(open, 1500);
        }).catch(() => { if (!disposed && !this.exited) retry = setTimeout(open, 1500); });
      };
    };
    open();
    return () => {
      disposed = true;
      clearTimeout(retry);
      this.notify = null;
      this.socket?.close();
      this.rejectPending("页面已断开。");
    };
  }

  private rejectPending(message: string) {
    for (const reply of this.replies.values()) { clearTimeout(reply.timer); reply.reject(new Error(message)); }
    this.replies.clear();
  }
  private send(requestId: number, command: object): Promise<void> {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) return Promise.reject(new Error("尚未连接到服务。"));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.replies.delete(requestId); reject(new Error("未收到操作确认，请等待同步；不会自动重发。")); }, 10_000);
      this.replies.set(requestId, { resolve, reject, timer });
      this.socket!.send(JSON.stringify({ ...command, request_id: requestId }));
    });
  }
  submit({ requestId, line, revision }: Parameters<HostAdapter["submit"]>[0]) { return this.send(requestId, { type: "submit", line, revision }); }
  authorize(id: number, allowed: boolean) { return this.send(this.nextControl--, { type: "authorize", id, allowed }); }
  async copy(text: string) {
    if (!globalThis.isSecureContext || !navigator.clipboard) throw new Error("请手动复制");
    await navigator.clipboard.writeText(text);
  }
  close(revision?: number) { return this.submit({ requestId: this.nextControl--, line: "/exit", revision }); }
  async forceClose() { this.exited = true; this.socket?.close(); this.notify?.({ type: "client_exited" }); }
}
export const host = new WebHost();
