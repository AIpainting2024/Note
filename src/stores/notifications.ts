import { create } from "zustand";
import { api, errorText } from "../lib/api";
import type { Notification } from "../lib/types";
import { useSettingsStore } from "./settings";
import { useUI } from "./ui";

type State = {
  items: Notification[];
  unread: number;
  panelOpen: boolean;
  toasts: Notification[];
  error: string | null;

  load: () => Promise<void>;
  refreshUnread: () => Promise<void>;
  setPanelOpen: (open: boolean) => void;
  markRead: (id: number) => Promise<void>;
  dismiss: (id: number) => Promise<void>;
  snooze: (id: number, minutes: number) => Promise<void>;
  complete: (n: Notification) => Promise<void>;
  muteToday: () => Promise<void>;
  /** 后端 notification-created 事件进来时调用 */
  push: (n: Notification) => void;
  dropToast: (id: number) => void;
};

export const useNotifications = create<State>((set, get) => ({
  items: [],
  unread: 0,
  panelOpen: false,
  toasts: [],
  error: null,

  load: async () => {
    try {
      const [items, unread] = await Promise.all([
        api.notificationsList(50),
        api.notificationsUnread(),
      ]);
      set({ items, unread, error: null });
    } catch (err) {
      set({ error: errorText(err) });
    }
  },

  refreshUnread: async () => {
    try {
      set({ unread: await api.notificationsUnread() });
    } catch {
      /* 静默 */
    }
  },

  setPanelOpen: (panelOpen) => set({ panelOpen }),

  markRead: async (id) => {
    try {
      await api.notificationMarkRead(id);
      set((s) => ({
        items: s.items.map((n) => (n.id === id ? { ...n, readAt: n.readAt ?? new Date().toISOString() } : n)),
      }));
      void get().refreshUnread();
    } catch (err) {
      set({ error: errorText(err) });
    }
  },

  dismiss: async (id) => {
    try {
      await api.notificationDismiss(id);
      set((s) => ({ items: s.items.filter((n) => n.id !== id), toasts: s.toasts.filter((n) => n.id !== id) }));
      void get().refreshUnread();
    } catch (err) {
      set({ error: errorText(err) });
    }
  },

  snooze: async (id, minutes) => {
    try {
      await api.notificationSnooze(id, minutes);
      set((s) => ({ items: s.items.filter((n) => n.id !== id), toasts: s.toasts.filter((n) => n.id !== id) }));
      void get().refreshUnread();
    } catch (err) {
      set({ error: errorText(err) });
    }
  },

  complete: async (n) => {
    try {
      if (n.entryId != null) {
        await api.setEntryCompleted(n.entryId, true);
        useUI.getState().bumpRevision();
      }
      await get().dismiss(n.id);
    } catch (err) {
      set({ error: errorText(err) });
    }
  },

  muteToday: async () => {
    try {
      await api.muteRemindersToday();
      await useSettingsStore.getState().load();
      set({ items: [], toasts: [] });
      void get().refreshUnread();
    } catch (err) {
      set({ error: errorText(err) });
    }
  },

  push: (n) =>
    set((s) => ({
      items: [n, ...s.items.filter((x) => x.id !== n.id)],
      unread: s.unread + 1,
      toasts: [...s.toasts, n].slice(-3),
    })),

  dropToast: (id) => set((s) => ({ toasts: s.toasts.filter((n) => n.id !== id) })),
}));
