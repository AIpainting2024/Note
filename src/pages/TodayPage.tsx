import { EntryList } from "../components/EntryList";
import { PageHeader } from "../components/PageHeader";
import { QuickInput } from "../components/QuickInput";
import { useCounts, useDayEntries } from "../hooks/useEntries";
import { humanDay, todayStr } from "../lib/dates";
import { useVisibleEntries } from "../lib/visibility";

export function TodayPage() {
  const day = todayStr();
  const { entries, loading, error, reload } = useDayEntries(day);
  const visible = useVisibleEntries();
  const counts = useCounts();

  const subtitle = counts
    ? `${humanDay(day)} · 今日 ${counts.todayEntries} 条记录 · 待办 ${counts.pendingTodos} 项${
        counts.overdueTodos > 0 ? ` · 逾期 ${counts.overdueTodos}` : ""
      }`
    : humanDay(day);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <PageHeader title="今天" subtitle={subtitle} />

      <div className="px-6 pb-3">
        <QuickInput />
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-8">
        <EntryList
          entries={visible(entries)}
          loading={loading}
          error={error}
          emptyText="今天还没有记录"
          emptyHint="在上面写一句，Ctrl+Enter 就存下来了"
          onChange={reload}
        />
      </div>
    </div>
  );
}
