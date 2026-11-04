import {
  CalendarDays,
  ListChecks,
  Pencil,
  Pin,
  PinOff,
  SquareCheckBig,
  Trash2,
  Undo2,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api, errorText } from "../lib/api";
import { cn } from "../lib/cn";
import { dueLabel, timeOf } from "../lib/dates";
import { Markdown } from "../lib/markdown";
import { projectHex } from "../lib/projects";
import type { Entry } from "../lib/types";
import { useUI } from "../stores/ui";
import { Checkbox } from "./ui/Checkbox";

type Props = {
  entry: Entry;
  onChange: () => void;
};

type SaveState = "idle" | "saving" | "saved" | "error";

export function EntryCard({ entry, onChange }: Props) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(entry.content);
  const [saveState, setSaveState] = useState<SaveState>("idle");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [showDue, setShowDue] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const timer = useRef<number | undefined>(undefined);
  const textarea = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (!editing) setDraft(entry.content);
  }, [entry.content, editing]);

  useEffect(() => {
    if (saveState !== "saved") return;
    const id = window.setTimeout(() => setSaveState("idle"), 2000);
    return () => window.clearTimeout(id);
  }, [saveState]);

  useEffect(() => {
    if (showDue) textarea.current?.focus();
  }, [showDue]);

  const commit = async (content: string) => {
    setSaveState("saving");
    try {
      await api.updateEntry(entry.id, { content });
      setSaveState("saved");
      useUI.getState().bumpRevision();
    } catch (err) {
      setSaveState("error");
      setError(errorText(err));
    }
  };

  const scheduleSave = (value: string) => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => void commit(value), 600);
  };

  const patch = async (data: Parameters<typeof api.updateEntry>[1]) => {
    try {
      await api.updateEntry(entry.id, data);
      useUI.getState().bumpRevision();
      onChange();
    } catch (err) {
      setError(errorText(err));
    }
  };

  const remove = async () => {
    try {
      await api.deleteEntry(entry.id);
      useUI.getState().bumpRevision();
      onChange();
    } catch (err) {
      setError(errorText(err));
    }
  };

  const done = Boolean(entry.completedAt);
  const due = entry.dueDate ? dueLabel(entry.dueDate, entry.dueTime) : null;

  const statusText =
    saveState === "saving"
      ? "正在保存…"
      : saveState === "saved"
        ? "已保存"
        : saveState === "error"
          ? "保存失败"
          : "";

  return (
    <div
      className={cn(
        "group relative rounded-lg border bg-panel px-3.5 py-2.5 transition-colors",
        editing ? "border-accent/60" : "border-line hover:border-ink-faint/50",
      )}
      onDoubleClick={() => {
        if (!editing) {
          setEditing(true);
          window.setTimeout(() => textarea.current?.focus(), 0);
        }
      }}
      onClick={(e) => {
        // 点卡片空白处打开右侧任务详情；按钮/输入框/复选框不触发
        const target = e.target as HTMLElement;
        if (target.closest("button, input, textarea, select, a")) return;
        useUI.getState().setSelectedEntry(entry);
      }}
    >
      <div className="flex items-start gap-2.5">
        {entry.isTodo && (
          <div className="mt-[3px]">
            <Checkbox
              checked={done}
              ariaLabel={done ? "标记为未完成" : "标记为完成"}
              onCheckedChange={async (checked) => {
                try {
                  await api.setEntryCompleted(entry.id, checked);
                  useUI.getState().bumpRevision();
                  onChange();
                } catch (err) {
                  setError(errorText(err));
                }
              }}
            />
          </div>
        )}

        <div className="min-w-0 flex-1">
          {editing ? (
            <textarea
              ref={textarea}
              value={draft}
              autoFocus
              rows={Math.min(12, draft.split("\n").length + 1)}
              onChange={(e) => {
                setDraft(e.target.value);
                scheduleSave(e.target.value);
              }}
              onBlur={() => {
                window.clearTimeout(timer.current);
                if (draft.trim() && draft !== entry.content) void commit(draft.trim());
                else setDraft(entry.content);
                setEditing(false);
              }}
              onKeyDown={(e) => {
                if (e.nativeEvent.isComposing) return;
                if (e.key === "Escape") {
                  window.clearTimeout(timer.current);
                  setDraft(entry.content);
                  setEditing(false);
                }
              }}
              className="w-full bg-transparent text-[13.5px] leading-relaxed text-ink focus:outline-none"
            />
          ) : (
            <div className={cn("text-[13.5px] leading-relaxed", done && "text-ink-faint line-through")}>
              <Markdown text={entry.content} />
            </div>
          )}

          <div className="mt-1 flex flex-wrap items-center gap-x-2.5 gap-y-1 text-[11.5px] text-ink-faint">
            <span>{timeOf(entry.createdAt)}</span>
            {entry.projectName && (
              <span className="flex min-w-0 items-center gap-1">
                <span
                  className="h-2 w-2 shrink-0 rounded-full"
                  style={{ background: projectHex(entry.projectColor) }}
                />
                <span className="max-w-[120px] truncate">{entry.projectName}</span>
              </span>
            )}
            {(entry.childTotal ?? 0) > 0 && (
              <span
                className={cn(
                  "flex items-center gap-0.5",
                  entry.childDone === entry.childTotal && "text-accent",
                )}
                title="子任务进度，点开卡片查看详情"
              >
                <ListChecks size={11} />
                {entry.childDone ?? 0}/{entry.childTotal}
              </span>
            )}
            {entry.isPinned && (
              <span className="flex items-center gap-0.5 text-accent">
                <Pin size={11} /> 置顶
              </span>
            )}
            {entry.isTodo && (
              <button
                type="button"
                onClick={() => setShowDue((v) => !v)}
                className={cn(
                  "flex items-center gap-0.5 rounded px-1 py-0.5 hover:bg-panel-2",
                  due?.tone === "overdue" && "text-danger",
                  due?.tone === "today" && "text-warn",
                )}
              >
                <CalendarDays size={11} />
                {due ? due.text : "设截止日期"}
              </button>
            )}
            {statusText && (
              <span className={cn(saveState === "error" && "text-danger")}>{statusText}</span>
            )}
            {error && <span className="text-danger">{error}</span>}
          </div>

          {showDue && entry.isTodo && (
            <div className="mt-2 flex flex-wrap items-center gap-2">
              <input
                type="date"
                value={entry.dueDate ?? ""}
                onChange={(e) => void patch({ dueDate: e.target.value || null, dueTime: e.target.value ? entry.dueTime : null })}
                className="h-7 rounded-md border border-line bg-panel px-2 text-[12px] text-ink focus:border-accent focus:outline-none"
              />
              <input
                type="time"
                value={entry.dueTime ?? ""}
                disabled={!entry.dueDate}
                onChange={(e) => void patch({ dueTime: e.target.value || null })}
                title="可选：具体到几点（设了时间才会触发「截止前提醒」）"
                className="h-7 rounded-md border border-line bg-panel px-2 text-[12px] text-ink focus:border-accent focus:outline-none disabled:opacity-40"
              />
              {entry.dueDate && (
                <button
                  type="button"
                  onClick={() => void patch({ dueDate: null, dueTime: null })}
                  className="text-[11.5px] text-ink-faint hover:text-danger"
                >
                  清除
                </button>
              )}
              <button
                type="button"
                onClick={() => setShowDue(false)}
                className="text-[11.5px] text-ink-muted hover:text-ink"
              >
                完成
              </button>
            </div>
          )}
        </div>

        <div
          className={cn(
            "flex shrink-0 items-center gap-0.5 transition-opacity",
            confirmDelete ? "opacity-100" : "opacity-0 group-hover:opacity-100",
          )}
        >
          {confirmDelete ? (
            <>
              <button
                type="button"
                onClick={() => void remove()}
                className="rounded px-1.5 py-1 text-[11.5px] text-danger hover:bg-danger/10"
              >
                确认删除
              </button>
              <button
                type="button"
                onClick={() => setConfirmDelete(false)}
                className="rounded px-1.5 py-1 text-[11.5px] text-ink-muted hover:bg-panel-2"
              >
                取消
              </button>
            </>
          ) : (
            <>
              <IconAction
                label={entry.isPinned ? "取消置顶" : "置顶"}
                onClick={() => void patch({ isPinned: !entry.isPinned })}
              >
                {entry.isPinned ? <PinOff size={14} /> : <Pin size={14} />}
              </IconAction>
              <IconAction
                label={entry.isTodo ? "取消待办" : "标为待办"}
                onClick={() => void patch({ isTodo: !entry.isTodo })}
              >
                {entry.isTodo ? <Undo2 size={14} /> : <SquareCheckBig size={14} />}
              </IconAction>
              <IconAction
                label="编辑"
                onClick={() => {
                  setEditing(true);
                  window.setTimeout(() => textarea.current?.focus(), 0);
                }}
              >
                <Pencil size={14} />
              </IconAction>
              <IconAction label="删除" danger onClick={() => setConfirmDelete(true)}>
                <Trash2 size={14} />
              </IconAction>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

function IconAction({
  label,
  children,
  onClick,
  danger,
}: {
  label: string;
  children: React.ReactNode;
  onClick: () => void;
  danger?: boolean;
}) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      onClick={onClick}
      className={cn(
        "rounded p-1.5 text-ink-faint hover:bg-panel-2",
        danger ? "hover:text-danger" : "hover:text-ink",
      )}
    >
      {children}
    </button>
  );
}
