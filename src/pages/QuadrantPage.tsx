import { Check, Grid2x2 } from "lucide-react";
import { useEffect, useState } from "react";
import { api, errorText } from "../lib/api";
import { cn } from "../lib/cn";
import type { Entry } from "../lib/types";
import { useUI } from "../stores/ui";
import { Checkbox } from "../components/ui/Checkbox";

const QUADRANTS = [
  { level: 1, title: "重要且紧急", hint: "马上做", tone: "danger" },
  { level: 2, title: "重要不紧急", hint: "计划做", tone: "accent" },
  { level: 3, title: "紧急不重要", hint: "尽快处理或委托", tone: "warn" },
  { level: 4, title: "不重要不紧急", hint: "有空再说", tone: "muted" },
] as const;

const TONE: Record<string, string> = {
  danger: "border-danger/40 bg-danger/5",
  accent: "border-accent/40 bg-accent/5",
  warn: "border-warn/40 bg-warn/5",
  muted: "border-line bg-panel",
};

/** 四象限：只放公开任务，不关联项目（保密项目的任务不会出现在这里） */
export function QuadrantPage() {
  const revision = useUI((s) => s.revision);
  const setSelectedEntry = useUI((s) => s.setSelectedEntry);
  const [items, setItems] = useState<Entry[]>([]);
  const [unassigned, setUnassigned] = useState<Entry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [dragId, setDragId] = useState<number | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let alive = true;
    Promise.all([api.listQuadrantEntries(), api.listUnassignedEntries()])
      .then(([list, rest]) => {
        if (!alive) return;
        setItems(list);
        setUnassigned(rest);
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

  const move = async (id: number, level: number | null) => {
    try {
      const next = await api.setEntryQuadrant(id, level);
      setItems((prev) => {
        const rest = prev.filter((e) => e.id !== id);
        return next.quadrant == null ? rest : [...rest, next];
      });
      setUnassigned((prev) => {
        const rest = prev.filter((e) => e.id !== id);
        return next.quadrant == null ? [next, ...rest] : rest;
      });
      setError(null);
    } catch (err) {
      setError(errorText(err));
    }
  };

  const toggle = async (entry: Entry, done: boolean) => {
    try {
      await api.setEntryCompleted(entry.id, done);
      // v0.7：勾完成后立即从象限消失（象限只留未完成事项）
      if (done) setItems((prev) => prev.filter((item) => item.id !== entry.id));
      useUI.getState().bumpRevision();
    } catch (err) {
      setError(errorText(err));
    }
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex items-center gap-2 border-b border-line px-6 py-3">
        <Grid2x2 size={16} className="text-accent" />
        <h1 className="text-[15px] font-semibold text-ink">四象限</h1>
        <span className="text-[11.5px] text-ink-faint">
          把事项拖进对应象限；只显示公开任务，不含保密项目
        </span>
      </header>

      {error && <p className="px-6 pt-2 text-[12px] text-danger">{error}</p>}

      <div className="grid min-h-0 flex-1 grid-cols-2 grid-rows-2 gap-3 overflow-y-auto p-4">
        {QUADRANTS.map((quadrant) => {
          const list = items.filter((e) => e.quadrant === quadrant.level);
          return (
            <section
              key={quadrant.level}
              onDragOver={(e) => e.preventDefault()}
              onDrop={() => {
                if (dragId != null) void move(dragId, quadrant.level);
                setDragId(null);
              }}
              className={cn(
                "flex min-h-0 flex-col rounded-xl border p-3",
                TONE[quadrant.tone],
              )}
            >
              <header className="mb-2 flex items-baseline gap-2">
                <h2 className="text-[13px] font-semibold text-ink">
                  {quadrant.level}. {quadrant.title}
                </h2>
                <span className="text-[11px] text-ink-faint">{quadrant.hint}</span>
                <span className="ml-auto text-[11px] text-ink-faint">{list.length} 项</span>
              </header>

              <div className="min-h-0 flex-1 space-y-1 overflow-y-auto">
                {list.length === 0 && (
                  <p className="px-1 py-3 text-[11.5px] text-ink-faint">把下面的任务拖进来</p>
                )}
                {list.map((entry) => (
                  <div
                    key={entry.id}
                    draggable
                    onDragStart={() => setDragId(entry.id)}
                    className="group flex items-start gap-1.5 rounded-md bg-panel/80 px-2 py-1.5 hover:bg-panel"
                  >
                    <div className="mt-0.5">
                      <Checkbox
                        size="sm"
                        checked={Boolean(entry.completedAt)}
                        ariaLabel="标记为完成"
                        onCheckedChange={(v) => void toggle(entry, v)}
                      />
                    </div>
                    <button
                      type="button"
                      onClick={() => setSelectedEntry(entry)}
                      className={cn(
                        "min-w-0 flex-1 text-left text-[12.5px] leading-snug",
                        entry.completedAt ? "text-ink-faint line-through" : "text-ink",
                      )}
                    >
                      {entry.content}
                    </button>
                    <button
                      type="button"
                      title="移出象限"
                      onClick={() => void move(entry.id, null)}
                      className="shrink-0 rounded p-1 text-ink-faint opacity-0 hover:text-danger group-hover:opacity-100"
                    >
                      ×
                    </button>
                  </div>
                ))}
              </div>
            </section>
          );
        })}
      </div>

      <div className="border-t border-line bg-panel-2 px-4 py-2">
        <p className="mb-1 text-[11.5px] text-ink-faint">
          未划分的公开任务（{unassigned.length}）—— 拖到上面某个象限
        </p>
        <div className="flex max-h-[110px] flex-wrap gap-1.5 overflow-y-auto">
          {loading && <span className="text-[11.5px] text-ink-faint">加载中…</span>}
          {!loading && unassigned.length === 0 && (
            <span className="text-[11.5px] text-ink-faint">没有待划分的任务</span>
          )}
          {unassigned.map((entry) => (
            <div
              key={entry.id}
              draggable
              onDragStart={() => setDragId(entry.id)}
              className="flex max-w-[260px] items-center gap-1 rounded-md border border-line bg-panel px-2 py-1 text-[12px] text-ink"
            >
              <span className="truncate">{entry.content}</span>
              <span className="flex shrink-0 gap-0.5">
                {QUADRANTS.map((q) => (
                  <button
                    key={q.level}
                    type="button"
                    title={`放到：${q.title}`}
                    onClick={() => void move(entry.id, q.level)}
                    className="rounded px-1 text-[10.5px] text-ink-faint hover:bg-accent-soft hover:text-accent"
                  >
                    {q.level}
                  </button>
                ))}
              </span>
            </div>
          ))}
        </div>
        <p className="mt-1 flex items-center gap-1 text-[10.5px] text-ink-faint">
          <Check size={10} /> 象限只影响视图，不改变任务本身的日期与项目
        </p>
      </div>
    </div>
  );
}
