import { emit } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { CornerDownLeft } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api, errorText } from "../lib/api";
import { Button } from "../components/ui/Button";

/** 全局快捷键唤起的独立小窗：只做一件事——记一笔，然后收起 */
export default function QuickWindow() {
  const [text, setText] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const ref = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    ref.current?.focus();
  }, []);

  const hide = async () => {
    try {
      await getCurrentWindow().hide();
    } catch (err) {
      // 权限缺失或窗口已关闭时不能静默失败，否则小窗会一直留在屏幕上
      setError(`收起小窗失败：${errorText(err)}`);
    }
  };

  const save = async () => {
    const raw = text.trim();
    if (!raw || saving) return;

    const isTodo = /^[-*]\s+/.test(raw);
    const content = raw.replace(/^[-*]\s+/, "");

    setSaving(true);
    setError(null);
    try {
      await api.createEntry(content, { isTodo });
      setText("");
      await emit("entries-changed");
      await hide();
    } catch (err) {
      setError(errorText(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="flex h-full flex-col bg-panel text-ink">
      <div
        data-tauri-drag-region
        className="flex h-7 shrink-0 items-center justify-between px-3 text-[11px] text-ink-faint"
      >
        <span data-tauri-drag-region>快速记录</span>
        <span data-tauri-drag-region>Esc 收起</span>
      </div>

      <div className="min-h-0 flex-1 px-3">
        <textarea
          ref={ref}
          value={text}
          rows={3}
          placeholder="随手记一笔……  以「- 」开头记为待办"
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.nativeEvent.isComposing) return;
            if (e.key === "Escape") {
              e.preventDefault();
              void hide();
            }
            if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
              e.preventDefault();
              void save();
            }
          }}
          className="h-full w-full bg-transparent text-[13.5px] leading-relaxed text-ink placeholder:text-ink-faint focus:outline-none"
        />
      </div>

      <div className="flex shrink-0 items-center justify-between gap-2 px-3 pb-2.5">
        {error ? (
          <span className="truncate text-[11.5px] text-danger">{error}</span>
        ) : (
          <span className="flex items-center gap-1 text-[11px] text-ink-faint">
            <CornerDownLeft size={11} /> Ctrl + Enter 保存并收起
          </span>
        )}
        <Button variant="primary" size="sm" disabled={!text.trim() || saving} onClick={() => void save()}>
          {saving ? "保存中…" : "记下"}
        </Button>
      </div>
    </div>
  );
}
