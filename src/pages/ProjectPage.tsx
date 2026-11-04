import { CheckCircle2, Pencil, Plus, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { EntryCard } from "../components/EntryCard";
import { PageHeader } from "../components/PageHeader";
import { ProjectDialog } from "../components/ProjectDialog";
import { Button } from "../components/ui/Button";
import { api, errorText } from "../lib/api";
import { cn } from "../lib/cn";
import { PROJECT_STATUS_LABEL, projectHex } from "../lib/projects";
import type { Entry } from "../lib/types";
import { useProjects } from "../stores/projects";
import { useUI } from "../stores/ui";

export function ProjectPage() {
  const projectId = useUI((s) => s.selectedProjectId);
  const revision = useUI((s) => s.revision);
  const setView = useUI((s) => s.setView);
  const setSelectedProject = useUI((s) => s.setSelectedProject);
  const projects = useProjects((s) => s.items);
  const removeProject = useProjects((s) => s.remove);
  const loadProjects = useProjects((s) => s.load);

  const project = projects.find((p) => p.id === projectId) ?? null;

  const [tab, setTab] = useState<"tasks" | "notes">("tasks");
  const [items, setItems] = useState<Entry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [editing, setEditing] = useState(false);

  const reload = () => useUI.getState().bumpRevision();

  /**
   * 只有「换项目 / 换页签」才进入加载态并清空列表；
   * revision 触发的刷新必须静默替换，否则正在原地编辑的卡片会被卸载（输入框丢失焦点、
   * 编辑态被强行结束——表现为「输入时自动保存并退出编辑」）。
   */
  const queryKey = useRef<string>("");

  useEffect(() => {
    if (!projectId) return;
    const key = `${projectId}:${tab}`;
    const switched = queryKey.current !== key;
    queryKey.current = key;

    let alive = true;
    if (switched) {
      setItems([]);
      setLoading(true);
    }

    api
      .listProjectEntries(projectId, tab === "tasks")
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
  }, [projectId, tab, revision]);

  const addTask = async () => {
    const content = draft.trim();
    if (!content || !projectId) return;
    try {
      await api.createEntry(content, { isTodo: true, projectId });
      setDraft("");
      reload();
    } catch (err) {
      setError(errorText(err));
    }
  };

  const dropProject = async () => {
    if (!project) return;
    const ok = window.confirm(
      `删除项目「${project.name}」后，它下面的任务与记录会转为「无项目」，不会被删除。继续吗？`,
    );
    if (!ok) return;
    await removeProject(project.id);
    await loadProjects();
    setSelectedProject(null);
    setView("today");
  };

  if (!projectId || !project) {
    return (
      <div className="flex min-h-0 flex-1 flex-col">
        <PageHeader title="项目" subtitle="按项目组织任务与工作记录" showPanelToggle={false} />
        <div className="flex flex-1 items-center justify-center px-6 pb-16">
          <p className="rounded-xl border border-dashed border-line px-8 py-10 text-center text-[13px] text-ink-faint">
            左侧选一个项目，或点「＋」新建
          </p>
        </div>
      </div>
    );
  }

  const hex = projectHex(project.color);
  const progress =
    project.taskTotal > 0 ? Math.round((project.taskDone / project.taskTotal) * 100) : 0;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <PageHeader
        title={project.name}
        subtitle={
          <span className="flex flex-wrap items-center gap-x-2.5 gap-y-1">
            <span className="flex items-center gap-1.5">
              <span className="h-2 w-2 rounded-full" style={{ background: hex }} />
              {PROJECT_STATUS_LABEL[project.status] ?? project.status}
            </span>
            <span>任务 {project.taskTotal} · 完成 {project.taskDone}</span>
            {project.taskOverdue > 0 && <span className="text-danger">逾期 {project.taskOverdue}</span>}
            <span>记录 {project.noteTotal}</span>
            <span>完成率 {progress}%</span>
          </span>
        }
        actions={
          <>
            <Button variant="ghost" size="sm" onClick={() => setEditing(true)}>
              <Pencil size={12} /> 编辑
            </Button>
            <Button variant="danger" size="sm" onClick={() => void dropProject()}>
              <Trash2 size={12} /> 删除
            </Button>
          </>
        }
      />

      <div className="px-6">
        <div className="h-1 overflow-hidden rounded-full bg-line">
          <div className="h-full rounded-full transition-all" style={{ width: `${progress}%`, background: hex }} />
        </div>
      </div>

      <div className="flex gap-1 px-6 pb-2 pt-3">
        {(
          [
            { key: "tasks", label: `任务 ${project.taskTotal}` },
            { key: "notes", label: `记录 ${project.noteTotal}` },
          ] as const
        ).map((t) => (
          <button
            key={t.key}
            type="button"
            onClick={() => setTab(t.key)}
            className={cn(
              "h-7 rounded-md px-2.5 text-[12.5px]",
              tab === t.key
                ? "bg-accent-soft font-medium text-accent"
                : "text-ink-muted hover:bg-panel hover:text-ink",
            )}
          >
            {t.label}
          </button>
        ))}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-8">
        {error && <p className="mb-2 text-[12px] text-danger">{error}</p>}

        {tab === "tasks" && (
          <div className="mb-2 flex items-center gap-1.5 rounded-lg border border-line bg-panel px-2.5 py-1.5">
            <Plus size={13} className="shrink-0 text-ink-faint" />
            <input
              value={draft}
              placeholder="添加任务，回车确认"
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.nativeEvent.isComposing) void addTask();
              }}
              className="h-6 w-full bg-transparent text-[13px] text-ink placeholder:text-ink-faint focus:outline-none"
            />
          </div>
        )}

        {loading && items.length === 0 ? (
          <p className="py-6 text-center text-[12.5px] text-ink-faint">加载中…</p>
        ) : items.length === 0 ? (
          <p className="rounded-xl border border-dashed border-line px-6 py-12 text-center text-[13px] text-ink-faint">
            {tab === "tasks" ? "这个项目还没有任务" : "这个项目还没有记录"}
          </p>
        ) : (
          <div className="flex flex-col gap-1.5">
            {items.map((entry) => (
              <EntryCard key={entry.id} entry={entry} onChange={reload} />
            ))}
          </div>
        )}
      </div>

      <ProjectDialog open={editing} onOpenChange={setEditing} project={project} />
      {project.taskTotal > 0 && project.taskDone === project.taskTotal && (
        <div className="pointer-events-none absolute bottom-4 left-1/2 flex -translate-x-1/2 items-center gap-1.5 rounded-full bg-panel px-3 py-1.5 text-[12px] text-ink-muted shadow-lg">
          <CheckCircle2 size={13} className="text-accent" /> 这个项目的任务已全部完成
        </div>
      )}
    </div>
  );
}
