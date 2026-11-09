import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { NotificationPanel } from "../components/NotificationPanel";
import { NotificationToasts } from "../components/NotificationToasts";
import { SearchDialog } from "../components/SearchDialog";
import { SecretDialog } from "../components/SecretDialog";
import { TaskDetail } from "../components/TaskDetail";
import { TodoPanel } from "../components/TodoPanel";
import { matchShortcut } from "../lib/shortcuts";
import type { Notification, Theme } from "../lib/types";
import { CalendarPage } from "../pages/CalendarPage";
import { InboxPage } from "../pages/InboxPage";
import { PomodoroPage } from "../pages/PomodoroPage";
import { ProjectPage } from "../pages/ProjectPage";
import { QuadrantPage } from "../pages/QuadrantPage";
import { SettingsPage } from "../pages/SettingsPage";
import { TodayPage } from "../pages/TodayPage";
import { TodosPage } from "../pages/TodosPage";
import { useNotifications } from "../stores/notifications";
import { useProjects } from "../stores/projects";
import { useSettingsStore } from "../stores/settings";
import { useUI } from "../stores/ui";
import { Sidebar } from "./Sidebar";

function applyTheme(theme: Theme) {
  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const dark = theme === "dark" || (theme === "system" && prefersDark);
  document.documentElement.classList.toggle("dark", dark);
}

export default function App() {
  const view = useUI((s) => s.view);
  const theme = useUI((s) => s.theme);
  const rightPanel = useUI((s) => s.rightPanel);
  const rightPanelWidth = useUI((s) => s.rightPanelWidth);
  const selectedEntry = useUI((s) => s.selectedEntry);
  const setSearchOpen = useUI((s) => s.setSearchOpen);
  const secretDialogOpen = useUI((s) => s.secretDialogOpen);
  const setSecretDialogOpen = useUI((s) => s.setSecretDialogOpen);
  const loadSettings = useSettingsStore((s) => s.load);
  const settings = useSettingsStore((s) => s.settings);
  const revision = useUI((s) => s.revision);
  const loadProjects = useProjects((s) => s.load);

  useEffect(() => {
    void loadSettings().then(() => {
      const s = useSettingsStore.getState().settings;
      if (s) useUI.getState().setTheme(s.theme);
      void useNotifications.getState().load();
      void loadProjects();
    });
  }, [loadSettings, loadProjects]);

  // 项目列表随数据变化刷新（任务数、完成率、逾期数）
  useEffect(() => {
    void loadProjects();
  }, [revision, loadProjects]);

  // 主题：跟随设置；system 模式监听系统配色变化
  useEffect(() => {
    applyTheme(theme);
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
      if (useUI.getState().theme === "system") applyTheme("system");
    };
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [theme]);

  // 后端事件：记录变化 / 新通知
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    const unlisteners: Array<() => void> = [];
    listen("entries-changed", () => useUI.getState().bumpRevision())
      .then((fn) => unlisteners.push(fn))
      .catch(() => undefined);
    listen<Notification>("notification-created", (e) => {
      useNotifications.getState().push(e.payload);
    })
      .then((fn) => unlisteners.push(fn))
      .catch(() => undefined);
    return () => unlisteners.forEach((fn) => fn());
  }, []);

  // 窗口重新获得焦点时刷新（托盘隐藏期间的通知/记录在下一次打开时补齐）
  useEffect(() => {
    const onFocus = () => {
      useUI.getState().bumpRevision();
      void useNotifications.getState().load();
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, []);

  // 应用内快捷键（搜索键可在设置里改）
  useEffect(() => {
    const searchAccel = settings?.searchShortcut ?? "CommandOrControl+K";
    const onKey = (e: KeyboardEvent) => {
      if (e.isComposing) return;
      if (matchShortcut(searchAccel, e)) {
        e.preventDefault();
        setSearchOpen(true);
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "n" && !e.shiftKey) {
        e.preventDefault();
        const el = document.getElementById("quick-input") as HTMLTextAreaElement | null;
        el?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [settings?.searchShortcut, setSearchOpen]);

  return (
    <div className="flex h-full bg-canvas text-ink">
      <Sidebar />

      <main className="flex min-w-0 flex-1 flex-col">
        {view === "today" && <TodayPage />}
        {view === "calendar" && <CalendarPage />}
        {view === "todos" && <TodosPage />}
        {view === "inbox" && <InboxPage />}
        {view === "quadrant" && <QuadrantPage />}
        {view === "pomodoro" && <PomodoroPage />}
        {view === "project" && <ProjectPage />}
        {view === "settings" && <SettingsPage />}
      </main>

      {rightPanel && view !== "settings" && view !== "quadrant" && view !== "pomodoro" && (
        <>
          {/* 拖动改变右侧面板宽度（需求 2） */}
          <div
            role="separator"
            aria-orientation="vertical"
            title="拖动调整宽度"
            onMouseDown={(event) => {
              event.preventDefault();
              const startX = event.clientX;
              const startWidth = useUI.getState().rightPanelWidth;
              const onMove = (move: MouseEvent) => {
                useUI.getState().setRightPanelWidth(startWidth + (startX - move.clientX));
              };
              const onUp = () => {
                window.removeEventListener("mousemove", onMove);
                window.removeEventListener("mouseup", onUp);
                document.body.style.cursor = "";
              };
              document.body.style.cursor = "col-resize";
              window.addEventListener("mousemove", onMove);
              window.addEventListener("mouseup", onUp);
            }}
            className="w-1 shrink-0 cursor-col-resize bg-line/40 transition-colors hover:bg-accent/40"
          />
          <aside
            style={{ width: rightPanelWidth }}
            className="shrink-0 border-l border-line bg-panel-2"
          >
            {selectedEntry ? <TaskDetail /> : <TodoPanel />}
          </aside>
        </>
      )}

      <SearchDialog />
      <NotificationPanel />
      <NotificationToasts />
      {/* 保密密码对话框提升到应用级：侧边栏「保密项目」和新建保密项目都走这里（v0.10） */}
      <SecretDialog open={secretDialogOpen} onOpenChange={setSecretDialogOpen} />
    </div>
  );
}
