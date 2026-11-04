import { ChevronRight, CircleCheck, CircleDashed, Clock } from "lucide-react";
import { useEffect, useState } from "react";
import { api, errorText } from "../lib/api";
import { cn } from "../lib/cn";
import { dueLabel, shortDay } from "../lib/dates";
import type { Entry, TodoGroup } from "../lib/types";
import { useUI } from "../stores/ui";
import { Checkbox } from "./ui/Checkbox";

const LIMIT_PER_GROUP = 30;

export function TodoPanel() {
  const revision = useUI((s) => s.revision);
  const setView = useUI((s) => s.setView);
  const setSelectedDay = useUI((s) => s.setSelectedDay);

  const [pending, setPending] = useState<Entry[]>([]);
  const [overdue, setOverdue] = useState<Entry[]>([]);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    try {
      const [p, o] = await Promise.all([
        api.listTodos("pending" satisfies TodoGroup),
        api.listTodos("overdue" satisfies TodoGroup),
      ]);
      setPending(p);
      setOverdue(o);
      setError(null);
    } catch (err) {
      setError(errorText(err));
    }
  };

  useEffect(() => {
    void load();
  }, [revision]);

  const complete = async (entry: Entry, done: boolean) => {
    try {
      await api.setEntryCompleted(entry.id, done);
      useUI.getState().bumpRevision();
    } catch (err) {
      setError(errorText(err));
    }
  };

  const openDay = (day: string) => {
    setSelectedDay(day);
    setView("calendar");
  };

  const openTodos = () => setView("todos");

  const overdueIds = new Set(overdue.map((e) => e.id));
  const rest = pending.filter((e) => !overdueIds.has(e.id));

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between px-4 pb-2 pt-4">
        <h2 className="text-[12.5px] font-semibold tracking-wide text-ink">待办</h2>
        <button
          type="button"
          onClick={openTodos}
          className="flex items-center gap-0.5 rounded px-1 py-0.5 text-[11.5px] text-ink-faint hover:text-accent"
        >
          全部 <ChevronRight size={12} />
        </button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-2.5 pb-4">
        {error && <p className="mx-1.5 mb-2 text-[11.5px] text-danger">{error}</p>}

        {overdue.length > 0 && (
          <Section title={`已逾期 · ${overdue.length}`} tone="danger">
            {overdue.slice(0, LIMIT_PER_GROUP).map((entry) => (
              <TodoItem
                key={entry.id}
                entry={entry}
                onToggle={(done) => void complete(entry, done)}
                onOpen={() => openDay(entry.day)}
              />
            ))}
          </Section>
        )}

        <Section title="未完成" tone="normal">
          {rest.length === 0 && overdue.length === 0 ? (
            <p className="flex items-center gap-1.5 px-1.5 py-3 text-[12px] text-ink-faint">
              <CircleCheck size={13} className="text-ink-faint" />
              没有未完成的待办
            </p>
          ) : (
            rest.slice(0, LIMIT_PER_GROUP).map((entry) => (
              <TodoItem
                key={entry.id}
                entry={entry}
                onToggle={(done) => void complete(entry, done)}
                onOpen={() => openDay(entry.day)}
              />
            ))
          )}
        </Section>
      </div>
    </div>
  );
}

function Section({
  title,
  tone,
  children,
}: {
  title: string;
  tone: "normal" | "danger";
  children: React.ReactNode;
}) {
  return (
    <div className="mb-3">
      <p
        className={cn(
          "mb-1 px-1.5 text-[11px] font-medium",
          tone === "danger" ? "text-danger" : "text-ink-faint",
        )}
      >
        {title}
      </p>
      <div className="space-y-0.5">{children}</div>
    </div>
  );
}

function TodoItem({
  entry,
  onToggle,
  onOpen,
}: {
  entry: Entry;
  onToggle: (done: boolean) => void;
  onOpen: () => void;
}) {
  const due = entry.dueDate ? dueLabel(entry.dueDate, entry.dueTime) : null;

  return (
    <div className="group flex items-start gap-2 rounded-md px-1.5 py-1.5 hover:bg-panel">
      <div className="mt-[3px]">
        <Checkbox checked={false} size="sm" onCheckedChange={onToggle} ariaLabel="完成待办" />
      </div>
      <button type="button" onClick={onOpen} className="min-w-0 flex-1 text-left">
        <p className="line-clamp-2 text-[12.5px] leading-snug text-ink">{entry.content}</p>
        <p className="mt-0.5 flex items-center gap-1.5 text-[11px] text-ink-faint">
          {due && (
            <span
              className={cn(
                "flex items-center gap-0.5",
                due.tone === "overdue" && "text-danger",
                due.tone === "today" && "text-warn",
              )}
            >
              <Clock size={10} /> {due.text}
            </span>
          )}
          <span className="flex items-center gap-0.5">
            <CircleDashed size={10} /> {shortDay(entry.day)}
          </span>
        </p>
      </button>
    </div>
  );
}
