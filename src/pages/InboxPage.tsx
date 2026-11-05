import { Inbox, Plus } from "lucide-react";
import { useEffect, useState } from "react";
import { EntryList } from "../components/EntryList";
import { PageHeader } from "../components/PageHeader";
import { api, errorText } from "../lib/api";
import { useVisibleEntries } from "../lib/visibility";
import type { Entry } from "../lib/types";
import { useUI } from "../stores/ui";

/** 收件箱（v0.7）：没有归属项目的记录与任务，先收进来再决定放哪个项目 */
export function InboxPage() {
  const revision = useUI((s) => s.revision);
  const visible = useVisibleEntries();
  const [items, setItems] = useState<Entry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);

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
      await api.createEntry(content, { isTodo: true, projectId: null });
      setDraft("");
      useUI.getState().bumpRevision();
    } catch (err) {
      setError(errorText(err));
    } finally {
      setBusy(false);
    }
  };

  const shown = visible(items);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <PageHeader title="收件箱" subtitle={`没有归属项目的记录与任务 · ${shown.length} 条`} />

      <div className="px-6 pb-3">
        <div className="flex items-center gap-1.5 rounded-xl border border-line bg-panel px-2.5 py-2 focus-within:border-accent/60">
          <Plus size={13} className="shrink-0 text-ink-faint" />
          <input
            value={draft}
            placeholder="先收进来，回车确认（之后可在任务详情里归到项目）"
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.nativeEvent.isComposing) void add();
            }}
            className="h-6 w-full bg-transparent text-[13px] text-ink placeholder:text-ink-faint focus:outline-none"
          />
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-8">
        {shown.length === 0 && !loading ? (
          <div className="flex flex-col items-center justify-center gap-1.5 rounded-xl border border-dashed border-line px-6 py-12 text-center">
            <Inbox size={20} className="text-ink-faint" />
            <p className="text-[13px] text-ink-muted">收件箱是空的</p>
            <p className="text-[11.5px] text-ink-faint">
              没有归属项目的记录和任务都会出现在这里
            </p>
          </div>
        ) : (
          <EntryList
            entries={shown}
            loading={loading}
            error={error}
            emptyText="收件箱是空的"
            emptyHint="在上面写一条，回车就收进来"
            onChange={() => useUI.getState().bumpRevision()}
          />
        )}
      </div>
    </div>
  );
}
