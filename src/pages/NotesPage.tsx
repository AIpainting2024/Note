import { Plus, StickyNote } from "lucide-react";
import { useEffect, useState } from "react";
import { PageHeader } from "../components/PageHeader";
import { Button } from "../components/ui/Button";
import { Modal } from "../components/ui/Modal";
import { api, errorText } from "../lib/api";
import { shortDay } from "../lib/dates";
import type { Entry } from "../lib/types";
import { useVisibleEntries } from "../lib/visibility";
import { useUI } from "../stores/ui";

/** 日常便签色（浅底深字，深浅色模式下都像真的便签纸） */
const NOTE_COLORS = [
  { bg: "#FFF3B0", border: "#EBDC94", pin: "#D9B23A" }, // 鹅黄
  { bg: "#D9F0C6", border: "#C2E0AC", pin: "#5C9B48" }, // 浅绿
  { bg: "#FFDCE3", border: "#F3C4CE", pin: "#C96A80" }, // 浅粉
  { bg: "#D8EBF7", border: "#BFDCEF", pin: "#4A87B0" }, // 浅蓝
  { bg: "#FFE5C7", border: "#F3CFA8", pin: "#C2803A" }, // 浅橙
  { bg: "#E8DEF6", border: "#D6C8EC", pin: "#7C63B0" }, // 淡紫
];

/** 同一条便签永远同一个颜色，重启也不会跳 */
function colorFor(id: number) {
  return NOTE_COLORS[Math.abs(id) % NOTE_COLORS.length];
}

/**
 * 便签贴（v0.11，原「收件箱」）：
 * 只展示没有归属项目的**记录**（便签），正方形卡片网格，不占用右侧任务详情。
 * 双击便签进入编辑。
 */
export function NotesPage() {
  const revision = useUI((s) => s.revision);
  const visible = useVisibleEntries();
  const [items, setItems] = useState<Entry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState<Entry | null>(null);
  const [editText, setEditText] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    api
      .listInboxEntries()
      .then((list) => {
        if (!alive) return;
        setItems(list);
        setError(null);
      })
      .catch((err) => {
        if (alive) setError(errorText(err));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [revision]);

  const add = async () => {
    const content = draft.trim();
    if (!content || busy) return;
    setBusy(true);
    try {
      // 便签 = 记录，不是待办
      await api.createEntry(content, { isTodo: false, projectId: null });
      setDraft("");
      useUI.getState().bumpRevision();
    } catch (err) {
      setError(errorText(err));
    } finally {
      setBusy(false);
    }
  };

  const openEdit = (entry: Entry) => {
    setEditing(entry);
    setEditText(entry.content);
  };

  const save = async () => {
    if (!editing || saving) return;
    const content = editText.trim();
    if (!content) return;
    setSaving(true);
    try {
      await api.updateEntry(editing.id, { content });
      setEditing(null);
      useUI.getState().bumpRevision();
    } catch (err) {
      setError(errorText(err));
    } finally {
      setSaving(false);
    }
  };

  const remove = async () => {
    if (!editing) return;
    try {
      await api.deleteEntry(editing.id);
      setEditing(null);
      useUI.getState().bumpRevision();
    } catch (err) {
      setError(errorText(err));
    }
  };

  const notes = visible(items);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <PageHeader
        title="便签贴"
        subtitle={loading ? "加载中…" : `${notes.length} 张便签 · 双击可以编辑`}
      />

      <div className="px-6 pb-4">
        <div className="flex items-center gap-1.5 rounded-xl border border-line bg-panel px-2.5 py-2 focus-within:border-accent/60">
          <Plus size={13} className="shrink-0 text-ink-faint" />
          <input
            value={draft}
            placeholder="写一张便签，回车贴上"
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.nativeEvent.isComposing) void add();
            }}
            className="h-6 w-full bg-transparent text-[13px] text-ink placeholder:text-ink-faint focus:outline-none"
          />
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-10">
        {error && <p className="mb-2 text-[12.5px] text-danger">{error}</p>}

        {!loading && notes.length === 0 ? (
          <div className="flex flex-col items-center justify-center gap-1.5 rounded-xl border border-dashed border-line px-6 py-14 text-center">
            <StickyNote size={22} className="text-ink-faint" />
            <p className="text-[13px] text-ink-muted">还没有便签</p>
            <p className="text-[11.5px] text-ink-faint">在上面写一张，回车就贴上来</p>
          </div>
        ) : (
          <div className="grid gap-3 [grid-template-columns:repeat(auto-fill,minmax(168px,1fr))]">
            {notes.map((note) => {
              const color = colorFor(note.id);
              return (
                <button
                  key={note.id}
                  type="button"
                  title="双击编辑"
                  aria-label={`便签：${note.content}`}
                  onDoubleClick={() => openEdit(note)}
                  style={{ background: color.bg, borderColor: color.border }}
                  className="group relative flex aspect-square flex-col overflow-hidden rounded-lg border p-3 text-left shadow-sm transition-transform hover:-translate-y-0.5 hover:shadow-md"
                >
                  <span
                    className="absolute left-1/2 top-0 h-2 w-10 -translate-x-1/2 rounded-b"
                    style={{ background: color.pin, opacity: 0.55 }}
                  />
                  <p className="mt-1 line-clamp-6 whitespace-pre-wrap break-words text-[12.5px] leading-relaxed text-[#3a352c]">
                    {note.content}
                  </p>
                  <span className="mt-auto flex items-center justify-between pt-2 text-[10.5px] text-[#7a6f5c]">
                    <span>{shortDay(note.day)}</span>
                    <span className="opacity-0 transition-opacity group-hover:opacity-100">
                      双击编辑
                    </span>
                  </span>
                </button>
              );
            })}
          </div>
        )}
      </div>

      <Modal
        open={Boolean(editing)}
        onOpenChange={(open) => {
          if (!open) setEditing(null);
        }}
        title="编辑便签"
        className="w-[520px]"
      >
        <div className="p-4">
          <h2 className="mb-3 text-[14px] font-semibold text-ink">编辑便签</h2>
          <textarea
            value={editText}
            autoFocus
            rows={6}
            onChange={(e) => setEditText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && !e.nativeEvent.isComposing) {
                e.preventDefault();
                void save();
              }
            }}
            className="w-full resize-none rounded-lg border border-line bg-panel px-3 py-2 text-[13px] leading-relaxed text-ink focus:border-accent/60 focus:outline-none"
          />
          <p className="mt-1 text-[11px] text-ink-faint">Ctrl+Enter 保存</p>
          <div className="mt-4 flex items-center justify-between">
            <Button variant="ghost" size="sm" onClick={() => void remove()}>
              删除
            </Button>
            <div className="flex gap-2">
              <Button variant="ghost" size="sm" onClick={() => setEditing(null)}>
                取消
              </Button>
              <Button variant="primary" size="sm" disabled={saving} onClick={() => void save()}>
                保存
              </Button>
            </div>
          </div>
        </div>
      </Modal>
    </div>
  );
}
