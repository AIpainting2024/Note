import { emit } from "@tauri-apps/api/event";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { CornerDownLeft, GripVertical } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { api, errorText } from "../lib/api";
import type { Project } from "../lib/types";
import { Button } from "../components/ui/Button";
import { useSecret } from "../stores/secret";

const WIDTH = 560;
const MIN_HEIGHT = 220;
const MAX_HEIGHT = 560;

/**
 * 全局快捷键唤起的独立小窗（v0.7）：
 * - 顶部条可拖动整个窗口，条上带项目下拉，直接决定这条记到哪个项目；
 * - 输入框随内容自动向下扩展，窗口高度跟着长，不出现内部滚动条。
 */
export default function QuickWindow() {
  const [text, setText] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [projectId, setProjectId] = useState<number | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const ref = useRef<HTMLTextAreaElement>(null);
  const unlocked = useSecret((s) => s.status.unlocked);

  useEffect(() => {
    void useSecret.getState().load();
    api
      .listProjects(true)
      .then((list) => setProjects(list))
      .catch(() => undefined);
  }, []);

  // 保密项目未解锁时不进下拉
  const options = useMemo(
    () => projects.filter((p) => p.status !== "archived" && (!p.isSecret || unlocked)),
    [projects, unlocked],
  );

  const fit = (el: HTMLTextAreaElement) => {
    el.style.height = "auto";
    const textHeight = Math.max(el.scrollHeight, 84);
    el.style.height = `${textHeight}px`;
    const total = 36 + textHeight + 52;
    const height = Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, total));
    void getCurrentWindow()
      .setSize(new LogicalSize(WIDTH, height))
      .catch(() => undefined);
  };

  useEffect(() => {
    ref.current?.focus();
    if (ref.current) fit(ref.current);
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

    // 行首「- 」= 待办；末尾 #项目名 仍可用（下拉选了项目时以拉为准）
    const isTodo = /^[-*]\s+/.test(raw);
    let content = raw.replace(/^[-*]\s+/, "");
    let targetProject = projectId;
    const hash = /(?:^|\s)#([^\s#]+)\s*$/.exec(content);
    if (hash && targetProject == null) {
      const found = options.find((p) => p.name.trim() === hash[1]);
      if (found) {
        targetProject = found.id;
        content = content.slice(0, hash.index).trimEnd();
      }
    }

    setSaving(true);
    setError(null);
    try {
      await api.createEntry(content, { isTodo, projectId: targetProject });
      setText("");
      if (ref.current) fit(ref.current);
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
      {/* 顶部条：整条可拖动窗口（deep 让图标/文字上的点击也算拖动），
          右侧项目下拉是 SELECT，会自己拦截点击，不会被拖动吞掉 */}
      <div
        data-tauri-drag-region="deep"
        className="flex h-9 shrink-0 cursor-grab items-center gap-2 border-b border-line bg-panel-2 px-2.5 select-none active:cursor-grabbing"
        title="按住这里拖动窗口"
      >
        <div className="flex h-full flex-1 items-center gap-1.5">
          <GripVertical size={13} className="text-ink-faint" />
          <span className="text-[11.5px] text-ink-muted">快速记录</span>
        </div>

        <select
          value={projectId ?? ""}
          onChange={(e) => setProjectId(e.target.value ? Number(e.target.value) : null)}
          title="记到哪个项目"
          className="h-6 max-w-[150px] cursor-pointer rounded-md border border-line bg-panel px-1.5 text-[11.5px] text-ink focus:border-accent focus:outline-none"
        >
          <option value="">不归项目</option>
          {options.map((project) => (
            <option key={project.id} value={project.id}>
              {project.name}
            </option>
          ))}
        </select>

        <span className="shrink-0 text-[10.5px] text-ink-faint">Esc 收起</span>
      </div>

      <div className="min-h-0 flex-1 px-3 pt-2">
        <textarea
          ref={ref}
          value={text}
          rows={3}
          placeholder="随手记一笔……  以「- 」开头记为待办"
          onChange={(e) => {
            setText(e.target.value);
            fit(e.currentTarget);
          }}
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
          className="h-full w-full resize-none overflow-hidden bg-transparent text-[13.5px] leading-relaxed text-ink placeholder:text-ink-faint focus:outline-none"
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
