import { Search } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api, errorText } from "../lib/api";
import { shortDay, humanDay } from "../lib/dates";
import { HighlightedSnippet } from "../lib/markdown";
import { projectHex } from "../lib/projects";
import type { SearchHit } from "../lib/types";
import { useUI } from "../stores/ui";
import { Modal } from "./ui/Modal";

export function SearchDialog() {
  const open = useUI((s) => s.searchOpen);
  const setOpen = useUI((s) => s.setSearchOpen);
  const setView = useUI((s) => s.setView);
  const setSelectedDay = useUI((s) => s.setSelectedDay);

  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [active, setActive] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setHits([]);
      setActive(0);
      window.setTimeout(() => inputRef.current?.focus(), 30);
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const q = query.trim();
    if (!q) {
      setHits([]);
      return;
    }
    let alive = true;
    const timer = window.setTimeout(async () => {
      try {
        const result = await api.searchEntries(q, 50);
        if (alive) {
          setHits(result);
          setActive(0);
          setError(null);
        }
      } catch (err) {
        if (alive) setError(errorText(err));
      }
    }, 200);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [query, open]);

  const jump = (hit: SearchHit) => {
    setSelectedDay(hit.entry.day);
    setView("calendar");
    setOpen(false);
  };

  return (
    <Modal open={open} onOpenChange={setOpen} title="搜索" className="overflow-hidden">
      <div className="flex items-center gap-2 border-b border-line px-3.5 py-2.5">
        <Search size={15} className="shrink-0 text-ink-faint" />
        <input
          ref={inputRef}
          value={query}
          placeholder="搜索记录正文…"
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.nativeEvent.isComposing) return;
            if (e.key === "Escape") setOpen(false);
            if (e.key === "ArrowDown") {
              e.preventDefault();
              setActive((i) => Math.min(i + 1, hits.length - 1));
            }
            if (e.key === "ArrowUp") {
              e.preventDefault();
              setActive((i) => Math.max(i - 1, 0));
            }
            if (e.key === "Enter" && hits[active]) {
              e.preventDefault();
              jump(hits[active]);
            }
          }}
          className="h-7 w-full bg-transparent text-[14px] text-ink placeholder:text-ink-faint focus:outline-none"
        />
        <kbd className="shrink-0 rounded border border-line px-1.5 py-0.5 text-[10.5px] text-ink-faint">
          Esc
        </kbd>
      </div>

      <div className="max-h-[52vh] overflow-y-auto p-1.5">
        {error && <p className="px-2 py-3 text-[12.5px] text-danger">{error}</p>}

        {!error && query.trim() === "" && (
          <p className="px-2 py-6 text-center text-[12.5px] text-ink-faint">
            输入关键词搜索全部记录，支持中文
          </p>
        )}

        {!error && query.trim() !== "" && hits.length === 0 && (
          <p className="px-2 py-6 text-center text-[12.5px] text-ink-faint">没有匹配的记录</p>
        )}

        {hits.map((hit, index) => (
          <button
            key={hit.entry.id}
            type="button"
            onClick={() => jump(hit)}
            onMouseEnter={() => setActive(index)}
            className={
              "block w-full rounded-md px-2.5 py-2 text-left " +
              (index === active ? "bg-panel-2" : "hover:bg-panel-2")
            }
          >
            <p className="mb-0.5 flex items-center gap-1.5 text-[11px] text-ink-faint">
              <span>
                {shortDay(hit.entry.day)} · {humanDay(hit.entry.day).split(" ")[1]}
                {hit.entry.isTodo ? " · 待办" : ""}
              </span>
              {hit.entry.projectName && (
                <span className="flex min-w-0 items-center gap-1">
                  <span
                    className="h-1.5 w-1.5 shrink-0 rounded-full"
                    style={{ background: projectHex(hit.entry.projectColor) }}
                  />
                  <span className="max-w-[100px] truncate">{hit.entry.projectName}</span>
                </span>
              )}
              {hit.entry.parentId != null && (
                <span className="min-w-0 truncate">
                  · 父任务：{hit.parentContent ?? "（已删除）"}
                </span>
              )}
            </p>
            <p className="line-clamp-2 text-[13px] leading-relaxed text-ink">
              <HighlightedSnippet snippet={hit.snippet} />
            </p>
          </button>
        ))}
      </div>
    </Modal>
  );
}
