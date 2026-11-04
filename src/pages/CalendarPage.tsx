import { ChevronLeft, ChevronRight, RotateCcw, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { Calendar } from "../components/Calendar";
import { DayView } from "../components/DayView";
import { EntryList } from "../components/EntryList";
import { PageHeader } from "../components/PageHeader";
import { WeekGrid } from "../components/WeekGrid";
import { Button } from "../components/ui/Button";
import { useDayEntries, useTodosRange } from "../hooks/useEntries";
import { api, errorText } from "../lib/api";
import { cn } from "../lib/cn";
import { addDays, humanDay, relativeDayLabel, shortDay, timeOf, todayStr, weekDays } from "../lib/dates";
import type { CalendarMode, Entry } from "../lib/types";
import { useUI } from "../stores/ui";

const MODES: { key: CalendarMode; label: string }[] = [
  { key: "month", label: "月" },
  { key: "week", label: "周" },
  { key: "day", label: "日" },
];

export function CalendarPage() {
  const selectedDay = useUI((s) => s.selectedDay);
  const setSelectedDay = useUI((s) => s.setSelectedDay);
  const mode = useUI((s) => s.calendarMode);
  const setMode = useUI((s) => s.setCalendarMode);
  const revision = useUI((s) => s.revision);

  const { entries, loading, error, reload } = useDayEntries(selectedDay);
  const week = weekDays(selectedDay);
  const { todos: weekTodos } = useTodosRange(week[0], week[6]);

  const [trashView, setTrashView] = useState(false);
  const [trash, setTrash] = useState<Entry[]>([]);
  const [trashError, setTrashError] = useState<string | null>(null);

  useEffect(() => {
    if (!trashView || mode !== "month") return;
    let alive = true;
    api
      .listTrash()
      .then((list) => {
        if (alive) {
          setTrash(list);
          setTrashError(null);
        }
      })
      .catch((err) => {
        if (alive) setTrashError(errorText(err));
      });
    return () => {
      alive = false;
    };
  }, [trashView, mode, revision]);

  const restore = async (entry: Entry) => {
    try {
      await api.restoreEntry(entry.id);
      useUI.getState().bumpRevision();
    } catch (err) {
      setTrashError(errorText(err));
    }
  };

  const purge = async (entry: Entry) => {
    if (!window.confirm("彻底删除后无法恢复，确定吗？")) return;
    try {
      await api.purgeEntry(entry.id);
      useUI.getState().bumpRevision();
    } catch (err) {
      setTrashError(errorText(err));
    }
  };

  const step = mode === "week" ? 7 : 1;
  const title =
    mode === "week"
      ? `${shortDay(week[0])} — ${shortDay(week[6])}`
      : relativeDayLabel(selectedDay);

  const subtitle =
    mode === "week"
      ? `${humanDay(week[0])} 起 · 本周 ${weekTodos.length} 个待办`
      : `${humanDay(selectedDay)} · ${entries.length} 条记录`;

  return (
    <div className="flex min-h-0 flex-1">
      <div className="w-[248px] shrink-0 overflow-y-auto border-r border-line bg-panel-2 px-3 py-4">
        <Calendar selected={selectedDay} onSelect={setSelectedDay} />
      </div>

      <div className="flex min-w-0 flex-1 flex-col">
        <PageHeader
          title={title}
          subtitle={subtitle}
          actions={
            <>
              <div className="flex rounded-md border border-line bg-panel p-0.5">
                {MODES.map((m) => (
                  <button
                    key={m.key}
                    type="button"
                    onClick={() => setMode(m.key)}
                    className={cn(
                      "h-6 w-7 rounded text-[12px]",
                      mode === m.key
                        ? "bg-accent-soft font-medium text-accent"
                        : "text-ink-muted hover:text-ink",
                    )}
                  >
                    {m.label}
                  </button>
                ))}
              </div>

              <div className="flex items-center rounded-md border border-line bg-panel">
                <button
                  type="button"
                  title={mode === "week" ? "上一周" : "前一天"}
                  onClick={() => setSelectedDay(addDays(selectedDay, -step))}
                  className="rounded-l-md px-1.5 py-1.5 text-ink-faint hover:bg-panel-2 hover:text-ink"
                >
                  <ChevronLeft size={14} />
                </button>
                <button
                  type="button"
                  onClick={() => setSelectedDay(todayStr())}
                  className="px-2 py-1 text-[12px] text-ink-muted hover:text-ink"
                >
                  今天
                </button>
                <button
                  type="button"
                  title={mode === "week" ? "下一周" : "后一天"}
                  onClick={() => setSelectedDay(addDays(selectedDay, step))}
                  className="rounded-r-md px-1.5 py-1.5 text-ink-faint hover:bg-panel-2 hover:text-ink"
                >
                  <ChevronRight size={14} />
                </button>
              </div>

              {mode === "month" && (
                <div className="flex rounded-md border border-line bg-panel p-0.5">
                  <button
                    type="button"
                    onClick={() => setTrashView(false)}
                    className={cn(
                      "rounded px-2 py-1 text-[12px]",
                      !trashView ? "bg-accent-soft font-medium text-accent" : "text-ink-muted hover:text-ink",
                    )}
                  >
                    记录
                  </button>
                  <button
                    type="button"
                    onClick={() => setTrashView(true)}
                    className={cn(
                      "rounded px-2 py-1 text-[12px]",
                      trashView ? "bg-accent-soft font-medium text-accent" : "text-ink-muted hover:text-ink",
                    )}
                  >
                    回收站
                  </button>
                </div>
              )}
            </>
          }
        />

        <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-8">
          {mode === "week" && <WeekGrid day={selectedDay} onOpenDay={(d) => { setSelectedDay(d); setMode("day"); }} />}

          {mode === "day" && <DayView day={selectedDay} />}

          {mode === "month" &&
            (trashView ? (
              <TrashList
                items={trash}
                error={trashError}
                onRestore={(e) => void restore(e)}
                onPurge={(e) => void purge(e)}
              />
            ) : entries.length === 0 && !loading ? (
              <EmptyDay day={selectedDay} />
            ) : (
              <EntryList entries={entries} loading={loading} error={error} onChange={reload} />
            ))}
        </div>
      </div>
    </div>
  );
}

function EmptyDay({ day }: { day: string }) {
  return (
    <div className="flex flex-col items-center justify-center gap-1.5 rounded-xl border border-dashed border-line px-6 py-12 text-center">
      <p className="text-[13px] text-ink-muted">{shortDay(day)} 没有记录</p>
      <p className="text-[11.5px] text-ink-faint">用全局快捷键随时记一笔</p>
    </div>
  );
}

function TrashList({
  items,
  error,
  onRestore,
  onPurge,
}: {
  items: Entry[];
  error: string | null;
  onRestore: (entry: Entry) => void;
  onPurge: (entry: Entry) => void;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      {error && <p className="text-[12.5px] text-danger">{error}</p>}
      {items.length === 0 ? (
        <p className="rounded-xl border border-dashed border-line px-6 py-12 text-center text-[13px] text-ink-faint">
          回收站是空的
        </p>
      ) : (
        items.map((entry) => (
          <div key={entry.id} className="flex items-start gap-3 rounded-lg border border-line bg-panel px-3.5 py-2.5">
            <div className="min-w-0 flex-1">
              <p className="line-clamp-2 text-[13px] text-ink-muted">{entry.content}</p>
              <p className="mt-0.5 text-[11.5px] text-ink-faint">
                {shortDay(entry.day)} {timeOf(entry.createdAt)} 删除
              </p>
            </div>
            <div className="flex shrink-0 items-center gap-1">
              <Button variant="ghost" size="sm" onClick={() => onRestore(entry)}>
                <RotateCcw size={12} /> 恢复
              </Button>
              <Button variant="danger" size="sm" onClick={() => onPurge(entry)}>
                <Trash2 size={12} /> 彻底删除
              </Button>
            </div>
          </div>
        ))
      )}
    </div>
  );
}
