import {
  CalendarDays,
  ChevronsDownUp,
  ChevronsUpDown,
  FolderKanban,
  ListChecks,
  Plus,
  Search,
  Settings,
  Sun,
} from "lucide-react";
import { useEffect, useState } from "react";
import { ProjectDialog } from "../components/ProjectDialog";
import { NotificationBell } from "../components/NotificationBell";
import { SidebarProject } from "../components/SidebarProject";
import { cn } from "../lib/cn";
import { displayShortcut } from "../lib/shortcuts";
import type { View } from "../lib/types";
import { useProjects } from "../stores/projects";
import { useSettingsStore } from "../stores/settings";
import { useUI } from "../stores/ui";

const NAV: { view: View; label: string; icon: typeof Sun }[] = [
  { view: "today", label: "今日", icon: Sun },
  { view: "calendar", label: "日历", icon: CalendarDays },
  { view: "todos", label: "待办", icon: ListChecks },
];

const EXPANDED_KEY = "worknote.sidebar.expandedProjects";

function loadExpanded(): Record<number, boolean> {
  try {
    const raw = localStorage.getItem(EXPANDED_KEY);
    return raw ? (JSON.parse(raw) as Record<number, boolean>) : {};
  } catch {
    return {};
  }
}

export function Sidebar() {
  const view = useUI((s) => s.view);
  const setView = useUI((s) => s.setView);
  const setSearchOpen = useUI((s) => s.setSearchOpen);
  const selectedProjectId = useUI((s) => s.selectedProjectId);
  const setSelectedProject = useUI((s) => s.setSelectedProject);
  const settings = useSettingsStore((s) => s.settings);
  const projects = useProjects((s) => s.items);

  const [creating, setCreating] = useState(false);
  const [expanded, setExpanded] = useState<Record<number, boolean>>(loadExpanded);

  // 展开状态记在本机，重启应用后保持
  useEffect(() => {
    try {
      localStorage.setItem(EXPANDED_KEY, JSON.stringify(expanded));
    } catch {
      /* 忽略配额/禁用 */
    }
  }, [expanded]);

  const searchHint = settings ? displayShortcut(settings.searchShortcut) : "Ctrl + K";
  const captureHint = settings ? displayShortcut(settings.globalShortcut) : "Ctrl + Shift + N";
  const visible = projects.filter((p) => p.status !== "archived");
  const allExpanded = visible.length > 0 && visible.every((p) => expanded[p.id]);

  // 进入某个项目页时，自动展开它在侧边栏里的任务，回来就能看到
  useEffect(() => {
    if (view === "project" && selectedProjectId != null) {
      setExpanded((prev) => (prev[selectedProjectId] ? prev : { ...prev, [selectedProjectId]: true }));
    }
  }, [view, selectedProjectId]);

  const openProject = (id: number) => {
    setSelectedProject(id);
    setView("project");
  };

  const toggleAll = () => {
    if (allExpanded) {
      setExpanded({});
    } else {
      setExpanded(Object.fromEntries(visible.map((p) => [p.id, true])));
    }
  };

  return (
    <nav className="flex w-[196px] shrink-0 select-none flex-col border-r border-line bg-panel-2">
      <div className="px-4 pb-3 pt-4">
        <div className="flex items-center gap-2">
          <div className="flex h-6 w-6 items-center justify-center rounded-md bg-accent text-[12px] font-bold text-white">
            W
          </div>
          <div className="leading-tight">
            <p className="text-[13px] font-semibold text-ink">WorkNote</p>
            <p className="text-[10.5px] text-ink-faint">日常记录</p>
          </div>
        </div>
      </div>

      <button
        type="button"
        onClick={() => setSearchOpen(true)}
        className="mx-2.5 mb-2 flex h-8 items-center gap-2 rounded-md border border-line bg-panel px-2.5 text-[12.5px] text-ink-faint hover:border-ink-faint/50 hover:text-ink"
      >
        <Search size={13} />
        <span className="flex-1 text-left">搜索</span>
        <kbd className="rounded border border-line px-1 py-0.5 text-[10px]">{searchHint}</kbd>
      </button>

      <div className="flex flex-col gap-0.5 px-2.5">
        {NAV.map(({ view: item, label, icon: Icon }) => (
          <button
            key={item}
            type="button"
            onClick={() => setView(item)}
            className={cn(
              "flex h-8 items-center gap-2 rounded-md px-2.5 text-[13px] transition-colors",
              view === item
                ? "bg-accent-soft font-medium text-accent"
                : "text-ink-muted hover:bg-panel hover:text-ink",
            )}
          >
            <Icon size={15} />
            {label}
          </button>
        ))}
      </div>

      <div className="mt-3 flex items-center gap-1 pl-3 pr-2 pb-1">
        <span className="flex flex-1 items-center gap-1 text-[11px] font-medium tracking-wide text-ink-faint">
          <FolderKanban size={11} /> 项目
        </span>
        {visible.length > 0 && (
          <button
            type="button"
            aria-label={allExpanded ? "全部收起" : "全部展开"}
            title={allExpanded ? "全部收起" : "全部展开"}
            onClick={toggleAll}
            className="rounded p-0.5 text-ink-faint hover:bg-panel hover:text-ink"
          >
            {allExpanded ? <ChevronsDownUp size={12} /> : <ChevronsUpDown size={12} />}
          </button>
        )}
        <button
          type="button"
          aria-label="新建项目"
          title="新建项目"
          onClick={() => setCreating(true)}
          className="rounded p-0.5 text-ink-faint hover:bg-panel hover:text-accent"
        >
          <Plus size={13} />
        </button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-2.5 pb-2">
        {visible.length === 0 ? (
          <p className="px-2 py-1 text-[11px] leading-relaxed text-ink-faint">还没有项目</p>
        ) : (
          visible.map((project) => (
            <SidebarProject
              key={project.id}
              project={project}
              active={view === "project" && selectedProjectId === project.id}
              expanded={Boolean(expanded[project.id])}
              onToggle={() =>
                setExpanded((prev) => ({ ...prev, [project.id]: !prev[project.id] }))
              }
              onOpenProject={() => openProject(project.id)}
            />
          ))
        )}
      </div>

      <div className="px-2.5 pb-1">
        <NotificationBell />
      </div>

      <div className="px-2.5 pb-3">
        <button
          type="button"
          onClick={() => setView("settings")}
          className={cn(
            "flex h-8 w-full items-center gap-2 rounded-md px-2.5 text-[13px] transition-colors",
            view === "settings"
              ? "bg-accent-soft font-medium text-accent"
              : "text-ink-muted hover:bg-panel hover:text-ink",
          )}
        >
          <Settings size={15} />
          设置
        </button>
        <p className="mt-2 px-2.5 text-[10.5px] leading-relaxed text-ink-faint">
          {captureHint} 随时记一笔
        </p>
      </div>

      <ProjectDialog open={creating} onOpenChange={setCreating} />
    </nav>
  );
}
