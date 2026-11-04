import { Check, GripVertical, Plus, Trash2, X } from "lucide-react";
import { useEffect, useState } from "react";
import { api, errorText } from "../lib/api";
import { imageFromClipboard, insertImageMarker, uploadImageFile } from "../lib/attachments";
import { cn } from "../lib/cn";
import { dueLabel, timeOf } from "../lib/dates";
import type { Entry } from "../lib/types";
import { useProjects } from "../stores/projects";
import { useUI } from "../stores/ui";
import { Button } from "./ui/Button";
import { Checkbox } from "./ui/Checkbox";

/** 右侧任务详情：字段编辑 + 子任务清单（勾选 / 添加 / 改名 / 拖动排序 / 删除） */
export function TaskDetail() {
  const entry = useUI((s) => s.selectedEntry);
  const setSelectedEntry = useUI((s) => s.setSelectedEntry);
  const revision = useUI((s) => s.revision);
  const projects = useProjects((s) => s.items);

  const [title, setTitle] = useState("");
  const [children, setChildren] = useState<Entry[]>([]);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [editingChild, setEditingChild] = useState<number | null>(null);
  const [childDraft, setChildDraft] = useState("");
  const [dragId, setDragId] = useState<number | null>(null);

  useEffect(() => {
    setTitle(entry?.content ?? "");
    setError(null);
    setDraft("");
  }, [entry?.id, entry?.content]);

  useEffect(() => {
    if (!entry) return;
    let alive = true;
    api
      .listChildren(entry.id)
      .then((list) => {
        if (alive) setChildren(list);
      })
      .catch((err) => {
        if (alive) setError(errorText(err));
      });
    return () => {
      alive = false;
    };
  }, [entry?.id, revision]);

  if (!entry) return null;

  const refresh = () => useUI.getState().bumpRevision();

  /** 标题里粘贴截图：存成附件后把标记插到光标处并立即保存 */
  const pasteImage = async (event: React.ClipboardEvent<HTMLTextAreaElement>) => {
    const file = imageFromClipboard(event.clipboardData);
    if (!file) return;
    event.preventDefault();
    try {
      const att = await uploadImageFile(file, entry.id);
      const next = insertImageMarker(event.currentTarget, setTitle, title, att.id);
      setTitle(next);
      void patch({ content: next });
    } catch (err) {
      setError(errorText(err));
    }
  };

  const patch = async (data: Parameters<typeof api.updateEntry>[1]) => {
    try {
      const next = await api.updateEntry(entry.id, data);
      setSelectedEntry({ ...entry, ...next });
      refresh();
    } catch (err) {
      setError(errorText(err));
    }
  };

  const toggleComplete = async (checked: boolean) => {
    const pendingChildren = children.filter((c) => !c.completedAt).length;
    if (checked && pendingChildren > 0) {
      const ok = window.confirm(
        `这条任务还有 ${pendingChildren} 个子任务没完成，仍要标记为完成吗？`,
      );
      if (!ok) return;
    }
    try {
      const next = await api.setEntryCompleted(entry.id, checked);
      setSelectedEntry({ ...entry, ...next });
      refresh();
    } catch (err) {
      setError(errorText(err));
    }
  };

  const addChild = async () => {
    const content = draft.trim();
    if (!content) return;
    try {
      await api.createChild(entry.id, content);
      setDraft("");
      refresh();
    } catch (err) {
      setError(errorText(err));
    }
  };

  const toggleChild = async (child: Entry, checked: boolean) => {
    try {
      await api.setEntryCompleted(child.id, checked);
      refresh();
    } catch (err) {
      setError(errorText(err));
    }
  };

  const removeChild = async (child: Entry) => {
    try {
      await api.deleteEntry(child.id);
      refresh();
    } catch (err) {
      setError(errorText(err));
    }
  };

  const renameChild = async (child: Entry) => {
    const content = childDraft.trim();
    setEditingChild(null);
    if (!content || content === child.content) return;
    try {
      await api.updateEntry(child.id, { content });
      refresh();
    } catch (err) {
      setError(errorText(err));
    }
  };

  const onDropChild = async (targetId: number) => {
    if (dragId == null || dragId === targetId) return;
    const ids = children.map((c) => c.id);
    const from = ids.indexOf(dragId);
    const to = ids.indexOf(targetId);
    if (from < 0 || to < 0) return;
    ids.splice(to, 0, ids.splice(from, 1)[0]);
    setDragId(null);
    try {
      await api.reorderChildren(entry.id, ids);
      refresh();
    } catch (err) {
      setError(errorText(err));
    }
  };

  const done = Boolean(entry.completedAt);
  const doneChildren = children.filter((c) => c.completedAt).length;
  const allChildrenDone = children.length > 0 && doneChildren === children.length;
  const due = entry.dueDate ? dueLabel(entry.dueDate, entry.dueTime) : null;

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-line px-4 py-3">
        <h2 className="text-[12.5px] font-semibold text-ink">任务详情</h2>
        <button
          type="button"
          aria-label="关闭详情"
          onClick={() => setSelectedEntry(null)}
          className="rounded p-1 text-ink-faint hover:bg-panel hover:text-ink"
        >
          <X size={14} />
        </button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
        <div className="flex items-start gap-2">
          <div className="mt-1">
            <Checkbox checked={done} ariaLabel="完成状态" onCheckedChange={(v) => void toggleComplete(v)} />
          </div>
          <textarea
            value={title}
            rows={Math.min(8, title.split("\n").length + 1)}
            onPaste={(e) => void pasteImage(e)}
            onChange={(e) => setTitle(e.target.value)}
            onBlur={() => {
              const next = title.trim();
              if (next && next !== entry.content) void patch({ content: next });
              else setTitle(entry.content);
            }}
            className={cn(
              "min-w-0 flex-1 bg-transparent text-[13.5px] leading-relaxed text-ink focus:outline-none",
              done && "text-ink-faint line-through",
            )}
          />
        </div>

        <div className="mt-3 space-y-2 border-t border-line pt-3">
          <Row label="项目">
            <select
              value={entry.projectId ?? ""}
              onChange={(e) => void patch({ projectId: e.target.value ? Number(e.target.value) : null })}
              className="h-7 w-full rounded-md border border-line bg-panel px-2 text-[12px] text-ink focus:border-accent focus:outline-none"
            >
              <option value="">无项目</option>
              {projects
                .filter((p) => p.status !== "archived" || p.id === entry.projectId)
                .map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
            </select>
          </Row>

          <Row label="截止">
            <div className="flex items-center gap-1.5">
              <input
                type="date"
                value={entry.dueDate ?? ""}
                onChange={(e) =>
                  void patch({
                    dueDate: e.target.value || null,
                    dueTime: e.target.value ? entry.dueTime : null,
                  })
                }
                className="h-7 rounded-md border border-line bg-panel px-2 text-[12px] text-ink focus:border-accent focus:outline-none"
              />
              <input
                type="time"
                value={entry.dueTime ?? ""}
                disabled={!entry.dueDate}
                onChange={(e) => void patch({ dueTime: e.target.value || null })}
                title="设了具体时间才会触发截止前提醒"
                className="h-7 rounded-md border border-line bg-panel px-2 text-[12px] text-ink focus:border-accent focus:outline-none disabled:opacity-40"
              />
            </div>
          </Row>

          <Row label="状态">
            <span className="text-[12px] text-ink-muted">
              {done ? "已完成" : due ? due.text : "进行中"}
              {entry.dueDate ? ` · 创建于 ${timeOf(entry.createdAt)}` : ""}
            </span>
          </Row>
        </div>

        <div className="mt-3 border-t border-line pt-3">
          <div className="mb-2 flex items-center justify-between">
            <h3 className="text-[12.5px] font-semibold text-ink">
              子任务
              {children.length > 0 && (
                <span className="ml-1.5 font-normal text-ink-faint">
                  {doneChildren}/{children.length}
                </span>
              )}
            </h3>
          </div>

          <div className="space-y-0.5">
            {children.map((child) => (
              <div
                key={child.id}
                draggable
                onDragStart={() => setDragId(child.id)}
                onDragOver={(e) => e.preventDefault()}
                onDrop={() => void onDropChild(child.id)}
                className="group flex items-start gap-1.5 rounded px-1 py-1 hover:bg-panel"
              >
                <GripVertical size={12} className="mt-1.5 shrink-0 text-ink-faint/50 opacity-0 group-hover:opacity-100" />
                <div className="mt-1">
                  <Checkbox
                    checked={Boolean(child.completedAt)}
                    size="sm"
                    ariaLabel="完成子任务"
                    onCheckedChange={(v) => void toggleChild(child, v)}
                  />
                </div>
                {editingChild === child.id ? (
                  <textarea
                    autoFocus
                    value={childDraft}
                    rows={Math.min(10, Math.max(1, childDraft.split("\n").length))}
                    onChange={(e) => setChildDraft(e.target.value)}
                    onBlur={() => void renameChild(child)}
                    onKeyDown={(e) => {
                      if (e.nativeEvent.isComposing) return;
                      // Enter 提交，Shift+Enter 换行（子任务也支持多行/粘贴整段文字）
                      if (e.key === "Enter" && !e.shiftKey) {
                        e.preventDefault();
                        void renameChild(child);
                      }
                      if (e.key === "Escape") setEditingChild(null);
                    }}
                    className={cn(
                      "min-h-[24px] min-w-0 flex-1 resize-none rounded border border-line bg-panel",
                      "px-1.5 py-1 text-[12.5px] leading-snug text-ink",
                      "focus:border-accent focus:outline-none",
                    )}
                  />
                ) : (
                  <button
                    type="button"
                    onClick={() => {
                      setEditingChild(child.id);
                      setChildDraft(child.content);
                    }}
                    className={cn(
                      "min-w-0 flex-1 whitespace-pre-wrap break-words text-left text-[12.5px] leading-snug",
                      child.completedAt ? "text-ink-faint line-through" : "text-ink",
                    )}
                  >
                    {child.content}
                  </button>
                )}
                <button
                  type="button"
                  aria-label="删除子任务"
                  onClick={() => void removeChild(child)}
                  className="shrink-0 rounded p-1 text-ink-faint opacity-0 hover:text-danger group-hover:opacity-100"
                >
                  <Trash2 size={11} />
                </button>
              </div>
            ))}
          </div>

          <div className="mt-1.5 flex items-center gap-1.5">
            <Plus size={12} className="shrink-0 text-ink-faint" />
            <input
              value={draft}
              placeholder="添加子任务，回车确认"
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.nativeEvent.isComposing) void addChild();
              }}
              className="h-6 w-full bg-transparent text-[12px] text-ink placeholder:text-ink-faint focus:outline-none"
            />
          </div>

          {allChildrenDone && !done && (
            <div className="mt-2 rounded-md border border-accent/30 bg-accent-soft/50 px-2 py-1.5">
              <p className="text-[11.5px] text-ink-muted">
                子任务全部完成（{doneChildren}/{children.length}），要把这条任务也标记完成吗？
              </p>
              <Button variant="primary" size="sm" className="mt-1.5" onClick={() => void toggleComplete(true)}>
                <Check size={12} /> 标记任务完成
              </Button>
            </div>
          )}
        </div>

        {error && <p className="mt-2 text-[11.5px] text-danger">{error}</p>}
      </div>
    </div>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2">
      <span className="w-[42px] shrink-0 text-[11.5px] text-ink-faint">{label}</span>
      <div className="min-w-0 flex-1">{children}</div>
    </div>
  );
}
