import { CornerDownLeft } from "lucide-react";
import { useRef, useState } from "react";
import { api, errorText } from "../lib/api";
import {
  imageFromClipboard,
  insertImageMarker,
  uploadImageFile,
} from "../lib/attachments";
import { cn } from "../lib/cn";
import { projectHex, splitProjectToken } from "../lib/projects";
import { useProjects } from "../stores/projects";
import { useUI } from "../stores/ui";
import { Button } from "./ui/Button";

type Props = {
  placeholder?: string;
  autoFocus?: boolean;
  /** 保存成功后回调（快速小窗用于保存后收起） */
  onSaved?: () => void;
  compact?: boolean;
};

export function QuickInput({ placeholder, autoFocus, onSaved, compact }: Props) {
  const [text, setText] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const ref = useRef<HTMLTextAreaElement>(null);
  const findByName = useProjects((s) => s.findByName);

  const { token } = splitProjectToken(text);
  const matched = token ? findByName(token) : undefined;

  const save = async () => {
    const raw = text.trim();
    if (!raw || saving) return;

    // 末尾 #项目名 → 归属项目；匹配不到就整段当正文
    const { content: withoutProject, token } = splitProjectToken(raw);
    const project = token ? findByName(token) : undefined;
    const effective = project ? withoutProject : raw;

    // 「- 」开头视为待办，去掉标记后入库
    const isTodo = /^[-*]\s+/.test(effective);
    const content = effective.replace(/^[-*]\s+/, "");

    setSaving(true);
    setError(null);
    try {
      const created = await api.createEntry(content, { isTodo, projectId: project?.id ?? null });
      // 正文里的 ![](att:<id>) 之前先落在「未归属」状态，保存后认领到这条记录
      await api.claimAttachments(created.id, content).catch(() => undefined);
      setText("");
      if (ref.current) ref.current.style.height = "auto";
      useUI.getState().bumpRevision();
      onSaved?.();
    } catch (err) {
      setError(errorText(err));
    } finally {
      setSaving(false);
    }
  };

  const grow = (el: HTMLTextAreaElement) => {
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 220)}px`;
  };

  /** 粘贴截图：图片先落库（未归属），记录保存后再认领 */
  const pasteImage = async (event: React.ClipboardEvent<HTMLTextAreaElement>) => {
    const file = imageFromClipboard(event.clipboardData);
    if (!file) return;
    event.preventDefault();
    try {
      const att = await uploadImageFile(file, null);
      const next = insertImageMarker(ref.current, setText, text, att.id);
      setText(next);
      if (ref.current) grow(ref.current);
    } catch (err) {
      setError(errorText(err));
    }
  };

  return (
    <div
      className={cn(
        "rounded-xl border border-line bg-panel transition-colors focus-within:border-accent/60",
        compact ? "p-2" : "p-2.5",
      )}
    >
      <textarea
        ref={ref}
        id="quick-input"
        value={text}
        autoFocus={autoFocus}
        rows={compact ? 2 : 2}
        placeholder={
          placeholder ??
          "记一笔……  Enter 换行，Ctrl+Enter 保存；以「- 」开头记为待办，末尾加 #项目名 归到项目"
        }
        onPaste={(e) => void pasteImage(e)}
        onChange={(e) => {
          setText(e.target.value);
          grow(e.target);
        }}
        onKeyDown={(e) => {
          if (e.nativeEvent.isComposing) return;
          if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
            e.preventDefault();
            void save();
          }
        }}
        className={cn(
          "w-full bg-transparent px-1.5 py-1 text-[13.5px] leading-relaxed text-ink",
          "placeholder:text-ink-faint focus:outline-none",
        )}
      />
      <div className="mt-1 flex items-center justify-between gap-2 px-1.5">
        {error ? (
          <span className="truncate text-[11.5px] text-danger">{error}</span>
        ) : matched ? (
          <span className="flex min-w-0 items-center gap-1 text-[11.5px] text-ink-muted">
            <span
              className="h-2 w-2 shrink-0 rounded-full"
              style={{ background: projectHex(matched.color) }}
            />
            <span className="truncate">归属：{matched.name}</span>
          </span>
        ) : token ? (
          <span className="truncate text-[11.5px] text-ink-faint">
            没有名为「{token}」的项目，会当作正文保存
          </span>
        ) : (
          <span className="flex items-center gap-1 text-[11.5px] text-ink-faint">
            <CornerDownLeft size={11} />
            Ctrl + Enter 保存
          </span>
        )}
        <Button
          variant="primary"
          size="sm"
          disabled={!text.trim() || saving}
          onClick={() => void save()}
        >
          {saving ? "保存中…" : "记下"}
        </Button>
      </div>
    </div>
  );
}
