import { useEffect, useMemo, useState } from "react";
import type { KeyboardEvent } from "react";
import type { MemoryEntry, MemoryPreview, MemoryStatus } from "./protocol";

export function searchMemories(entries: MemoryEntry[], query: string): MemoryEntry[] {
  const terms = query.toLowerCase().trim().split(/\s+/).filter(Boolean);
  if (!terms.length) return entries;
  return entries.map((entry) => ({ entry, score: terms.filter((term) => entry.content.toLowerCase().includes(term)).length }))
    .filter(({ score }) => score > 0)
    .sort((a, b) => b.score - a.score || a.entry.created_at_ms - b.entry.created_at_ms || (a.entry.id < b.entry.id ? -1 : a.entry.id > b.entry.id ? 1 : 0))
    .slice(0, 10).map(({ entry }) => entry);
}

export function memoryLabel(status: MemoryStatus | undefined): string {
  if (!status) return "等待连接";
  if (status.state === "disabled") return "已关闭";
  if (status.state === "unavailable") return "不可用";
  return `全局 ${status.count ?? 0} 条`;
}

function trapFocus(event: KeyboardEvent<HTMLDivElement>) {
  if (event.key !== "Tab") return;
  const fields = Array.from(event.currentTarget.querySelectorAll<HTMLElement>("button:not(:disabled), input:not(:disabled), textarea:not(:disabled)"));
  if (!fields.length) return;
  const index = fields.indexOf(document.activeElement as HTMLElement);
  event.preventDefault();
  fields[(index + (event.shiftKey ? fields.length - 1 : 1)) % fields.length]?.focus();
}

export default function MemoryPanel({ entries, status, disabled, error, notice, onSubmit, onClose }: {
  entries: MemoryEntry[];
  status: MemoryStatus | undefined;
  disabled: boolean;
  error: string | null;
  notice: string | null;
  onSubmit: (line: string) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [editor, setEditor] = useState<{ id: string | null; text: string } | null>(null);
  const [awaiting, setAwaiting] = useState(false);
  const found = useMemo(() => searchMemories(entries, query), [entries, query]);
  const writable = status?.state === "ready" && !disabled;

  useEffect(() => {
    if (!awaiting || disabled) return;
    if (error) setAwaiting(false);
    else if (notice && /^(已添加记忆|记忆已存在|已更新记忆|记忆未改变)/.test(notice)) {
      setEditor(null);
      setAwaiting(false);
    }
  }, [awaiting, disabled, error, notice]);

  function save() {
    if (!editor || !editor.text.trim() || !writable) return;
    setAwaiting(true);
    onSubmit(editor.id ? `/memory edit ${editor.id} ${editor.text}` : `/memory add ${editor.text}`);
  }

  return <div className="modal-backdrop memory-backdrop">
    <div className="modal memory-panel" role="dialog" aria-modal="true" aria-label="记忆管理" onKeyDown={(event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        if (editor && !disabled) { setEditor(null); setAwaiting(false); }
        else if (!disabled) onClose();
      }
      trapFocus(event);
    }}>
      <div className="memory-heading"><h2>长期记忆</h2><button className="secondary" autoFocus disabled={disabled} onClick={onClose}>关闭记忆面板</button></div>
      <p>{memoryLabel(status)} · 同一数据库下的所有 workspace 共享</p>
      {status?.error && <p className="memory-error" role="alert">{status.error}</p>}
      {error && <p className="memory-error" role="alert">{error}</p>}
      {notice && !notice.includes("\n") && <p className="memory-notice" role="status">{notice}</p>}
      <div className="memory-actions">
        <input aria-label="搜索记忆" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="关键词（任意匹配，最多 10 条）" />
        <button className="secondary" disabled={disabled || status?.state === "disabled"} onClick={() => onSubmit("/memory")}>刷新记忆</button>
        <button className="primary" disabled={!writable} onClick={() => { setEditor({ id: null, text: "" }); setAwaiting(false); }}>新增记忆</button>
        <button className="danger" disabled={!writable || !entries.length} onClick={() => onSubmit("/memory clear")}>清空记忆</button>
      </div>
      {editor && <form className="memory-editor" onSubmit={(event) => { event.preventDefault(); save(); }}>
        <label htmlFor="memory-content">{editor.id ? "编辑记忆正文" : "新增记忆正文"}</label>
        {editor.id && <code>{editor.id}</code>}
        <textarea id="memory-content" autoFocus value={editor.text} disabled={disabled} onChange={(event) => setEditor({ ...editor, text: event.target.value })} />
        <div className="memory-actions">
          <button className="secondary" type="button" disabled={disabled} onClick={() => { setEditor(null); setAwaiting(false); }}>取消编辑</button>
          <button className="primary" disabled={!writable || !editor.text.trim()} type="submit">保存记忆</button>
        </div>
      </form>}
      <div className="memory-list" aria-label="记忆列表">
        {status?.state === "ready" && !found.length && <p>{query.trim() ? "没有匹配的记忆。" : "暂无长期记忆。"}</p>}
        {found.map((entry) => <article className="memory-entry" key={entry.id}>
          <code>{entry.id}</code>
          <pre>{entry.content}</pre>
          <small>创建 {new Date(entry.created_at_ms).toLocaleString()} · 更新 {new Date(entry.updated_at_ms).toLocaleString()}</small>
          <div className="memory-actions">
            <button className="secondary" disabled={!writable} onClick={() => { setEditor({ id: entry.id, text: entry.content }); setAwaiting(false); }}>编辑记忆</button>
            <button className="danger" disabled={!writable} onClick={() => onSubmit(`/memory delete ${entry.id}`)}>删除记忆</button>
          </div>
        </article>)}
      </div>
    </div>
  </div>;
}

export function MemoryConfirmation({ preview, disabled, onCancel, onConfirm }: {
  preview: MemoryPreview;
  disabled: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return <div className="modal-backdrop memory-confirmation">
    <div className="modal" role="dialog" aria-modal="true" aria-label="记忆操作确认" onKeyDown={(event) => {
      if (event.key === "Escape") { event.preventDefault(); onCancel(); }
      trapFocus(event);
    }}>
      <h2>{preview.action.kind === "clear" ? `清空 ${preview.count} 条全局记忆？` : `删除 ${preview.count} 条记忆？`}</h2>
      <p>影响同一数据库下的所有 workspace。会话与 Trace 保留；对话中已经引用的记忆仍在历史中。</p>
      {preview.action.kind === "delete" && <code className="memory-id">{preview.action.id}</code>}
      {preview.entries.map((entry) => <pre key={entry.id}>{entry.content}</pre>)}
      {!preview.count && <p>目标记忆已不存在。</p>}
      <div className="modal-actions">
        <button className="secondary" autoFocus onClick={onCancel}>取消</button>
        <button className="danger" disabled={disabled} onClick={onConfirm}>{preview.action.kind === "clear" ? "确认清空" : "确认删除记忆"}</button>
      </div>
    </div>
  </div>;
}
