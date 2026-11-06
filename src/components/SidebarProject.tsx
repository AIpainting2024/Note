import { ChevronDown, ChevronRight, Clock, ListChecks } from "lucide-react";
import { useEffect, useState } from "react";
import { isOverdueRed } from "../lib/entries";
import { api, errorText } from "../lib/api";
import { cn } from "../lib/cn";
import { dueLabel } from "../lib/dates";
import { projectHex } from "../lib/projects";
import type { Entry, Project } from "../lib/types";
import { useUI } from "../stores/ui";
import { Checkbox } from "./ui/Checkbox";

const LIMIT = 40;

type Props = {
  project: Project;
  active: boolean;
  expanded: boolean;
  onToggle: () => void;
  onOpenProject: () => void;
};

/** 侧边栏里的一个项目：可展开看它下面的任务，任务可就地勾选完成 / 点开详情 */
export function SidebarProject({ project, active, expanded, onToggle, onOpenProject }: Props) {
  const revision = useUI((s) => s.revision);
  const setSelectedEntry = useUI((s) => s.setSelectedEntry);
  const [tasks, setTasks] = useState<Entry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!expanded) return;
    let alive = true;
    setLoading(true);
    api
      .listProjectEntries(project.id, true)
      .then((list) => {
        if (!alive) return;
        setTasks(list);
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
  }, [expanded, project.id, revision]);

  const complete = async (entry: Entry, checked: boolean) => {
    try {
      await api.setEntryCompleted(entry.id, checked);
      useUI.getState().bumpRevision();
    } catch (err) {
      setError(errorText(err));
    }
  };

  const pending = tasks.filter((t) => !t.completedAt).length;

  return (
    <div>
      <div
        className={cn(
          "group flex h-8 items-center rounded-md pr-1 transition-colors",
          active ? "bg-accent-soft text-accent" : "text-ink-muted hover:bg-panel hover:text-ink",
        )}
      >
        <button
          type="button"
          aria-label={expanded ? "收起项目任务" : "展开项目任务"}
          title={expanded ? "收起" : "展开"}
          onClick={onToggle}
          className="ml-0.5 shrink-0 rounded p-1 text-ink-faint hover:text-ink"
        >
          {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        </button>

        <button
          type="button"
          onClick={onOpenProject}
          className="flex min-w-0 flex-1 items-center gap-1.5 py-1 pr-1 text-left"
          title={`${project.name} · 任务 ${project.taskDone}/${project.taskTotal}`}
        >
          <span className="h-2 w-2 shrink-0 rounded-full" style={{ background: projectHex(project.color) }} />
          <span
            className={cn(
              "min-w-0 flex-1 truncate text-[12.5px]",
              active && "font-medium",
            )}
          >
            {project.name}
          </span>
          {project.taskOverdue > 0 && (
            <span className="shrink-0 text-[10.5px] text-danger">{project.taskOverdue}</span>
          )}
          <span className="shrink-0 text-[10.5px] text-ink-faint">
            {project.taskTotal > 0 ? `${project.taskDone}/${project.taskTotal}` : ""}
          </span>
        </button>
      </div>

      {expanded && (
        <div className="ml-[13px] border-l border-line pl-1.5 pb-1">
          {loading && tasks.length === 0 && (
            <p className="px-1.5 py-1 text-[10.5px] text-ink-faint">加载中…</p>
          )}
          {error && <p className="px-1.5 py-1 text-[10.5px] text-danger">{error}</p>}
          {!loading && tasks.length === 0 && !error && (
            <p className="px-1.5 py-1 text-[10.5px] text-ink-faint">还没有任务</p>
          )}

          {tasks.slice(0, LIMIT).map((task) => {
            const done = Boolean(task.completedAt);
            const due = task.dueDate ? dueLabel(task.dueDate, task.dueTime) : null;
            const childTotal = task.childTotal ?? 0;
            return (
              <div
                key={task.id}
                className="group flex items-start gap-1.5 rounded px-1 py-1 hover:bg-panel"
              >
                <div className="mt-[3px]">
                  <Checkbox
                    checked={done}
                    size="sm"
                    ariaLabel={done ? "标记为未完成" : "标记为完成"}
                    onCheckedChange={(v) => void complete(task, v)}
                  />
                </div>
                <button
                  type="button"
                  onClick={() => {
                    // 右侧详情 + 中间内容区一起联动（v0.7 需求 3/4）：
                    // 有项目就切到该项目的项目页，没有项目就切到收件箱。
                    const targetProject = task.projectId ?? project.id ?? null;
                    setSelectedEntry(task);
                    useUI.getState().setAddProjectId(targetProject);
                    useUI.getState().setSelectedProject(targetProject);
                    useUI.getState().setView(targetProject == null ? "inbox" : "project");
                  }}
                  className="min-w-0 flex-1 text-left"
                  title={task.content}
                >
                  <span
                    className={cn(
                      "block truncate text-[11.5px] leading-snug",
                      done ? "text-ink-faint line-through" : "text-ink",
                    )}
                  >
                    {task.content}
                  </span>
                  {(due || childTotal > 0) && (
                    <span className="mt-0.5 flex items-center gap-1.5 text-[10px] text-ink-faint">
                      {due && (
                        <span
                          className={cn(
                            "flex items-center gap-0.5",
                            isOverdueRed(task, due.tone) && "text-danger",
                            due.tone === "today" && "text-warn",
                          )}
                        >
                          <Clock size={9} /> {due.text}
                        </span>
                      )}
                      {childTotal > 0 && (
                        <span className="flex items-center gap-0.5">
                          <ListChecks size={9} /> {task.childDone ?? 0}/{childTotal}
                        </span>
                      )}
                    </span>
                  )}
                </button>
              </div>
            );
          })}

          {tasks.length > LIMIT && (
            <p className="px-1.5 py-1 text-[10.5px] text-ink-faint">
              还有 {tasks.length - LIMIT} 项，进入项目页查看
            </p>
          )}

          {tasks.length > 0 && pending === 0 && (
            <p className="px-1.5 py-1 text-[10.5px] text-accent">全部完成</p>
          )}
        </div>
      )}
    </div>
  );
}
