import {
  CalendarDays,
  ChevronsDownUp,
  ChevronsUpDown,
  FolderKanban,
  Grid2x2,
  StickyNote,
  ListChecks,
  Lock,
  LockOpen,
  Plus,
  Search,
  Settings,
  Sun,
  Timer,
} from "lucide-react";
import { useEffect, useState } from "react";
import { NotificationBell } from "../components/NotificationBell";
import { ProjectDialog } from "../components/ProjectDialog";
import { SidebarProject } from "../components/SidebarProject";
import { cn } from "../lib/cn";
import { displayShortcut } from "../lib/shortcuts";
import type { View } from "../lib/types";
import { useProjects } from "../stores/projects";
import { useSecret } from "../stores/secret";
import { useSettingsStore } from "../stores/settings";
import { useUI } from "../stores/ui";

const NAV: { view: View; label: string; icon: typeof Sun }[] = [
  { view: "today", label: "今日", icon: Sun },
  { view: "calendar", label: "日历", icon: CalendarDays },
  { view: "todos", label: "待办", icon: ListChecks },
  { view: "inbox", label: "便签贴", icon: StickyNote },
  { view: "quadrant", label: "四象限", icon: Grid2x2 },
  { view: "pomodoro", label: "番茄钟", icon: Timer },
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
  const pomodoroDone = useUI((s) => s.pomodoroDone);
  const setView = useUI((s) => s.setView);
  const setSearchOpen = useUI((s) => s.setSearchOpen);
  const selectedProjectId = useUI((s) => s.selectedProjectId);
  const setSelectedProject = useUI((s) => s.setSelectedProject);
  const settings = useSettingsStore((s) => s.settings);
  const projects = useProjects((s) => s.items);
  const secretStatus = useSecret((s) => s.status);
  const loadSecret = useSecret((s) => s.load);
  const lockSecret = useSecret((s) => s.lock);

  const [creating, setCreating] = useState(false);
  // 保密对话框提升到 App 级，这里只负责打开它（v0.10）
  const setSecretOpen = useUI((s) => s.setSecretDialogOpen);
  const [expanded, setExpanded] = useState<Record<number, boolean>>(loadExpanded);

  const searchHint = settings ? displayShortcut(settings.searchShortcut) : "Ctrl + K";
  const captureHint = settings ? displayShortcut(settings.globalShortcut) : "Ctrl + Shift + N";

  useEffect(() => {
    void loadSecret();
  }, [loadSecret]);

  // 展开状态记在本机，重启应用后保持
  useEffect(() => {
    try {
      localStorage.setItem(EXPANDED_KEY, JSON.stringify(expanded));
    } catch {
      /* 忽略配额/禁用 */
    }
  }, [expanded]);

  const active = projects.filter((p) => p.status !== "archived");
  const publicProjects = active.filter((p) => !p.isSecret);
  const secretProjects = active.filter((p) => p.isSecret);
  const secretVisible = secretStatus.configured && secretStatus.unlocked;

  const visibleForToggle = secretVisible ? active : publicProjects;
  const allExpanded =
    visibleForToggle.length > 0 && visibleForToggle.every((p) => expanded[p.id]);

  // 进入某个项目页时，自动展开它在侧边栏里的任务
  useEffect(() => {
    if (view === "project" && selectedProjectId != null) {
      setExpanded((prev) => (prev[selectedProjectId] ? prev : { ...prev, [selectedProjectId]: true }));
    }
  }, [view, selectedProjectId]);

  const openProject = (id: number) => {
    setSelectedProject(id);
    setAddProject(id);
    setView("project");
  };

  function setAddProject(id: number | null) {
    useUI.getState().setAddProjectId(id);
  }

  const toggleAll = () => {
    if (allExpanded) {
      setExpanded({});
    } else {
      setExpanded(Object.fromEntries(visibleForToggle.map((p) => [p.id, true])));
    }
  };

  return (
    <nav className="flex w-[196px] shrink-0 select-none flex-col border-r border-sidebar-line bg-sidebar">
      <div className="px-4 pb-2.5 pt-2">
        <div className="flex items-center gap-2">
          <img src="/logo.png" alt="青苍笔记" className="h-7 w-7 rounded-md" draggable={false} />
          <div className="leading-tight">
            <p className="text-[13px] font-semibold text-sidebar-ink">青苍笔记</p>
            <p className="text-[10.5px] text-sidebar-ink-faint">日常记录</p>
          </div>
        </div>
      </div>

      <button
        type="button"
        onClick={() => setSearchOpen(true)}
        className="mx-2.5 mb-2 flex h-8 items-center gap-2 rounded-md border border-sidebar-line bg-sidebar-hover/60 px-2.5 text-[12.5px] text-sidebar-ink-muted hover:border-sidebar-ink-faint/50 hover:text-sidebar-ink"
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
                : "text-sidebar-ink-muted hover:bg-sidebar-hover hover:text-sidebar-ink",
            )}
          >
            <Icon size={15} />
            {label}
            {item === "pomodoro" && pomodoroDone && (
              <span
                aria-label="番茄钟有未查看的完成提示"
                className="ml-auto h-1.5 w-1.5 rounded-full bg-danger"
              />
            )}
          </button>
        ))}
      </div>

      <div className="mt-3 flex items-center gap-1 pl-3 pr-2 pb-1">
        <span className="flex flex-1 items-center gap-1 text-[11px] font-medium tracking-wide text-sidebar-ink-faint">
          <FolderKanban size={11} /> 项目
        </span>
        {visibleForToggle.length > 0 && (
          <button
            type="button"
            aria-label={allExpanded ? "全部收起" : "全部展开"}
            title={allExpanded ? "全部收起" : "全部展开"}
            onClick={toggleAll}
            className="rounded p-0.5 text-sidebar-ink-faint hover:bg-sidebar-hover hover:text-sidebar-ink"
          >
            {allExpanded ? <ChevronsDownUp size={12} /> : <ChevronsUpDown size={12} />}
          </button>
        )}
        <button
          type="button"
          aria-label="新建项目"
          title="新建项目"
          onClick={() => setCreating(true)}
          className="rounded p-0.5 text-sidebar-ink-faint hover:bg-sidebar-hover hover:text-accent"
        >
          <Plus size={13} />
        </button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-2.5 pb-2">
        {publicProjects.length === 0 ? (
          <p className="px-2 py-1 text-[11px] leading-relaxed text-sidebar-ink-faint">还没有项目</p>
        ) : (
          publicProjects.map((project) => (
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

        {/* 保密项目区 */}
        <div className="mt-3 flex items-center gap-1 pl-0.5 pr-1 pb-1">
          <span className="flex flex-1 items-center gap-1 text-[11px] font-medium tracking-wide text-sidebar-ink-faint">
            {secretVisible ? <LockOpen size={11} /> : <Lock size={11} />} 保密项目
          </span>
          {secretVisible && (
            <button
              type="button"
              title="重新上锁"
              aria-label="重新上锁"
              onClick={() => void lockSecret()}
              className="rounded p-0.5 text-sidebar-ink-faint hover:bg-sidebar-hover hover:text-sidebar-ink"
            >
              <Lock size={12} />
            </button>
          )}
        </div>

        {!secretVisible ? (
          <button
            type="button"
            onClick={() => setSecretOpen(true)}
            className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[12px] text-sidebar-ink-muted hover:bg-sidebar-hover hover:text-sidebar-ink"
          >
            <Lock size={12} />
            {secretStatus.configured ? "已锁定 · 点此输入密码" : "设置密码并启用"}
          </button>
        ) : secretProjects.length === 0 ? (
          <p className="px-2 py-1 text-[11px] text-sidebar-ink-faint">
            还没有保密项目（新建项目时勾选「保密」）
          </p>
        ) : (
          secretProjects.map((project) => (
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
              : "text-sidebar-ink-muted hover:bg-sidebar-hover hover:text-sidebar-ink",
          )}
        >
          <Settings size={15} />
          设置
        </button>
        <p className="mt-2 px-2.5 text-[10.5px] leading-relaxed text-sidebar-ink-faint">
          {captureHint} 随时记一笔
        </p>
      </div>

      <ProjectDialog open={creating} onOpenChange={setCreating} />
    </nav>
  );
}
