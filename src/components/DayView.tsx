import { Check, Clock } from "lucide-react";
import { EntryList } from "./EntryList";
import { useDayEntries, useTodosRange } from "../hooks/useEntries";
import { api } from "../lib/api";
import { cn } from "../lib/cn";
import { dueLabel, slotOf, TIME_SLOTS } from "../lib/dates";
import type { Entry } from "../lib/types";
import { useUI } from "../stores/ui";

const ROWS = [{ key: "allDay", label: "全天" }, ...TIME_SLOTS.map((s) => ({ key: s.key, label: s.label }))];

/** 日视图：左侧时间轴看待办落在一天中的哪一段，右侧是当天全部记录 */
export function DayView({ day }: { day: string }) {
  const { todos } = useTodosRange(day, day);
  const { entries, loading, error, reload } = useDayEntries(day);

  const dropAllDay = async (entryId: number) => {
    await api.updateEntry(entryId, { dueTime: null });
    useUI.getState().bumpRevision();
  };

  const moveToSlot = async (entryId: number, slotKey: string) => {
    const slot = TIME_SLOTS.find((s) => s.key === slotKey);
    if (!slot) return;
    await api.updateEntry(entryId, { dueTime: `${String(slot.from).padStart(2, "0")}:00` });
    useUI.getState().bumpRevision();
  };

  return (
    <div className="flex min-h-0 flex-1 gap-4">
      <div className="w-[300px] shrink-0 overflow-y-auto rounded-xl border border-line bg-panel p-2">
        <p className="px-1 pb-1 text-[11px] text-ink-faint">当天待办 · 按截止时间</p>
        {ROWS.map((row) => {
          const items = todos.filter((t) => slotOf(t.dueTime) === row.key);
          return (
            <div
              key={row.key}
              onDragOver={(e) => e.preventDefault()}
              onDrop={(e) => {
                e.preventDefault();
                const id = Number(e.dataTransfer.getData("text/plain"));
                if (!id) return;
                if (row.key === "allDay") void dropAllDay(id);
                else void moveToSlot(id, row.key);
              }}
              className="mb-1 rounded-md border border-line/60 px-2 py-1.5"
            >
              <p className="mb-1 text-[10.5px] text-ink-faint">{row.label}</p>
              <div className="space-y-1">
                {items.length === 0 && <p className="text-[10.5px] text-ink-faint/70">—</p>}
                {items.map((t) => (
                  <DayChip key={t.id} entry={t} />
                ))}
              </div>
            </div>
          );
        })}
        <p className="px-1 pt-1 text-[10.5px] leading-relaxed text-ink-faint">
          把待办拖到不同时段可改时间，拖到「全天」清除时间。
        </p>
      </div>

      <div className="min-w-0 flex-1 overflow-y-auto">
        <EntryList
          entries={entries}
          loading={loading}
          error={error}
          emptyText="这天没有记录"
          emptyHint="在「今日」里记一笔，这里就会出现"
          onChange={reload}
        />
      </div>
    </div>
  );
}

function DayChip({ entry }: { entry: Entry }) {
  const done = Boolean(entry.completedAt);
  const due = entry.dueDate ? dueLabel(entry.dueDate, entry.dueTime) : null;
  return (
    <button
      type="button"
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData("text/plain", String(entry.id));
        e.dataTransfer.effectAllowed = "move";
      }}
      className={cn(
        "flex w-full items-start gap-1 rounded border-l-2 px-1.5 py-1 text-left text-[11.5px] leading-snug",
        done
          ? "border-l-ink-faint bg-panel-2 text-ink-faint line-through"
          : due?.tone === "overdue"
            ? "border-l-danger bg-danger/10 text-ink"
            : "border-l-accent bg-accent-soft text-ink",
      )}
      title={entry.content}
    >
      {done ? <Check size={11} className="mt-0.5 shrink-0" /> : <Clock size={11} className="mt-0.5 shrink-0 opacity-70" />}
      <span className="line-clamp-2">{entry.content}</span>
    </button>
  );
}
