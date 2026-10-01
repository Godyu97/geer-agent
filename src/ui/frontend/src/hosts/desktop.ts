import { Channel, invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open } from "@tauri-apps/plugin-dialog";
import type { HostAdapter } from "../host";
import type { Event } from "../model";

export const host: HostAdapter = {
  kind: "desktop",
  connect(onEvent) {
    const channel = new Channel<Event>();
    let active = true;
    channel.onmessage = (event) => { if (active) onEvent(event); };
    void invoke("gui_connect", { onEvent: channel }).catch((error) => {
      if (active) onEvent({ type: "startup_error", message: String(error) });
    });
    return () => { active = false; };
  },
  submit({ requestId, line, revision }) {
    return invoke("gui_submit", { requestId, line, ...(revision === undefined ? {} : { revision }) });
  },
  authorize: (id, allowed) => invoke("gui_authorize", { id, allowed }),
  copy: writeText,
  close: () => getCurrentWindow().close(),
  forceClose: () => invoke("gui_force_close"),
  async pickWorkspace(path) {
    const result = await open({ directory: true, multiple: false, defaultPath: path });
    return typeof result === "string" ? result : null;
  },
};
