import { endOfMonth, endOfWeek, eachDayOfInterval, format, isSameMonth, startOfMonth, startOfWeek } from "date-fns";
import { Plus } from "lucide-react";
import { useState } from "react";
import { cn } from "../lib/cn";
import type { Entry } from "../lib/types";
import { Checkbox } from "./ui/Checkbox";

type Props = {
  /** 当前月里的一天（YYYY-MM-DD） */
  anchor: string;
  /** 区间内的记录 + 任务（含按录入日、按截止日两种来源） */
  items: Entry[];
  loading: boolean;
  onOpenDay: (day: string) => void;
  onAdd: (day: string, content: string) => Promise<void>;
  onToggle: (entry: Entry, done: boolean) => Promise<void>;
};

const WEEK_LABELS = ["一", "二", "三", "四", "五", "六", "日"];
const MAX_VISIBLE = 3;

/**
 * 月度方格视图（v0.6 起，v0.7 扩展）：
 * 每个日期是一个正方形，格子里列出当天内容——
 * - 有截止日期的任务：放在截止日那天；
 * - 记录（便签）与没有截止日期的任务：放在**录入日**那天。
 * 任务前面有小方块可勾选完成；记录前面是小圆点（记录没有完成态）。
 * 也可以在格子里直接新增任务。只展示公开内容（保密项目不进日历）。
 */
export function MonthSquares({ anchor, items, loading, onOpenDay, onAdd, onToggle }: Props) {
  const base = new Date(`${anchor}T00:00:00`);
  const monthStart = startOfMonth(base);
  const gridStart = startOfWeek(monthStart, { weekStartsOn: 1 });
  const gridEnd = endOfWeek(endOfMonth(base), { weekStartsOn: 1 });
  const days = eachDayOfInterval({ start: gridStart, end: gridEnd });
  const today = format(new Date(), "yyyy-MM-dd");

  const [addingDay, setAddingDay] = useState<string | null>(null);
  const [draft, setDraft] = useState("");

  const visible = items.filter((entry) => !entry.projectIsSecret);

  // 归位规则：有截止日期 → 截止日；否则 → 录入日
  const byDay = new Map<string, Entry[]>();
  for (const entry of visible) {
    const key = entry.dueDate ?? entry.day;
    if (!key) continue;
    const list = byDay.get(key) ?? [];
    list.push(entry);
    byDay.set(key, list);
  }
  // 同一天里：未完成在前、已完成沉底；已完成内部按完成时间倒序（v0.9）
  for (const list of byDay.values()) {
    list.sort((a, b) => {
      const doneA = a.completedAt ? 1 : 0;
      const doneB = b.completedAt ? 1 : 0;
      if (doneA !== doneB) return doneA - doneB;
      if (doneA === 1) {
        const byDoneAt = (b.completedAt ?? "").localeCompare(a.completedAt ?? "");
        if (byDoneAt !== 0) return byDoneAt;
      }
      if (a.isTodo !== b.isTodo) return a.isTodo ? -1 : 1;
      return a.createdAt.localeCompare(b.createdAt);
    });
  }

  const submit = async (day: string) => {
    const content = draft.trim();
    if (!content) {
      setAddingDay(null);
      return;
    }
    await onAdd(day, content);
    setDraft("");
    setAddingDay(null);
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="mb-1.5 grid grid-cols-7 gap-1.5">
        {WEEK_LABELS.map((label) => (
          <div key={label} className="text-center text-[11px] text-ink-faint">
            周{label}
          </div>
        ))}
      </div>

      <div
        className="grid min-h-0 flex-1 grid-cols-7 gap-1.5"
        style={{ gridTemplateRows: `repeat(${days.length / 7}, minmax(0, 1fr))` }}
      >
        {days.map((date) => {
          const day = format(date, "yyyy-MM-dd");
          const list = byDay.get(day) ?? [];
          const inMonth = isSameMonth(date, base);
          const isToday = day === today;
          return (
            <div
              key={day}
              onDoubleClick={() => onOpenDay(day)}
              className={cn(
                "group flex min-h-[92px] min-w-0 flex-col overflow-hidden rounded-lg border p-1.5",
                inMonth ? "border-line bg-panel" : "border-line/50 bg-panel-2/40",
                isToday && "border-accent/60 ring-1 ring-accent/30",
              )}
              title="双击看这一天"
            >
              <div className="mb-1 flex items-center gap-1">
                <span
                  className={cn(
                    "text-[11.5px]",
                    isToday ? "font-semibold text-accent" : inMonth ? "text-ink-muted" : "text-ink-faint/60",
                  )}
                >
                  {format(date, "d")}
                </span>
                <button
                  type="button"
                  title="在这一天加一条任务"
                  onClick={() => {
                    setAddingDay(day);
                    setDraft("");
                  }}
                  className="ml-auto rounded p-0.5 text-ink-faint opacity-0 hover:bg-panel-2 hover:text-accent group-hover:opacity-100"
                >
                  <Plus size={11} />
                </button>
              </div>

              <div className="min-h-0 flex-1 space-y-0.5 overflow-y-auto">
                {list.slice(0, MAX_VISIBLE).map((entry) => (
                  <div key={entry.id} className="flex items-start gap-1">
                    <div className="mt-[2px] shrink-0">
                      {entry.isTodo ? (
                        <Checkbox
                          size="sm"
                          checked={Boolean(entry.completedAt)}
                          ariaLabel="标记为完成"
                          onCheckedChange={(checked) => void onToggle(entry, checked)}
                        />
                      ) : (
                        <span
                          aria-hidden
                          className="mt-[5px] block h-[5px] w-[5px] rounded-full bg-ink-faint/70"
                        />
                      )}
                    </div>
                    <button
                      type="button"
                      onClick={() => onOpenDay(day)}
                      className={cn(
                        "min-w-0 flex-1 truncate text-left text-[11px] leading-snug hover:underline",
                        entry.completedAt ? "text-ink-faint line-through" : "text-ink",
                      )}
                      title={entry.content}
                    >
                      {entry.content}
                    </button>
                  </div>
                ))}
                {list.length > MAX_VISIBLE && (
                  <button
                    type="button"
                    onClick={() => onOpenDay(day)}
                    className="text-[10.5px] text-ink-faint hover:text-accent"
                  >
                    还有 {list.length - MAX_VISIBLE} 条
                  </button>
                )}
                {addingDay === day && (
                  <input
                    autoFocus
                    value={draft}
                    onChange={(e) => setDraft(e.target.value)}
                    onBlur={() => void submit(day)}
                    onKeyDown={(e) => {
                      if (e.nativeEvent.isComposing) return;
                      if (e.key === "Enter") void submit(day);
                      if (e.key === "Escape") {
                        setDraft("");
                        setAddingDay(null);
                      }
                    }}
                    placeholder="新任务…"
                    className="h-5 w-full rounded border border-accent/50 bg-panel-2 px-1 text-[11px] text-ink focus:outline-none"
                  />
                )}
              </div>
            </div>
          );
        })}
      </div>

      {loading && <p className="mt-1 text-[11px] text-ink-faint">加载中…</p>}
    </div>
  );
}
