import { Check, Lock } from "lucide-react";
import { useEffect, useState } from "react";
import { errorText } from "../lib/api";
import { cn } from "../lib/cn";
import { PROJECT_COLORS, PROJECT_STATUS_OPTIONS } from "../lib/projects";
import type { Project, ProjectColor, ProjectStatus } from "../lib/types";
import { useProjects } from "../stores/projects";
import { useUI } from "../stores/ui";
import { Button } from "./ui/Button";
import { Input, Textarea } from "./ui/Input";
import { Modal } from "./ui/Modal";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** 传入表示编辑，不传表示新建 */
  project?: Project | null;
};

export function ProjectDialog({ open, onOpenChange, project }: Props) {
  const create = useProjects((s) => s.create);
  const update = useProjects((s) => s.update);
  const setSelectedProject = useUI((s) => s.setSelectedProject);
  const setView = useUI((s) => s.setView);

  const [name, setName] = useState("");
  const [color, setColor] = useState<string>(PROJECT_COLORS[0].key);
  const [description, setDescription] = useState("");
  const [status, setStatus] = useState<ProjectStatus>("active");
  const [isSecret, setIsSecret] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!open) return;
    setName(project?.name ?? "");
    setColor(project?.color ?? PROJECT_COLORS[0].key);
    setDescription(project?.description ?? "");
    setStatus((project?.status as ProjectStatus) ?? "active");
    setIsSecret(Boolean(project?.isSecret));
    setError(null);
  }, [open, project]);

  const submit = async () => {
    const trimmed = name.trim();
    if (!trimmed) {
      setError("项目名称不能为空");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      if (project) {
        await update(project.id, {
          name: trimmed,
          color: color as ProjectColor,
          description: description.trim() || null,
          status,
          isSecret,
        });
        onOpenChange(false);
      } else {
        const created = await create(trimmed, {
          color,
          description: description.trim() || null,
          isSecret,
        });
        if (created) {
          onOpenChange(false);
          setSelectedProject(created.id);
          setView("project");
        } else {
          setError(useProjects.getState().error ?? "创建失败");
        }
      }
    } catch (err) {
      setError(errorText(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      title={project ? "编辑项目" : "新建项目"}
      className="w-[460px]"
    >
      <div className="p-4">
        <h2 className="mb-3 text-[14px] font-semibold text-ink">{project ? "编辑项目" : "新建项目"}</h2>

        <label className="mb-1 block text-[12px] text-ink-muted">项目名称</label>
        <Input
          value={name}
          autoFocus
          placeholder="例如：机器人风险系统"
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.nativeEvent.isComposing) void submit();
          }}
        />

        <label className="mb-1 mt-3 block text-[12px] text-ink-muted">颜色</label>
        <div className="flex gap-1.5">
          {PROJECT_COLORS.map((c) => (
            <button
              key={c.key}
              type="button"
              title={c.label}
              aria-label={c.label}
              onClick={() => setColor(c.key)}
              className={cn(
                "h-6 w-6 rounded-full border-2 transition-transform",
                color === c.key ? "scale-110 border-ink" : "border-transparent",
              )}
              style={{ background: c.hex }}
            />
          ))}
        </div>

        <label className="mb-1 mt-3 block text-[12px] text-ink-muted">描述（可选）</label>
        <Textarea
          value={description}
          rows={2}
          placeholder="这个项目要做什么"
          onChange={(e) => setDescription(e.target.value)}
        />

        {project && (
          <>
            <label className="mb-1 mt-3 block text-[12px] text-ink-muted">状态</label>
            <div className="flex gap-1.5">
              {PROJECT_STATUS_OPTIONS.map((s) => (
                <button
                  key={s.key}
                  type="button"
                  onClick={() => setStatus(s.key)}
                  className={cn(
                    "h-7 rounded-md border px-2.5 text-[12px]",
                    status === s.key
                      ? "border-accent bg-accent-soft font-medium text-accent"
                      : "border-line text-ink-muted hover:text-ink",
                  )}
                >
                  {s.label}
                </button>
              ))}
            </div>
          </>
        )}

        <button
          type="button"
          onClick={() => setIsSecret((v) => !v)}
          className="mt-3 flex w-full items-center gap-2 rounded-md border border-line px-2.5 py-2 text-left hover:bg-panel"
        >
          <span
            className={cn(
              "flex h-4 w-4 items-center justify-center rounded border",
              isSecret ? "border-accent bg-accent text-white" : "border-line text-transparent",
            )}
          >
            <Check size={11} />
          </span>
          <span className="min-w-0 flex-1">
            <span className="block text-[12.5px] text-ink">保密项目</span>
            <span className="block text-[11px] text-ink-faint">
              归到侧边栏「保密项目」区，需密码解锁；其任务不进入日历与四象限
            </span>
          </span>
          <Lock size={13} className={cn(isSecret ? "text-accent" : "text-ink-faint")} />
        </button>

        {error && <p className="mt-2 text-[12px] text-danger">{error}</p>}

        <div className="mt-4 flex justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={() => onOpenChange(false)}>
            取消
          </Button>
          <Button variant="primary" size="sm" disabled={busy} onClick={() => void submit()}>
            {project ? "保存" : "创建"}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
