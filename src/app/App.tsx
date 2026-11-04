import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { NotificationPanel } from "../components/NotificationPanel";
import { NotificationToasts } from "../components/NotificationToasts";
import { SearchDialog } from "../components/SearchDialog";
import { TaskDetail } from "../components/TaskDetail";
import { TodoPanel } from "../components/TodoPanel";
import { matchShortcut } from "../lib/shortcuts";
import type { Notification, Theme } from "../lib/types";
import { CalendarPage } from "../pages/CalendarPage";
import { ProjectPage } from "../pages/ProjectPage";
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
  const selectedEntry = useUI((s) => s.selectedEntry);
  const setSearchOpen = useUI((s) => s.setSearchOpen);
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
        {view === "project" && <ProjectPage />}
        {view === "settings" && <SettingsPage />}
      </main>

      {rightPanel && view !== "settings" && (
        <aside className="w-[300px] shrink-0 border-l border-line bg-panel-2">
          {selectedEntry ? <TaskDetail /> : <TodoPanel />}
        </aside>
      )}

      <SearchDialog />
      <NotificationPanel />
      <NotificationToasts />
    </div>
  );
}
