import { Check, Clock } from "lucide-react";
import { useMemo } from "react";
import { isEffectivelyDone } from "../lib/entries";
import { useTodosRange } from "../hooks/useEntries";
import { api, errorText } from "../lib/api";
import { cn } from "../lib/cn";
import { dueLabel, parseDay, slotOf, TIME_SLOTS, todayStr, WEEKDAYS, weekDays } from "../lib/dates";
import type { Entry } from "../lib/types";
import { useUI } from "../stores/ui";

const ROWS = [{ key: "allDay", label: "全天" }, ...TIME_SLOTS.map((s) => ({ key: s.key, label: s.label }))];

type Props = {
  day: string;
  onOpenDay: (day: string) => void;
};

/** 周视图：7 列 × (全天 + 4 个时段)，待办按截止时间落位，拖到别的日期即改期 */
export function WeekGrid({ day, onOpenDay }: Props) {
  const days = useMemo(() => weekDays(day), [day]);
  const { todos, loading, error } = useTodosRange(days[0], days[6]);
  const today = todayStr();

  const byDay = useMemo(() => {
    const map = new Map<string, Entry[]>();
    for (const d of days) map.set(d, []);
    for (const t of todos) {
      if (!t.dueDate) continue;
      map.get(t.dueDate)?.push(t);
    }
    return map;
  }, [todos, days]);

  const moveTo = async (entryId: number, targetDay: string) => {
    try {
      await api.updateEntry(entryId, { dueDate: targetDay });
      useUI.getState().bumpRevision();
    } catch (err) {
      console.error(errorText(err));
    }
  };

  return (
    <div className="min-h-0 flex-1 overflow-auto">
      {error && <p className="px-1 pb-2 text-[12px] text-danger">{error}</p>}
      {loading && <p className="px-1 pb-2 text-[12px] text-ink-faint">加载中…</p>}

      <div className="grid min-w-[500px]" style={{ gridTemplateColumns: "40px repeat(7, minmax(0,1fr))" }}>
        {/* 表头：星期 + 日期（点击进入日视图） */}
        <div />
        {days.map((d) => {
          const date = parseDay(d);
          const isToday = d === today;
          return (
            <button
              key={d}
              type="button"
              onClick={() => onOpenDay(d)}
              className={cn(
                "border-b border-line px-1 pb-1.5 pt-1 text-center",
                isToday && "bg-accent-soft/40",
              )}
            >
              <span className={cn("block text-[10.5px]", isToday ? "text-accent" : "text-ink-faint")}>
                周{WEEKDAYS[date.getDay()]}
              </span>
              <span
                className={cn(
                  "mt-0.5 inline-flex h-6 min-w-6 items-center justify-center rounded-full px-1 text-[12.5px]",
                  isToday ? "bg-accent font-semibold text-white" : "text-ink",
                )}
              >
                {date.getDate()}
              </span>
            </button>
          );
        })}

        {/* 时间轴行 */}
        {ROWS.map((row) => (
          <div key={row.key} className="contents">
            <div className="border-b border-line/60 px-1 py-1.5 text-right text-[10px] text-ink-faint">
              {row.label}
            </div>
            {days.map((d) => {
              const items = (byDay.get(d) ?? []).filter((t) => slotOf(t.dueTime) === row.key);
              return (
                <div
                  key={`${d}-${row.key}`}
                  onDragOver={(e) => {
                    e.preventDefault();
                    e.dataTransfer.dropEffect = "move";
                  }}
                  onDrop={(e) => {
                    e.preventDefault();
                    const id = Number(e.dataTransfer.getData("text/plain"));
                    if (id) void moveTo(id, d);
                  }}
                  className={cn(
                    "min-h-[52px] space-y-1 border-b border-r border-line/60 p-1",
                    d === today && "bg-accent-soft/20",
                  )}
                >
                  {items.map((t) => (
                    <TodoChip key={t.id} entry={t} onClick={() => onOpenDay(t.day)} />
                  ))}
                </div>
              );
            })}
          </div>
        ))}
      </div>

      <p className="px-1 pt-2 text-[11px] text-ink-faint">
        提示：把待办拖到别的日期列即可改期；点日期表头进入当天视图。
      </p>
    </div>
  );
}

function TodoChip({ entry, onClick }: { entry: Entry; onClick: () => void }) {
  const done = isEffectivelyDone(entry);
  const due = entry.dueDate ? dueLabel(entry.dueDate, entry.dueTime) : null;

  return (
    <button
      type="button"
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData("text/plain", String(entry.id));
        e.dataTransfer.effectAllowed = "move";
      }}
      onClick={onClick}
      className={cn(
        "block w-full cursor-grab rounded border-l-2 px-1.5 py-1 text-left text-[11px] leading-snug transition-colors",
        "hover:brightness-110 active:cursor-grabbing",
        done
          ? "border-l-ink-faint bg-panel-2 text-ink-faint line-through"
          : due?.tone === "overdue"
            ? "border-l-danger bg-danger/10 text-ink"
            : due?.tone === "today"
              ? "border-l-warn bg-warn/10 text-ink"
              : "border-l-accent bg-accent-soft text-ink",
      )}
      title={`${entry.content}${entry.dueTime ? ` · ${entry.dueTime}` : ""}`}
    >
      <span className="flex items-start gap-1">
        {done ? <Check size={10} className="mt-1 shrink-0" /> : <Clock size={10} className="mt-1 shrink-0 opacity-70" />}
        <span className="line-clamp-2">{entry.content}</span>
      </span>
    </button>
  );
}
