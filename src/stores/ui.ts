import { create } from "zustand";
import { todayStr } from "../lib/dates";
import type { CalendarMode, Entry, Theme, View } from "../lib/types";

type UIState = {
  view: View;
  setView: (view: View) => void;

  selectedDay: string;
  setSelectedDay: (day: string) => void;

  calendarMode: CalendarMode;
  setCalendarMode: (mode: CalendarMode) => void;

  /** 当前打开的项目页 */
  selectedProjectId: number | null;
  setSelectedProject: (id: number | null) => void;

  /** 右侧任务详情面板：直接存对象，避免额外 get_entry 命令 */
  selectedEntry: Entry | null;
  setSelectedEntry: (entry: Entry | null) => void;

  searchOpen: boolean;
  setSearchOpen: (open: boolean) => void;

  theme: Theme;
  setTheme: (theme: Theme) => void;

  rightPanel: boolean;
  toggleRightPanel: () => void;

  /** 右侧面板宽度（可拖动，记住上次的位置） */
  rightPanelWidth: number;
  setRightPanelWidth: (width: number) => void;

  /** 项目页「添加任务」当前的归属项目（点侧边栏任务时同步切换） */
  addProjectId: number | null;
  setAddProjectId: (id: number | null) => void;

  /** 保密对话框：新建保密项目后自动弹出引导设密码/解锁（v0.10） */
  secretDialogOpen: boolean;
  setSecretDialogOpen: (open: boolean) => void;

  /** 数据变更计数器：任何写入后自增，页面据此重新拉取 */
  revision: number;
  bumpRevision: () => void;
};

export const useUI = create<UIState>((set) => ({
  view: "today",
  setView: (view) => set({ view }),

  selectedDay: todayStr(),
  setSelectedDay: (selectedDay) => set({ selectedDay }),

  calendarMode: "month",
  setCalendarMode: (calendarMode) => set({ calendarMode }),

  selectedProjectId: null,
  setSelectedProject: (selectedProjectId) => set({ selectedProjectId }),

  selectedEntry: null,
  setSelectedEntry: (selectedEntry) => set({ selectedEntry }),

  searchOpen: false,
  setSearchOpen: (searchOpen) => set({ searchOpen }),

  theme: "system",
  setTheme: (theme) => set({ theme }),

  rightPanel: true,
  toggleRightPanel: () => set((s) => ({ rightPanel: !s.rightPanel })),

  rightPanelWidth: (() => {
    const saved = Number(localStorage.getItem("worknote.rightPanelWidth"));
    return Number.isFinite(saved) && saved >= 260 && saved <= 720 ? saved : 300;
  })(),
  setRightPanelWidth: (rightPanelWidth) => {
    const width = Math.min(720, Math.max(260, Math.round(rightPanelWidth)));
    localStorage.setItem("worknote.rightPanelWidth", String(width));
    set({ rightPanelWidth: width });
  },

  addProjectId: null,
  setAddProjectId: (addProjectId) => set({ addProjectId }),

  secretDialogOpen: false,
  setSecretDialogOpen: (secretDialogOpen) => set({ secretDialogOpen }),

  revision: 0,
  bumpRevision: () => set((s) => ({ revision: s.revision + 1 })),
}));
