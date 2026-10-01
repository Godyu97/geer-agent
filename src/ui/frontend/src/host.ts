import type { Event } from "./model";

export type Submit = { requestId: number; line: string; revision?: number };
export interface HostAdapter {
  kind: "desktop" | "web";
  connect(onEvent: (event: Event) => void): () => void;
  submit(command: Submit): Promise<void>;
  authorize(id: number, allowed: boolean): Promise<void>;
  copy(text: string): Promise<void>;
  close(revision?: number): Promise<void>;
  forceClose(): Promise<void>;
  pickWorkspace?: (path?: string) => Promise<string | null>;
}
