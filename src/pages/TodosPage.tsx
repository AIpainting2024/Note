import { CalendarClock, Check, Clock, ListChecks } from "lucide-react";
import { useEffect, useState } from "react";
import { isOverdueRed } from "../lib/entries";
import { PageHeader } from "../components/PageHeader";
import { Checkbox } from "../components/ui/Checkbox";
import { api, errorText } from "../lib/api";
import { cn } from "../lib/cn";
import { dueLabel, shortDay } from "../lib/dates";
import { projectHex } from "../lib/projects";
import type { Entry, TodoGroup } from "../lib/types";
import { useVisibleEntries } from "../lib/visibility";
import { useUI } from "../stores/ui";

const TABS: { key: TodoGroup; label: string; icon: typeof ListChecks }[] = [
  { key: "pending", label: "未完成", icon: ListChecks },
  { key: "dueToday", label: "今日到期", icon: Clock },
  { key: "overdue", label: "已逾期", icon: CalendarClock },
  { key: "done", label: "已完成", icon: Check },
];

export function TodosPage() {
  const revision = useUI((s) => s.revision);
  const setView = useUI((s) => s.setView);
  const setSelectedDay = useUI((s) => s.setSelectedDay);

  const visible = useVisibleEntries();
  const [group, setGroup] = useState<TodoGroup>("pending");
  const [items, setItems] = useState<Entry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    api
      .listTodos(group)
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
  }, [group, revision]);

  const toggle = async (entry: Entry, done: boolean) => {
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

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <PageHeader
        title="待办"
        subtitle={loading ? "加载中…" : `${TABS.find((t) => t.key === group)?.label} ${items.length} 项`}
      />

      <div className="flex gap-1 px-6 pb-3">
        {TABS.map(({ key, label, icon: Icon }) => (
          <button
            key={key}
            type="button"
            onClick={() => setGroup(key)}
            className={cn(
              "flex h-7 items-center gap-1.5 rounded-md px-2.5 text-[12.5px] transition-colors",
              group === key
                ? "bg-accent-soft font-medium text-accent"
                : "text-ink-muted hover:bg-panel hover:text-ink",
            )}
          >
            <Icon size={13} />
            {label}
          </button>
        ))}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-8">
        {error && <p className="mb-2 text-[12.5px] text-danger">{error}</p>}

        {!loading && visible(items).length === 0 ? (
          <p className="rounded-xl border border-dashed border-line px-6 py-12 text-center text-[13px] text-ink-faint">
            {group === "done" ? "还没有完成的待办" : "这一栏是空的"}
          </p>
        ) : (
          <div className="flex flex-col gap-1">
            {visible(items).map((entry) => {
              const done = Boolean(entry.completedAt);
              const due = entry.dueDate ? dueLabel(entry.dueDate, entry.dueTime) : null;
              return (
                <div
                  key={entry.id}
                  className="group flex items-start gap-2.5 rounded-lg border border-line bg-panel px-3.5 py-2.5"
                >
                  <div className="mt-[3px]">
                    <Checkbox
                      checked={done}
                      ariaLabel={done ? "标记为未完成" : "标记为完成"}
                      onCheckedChange={(checked) => void toggle(entry, checked)}
                    />
                  </div>
                  <div className="min-w-0 flex-1">
                    <p
                      className={cn(
                        "text-[13.5px] leading-relaxed text-ink",
                        done && "text-ink-faint line-through",
                      )}
                    >
                      {entry.content}
                    </p>
                    <div className="mt-1 flex items-center gap-2.5 text-[11.5px] text-ink-faint">
                      {entry.projectName && (
                        <span className="flex min-w-0 items-center gap-1">
                          <span
                            className="h-1.5 w-1.5 shrink-0 rounded-full"
                            style={{ background: projectHex(entry.projectColor) }}
                          />
                          <span className="max-w-[100px] truncate">{entry.projectName}</span>
                        </span>
                      )}
                      {due && (
                        <span
                          className={cn(
                            "flex items-center gap-0.5",
                            isOverdueRed(entry, due.tone) && "text-danger",
                            due.tone === "today" && "text-warn",
                          )}
                        >
                          <Clock size={11} /> {due.text}
                        </span>
                      )}
                      <button
                        type="button"
                        onClick={() => openDay(entry.day)}
                        className="hover:text-accent"
                      >
                        {shortDay(entry.day)} 记录
                      </button>
                      {done && entry.completedAt && (
                        <span>{entry.completedAt.slice(11, 16)} 完成</span>
                      )}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
