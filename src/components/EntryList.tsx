import { Inbox } from "lucide-react";
import type { Entry } from "../lib/types";
import { EntryCard } from "./EntryCard";

type Props = {
  entries: Entry[];
  loading?: boolean;
  error?: string | null;
  emptyText?: string;
  emptyHint?: string;
  onChange: () => void;
};

export function EntryList({ entries, loading, error, emptyText, emptyHint, onChange }: Props) {
  if (error) {
    return (
      <div className="rounded-lg border border-danger/30 bg-danger/5 px-3 py-2 text-[12.5px] text-danger">
        {error}
      </div>
    );
  }

  if (loading) {
    return (
      <div className="space-y-1.5">
        {[0, 1, 2].map((i) => (
          <div key={i} className="h-[52px] animate-pulse rounded-lg border border-line bg-panel" />
        ))}
      </div>
    );
  }

  if (entries.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-1.5 rounded-xl border border-dashed border-line px-6 py-12 text-center">
        <Inbox size={22} className="text-ink-faint" />
        <p className="text-[13px] text-ink-muted">{emptyText ?? "这里还没有记录"}</p>
        {emptyHint && <p className="text-[11.5px] text-ink-faint">{emptyHint}</p>}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-1.5">
      {entries.map((entry) => (
        <EntryCard key={entry.id} entry={entry} onChange={onChange} />
      ))}
    </div>
  );
}
