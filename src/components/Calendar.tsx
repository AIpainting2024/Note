import { ChevronLeft, ChevronRight } from "lucide-react";
import { useMemo, useState } from "react";
import { cn } from "../lib/cn";
import { monthLabel, parseDay, todayStr } from "../lib/dates";
import { useMarkedDays } from "../hooks/useEntries";

const WEEKDAY_LABELS = ["一", "二", "三", "四", "五", "六", "日"];

function ymd(d: Date) {
  const y = d.getFullYear();
  const m = `${d.getMonth() + 1}`.padStart(2, "0");
  const day = `${d.getDate()}`.padStart(2, "0");
  return `${y}-${m}-${day}`;
}

type Props = {
  selected: string;
  onSelect: (day: string) => void;
};

export function Calendar({ selected, onSelect }: Props) {
  const [cursor, setCursor] = useState(() => parseDay(selected));

  const cells = useMemo(() => {
    const first = new Date(cursor.getFullYear(), cursor.getMonth(), 1);
    // 周一为一周第一天
    const offset = (first.getDay() + 6) % 7;
    const start = new Date(first);
    start.setDate(1 - offset);

    return Array.from({ length: 42 }, (_, i) => {
      const d = new Date(start);
      d.setDate(start.getDate() + i);
      return d;
    });
  }, [cursor]);

  const rangeStart = ymd(cells[0]);
  const rangeEnd = ymd(cells[cells.length - 1]);
  const marked = useMarkedDays(rangeStart, rangeEnd);
  const markedSet = useMemo(() => new Set(marked), [marked]);
  const today = todayStr();

  const shift = (months: number) => {
    setCursor(new Date(cursor.getFullYear(), cursor.getMonth() + months, 1));
  };

  return (
    <div className="select-none">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-[12.5px] font-medium text-ink">{monthLabel(cursor)}</span>
        <div className="flex items-center gap-0.5">
          <button
            type="button"
            aria-label="上个月"
            onClick={() => shift(-1)}
            className="rounded p-1 text-ink-faint hover:bg-panel hover:text-ink"
          >
            <ChevronLeft size={14} />
          </button>
          <button
            type="button"
            aria-label="下个月"
            onClick={() => shift(1)}
            className="rounded p-1 text-ink-faint hover:bg-panel hover:text-ink"
          >
            <ChevronRight size={14} />
          </button>
        </div>
      </div>

      <div className="grid grid-cols-7 gap-y-1 text-center">
        {WEEKDAY_LABELS.map((w) => (
          <span key={w} className="pb-1 text-[11px] text-ink-faint">
            {w}
          </span>
        ))}

        {cells.map((d) => {
          const day = ymd(d);
          const inMonth = d.getMonth() === cursor.getMonth();
          const isSelected = day === selected;
          const isToday = day === today;
          const hasEntry = markedSet.has(day);

          return (
            <button
              key={day}
              type="button"
              onClick={() => onSelect(day)}
              className={cn(
                "relative mx-auto flex h-8 w-8 items-center justify-center rounded-md text-[12.5px] transition-colors",
                !inMonth && "text-ink-faint/50",
                inMonth && !isSelected && "text-ink-muted hover:bg-panel",
                isSelected && "bg-accent text-white",
                isToday && !isSelected && "font-semibold text-accent",
              )}
            >
              {d.getDate()}
              {hasEntry && (
                <span
                  className={cn(
                    "absolute bottom-1 h-1 w-1 rounded-full",
                    isSelected ? "bg-white/80" : "bg-accent/70",
                  )}
                />
              )}
            </button>
          );
        })}
      </div>
    </div>
  );
}
