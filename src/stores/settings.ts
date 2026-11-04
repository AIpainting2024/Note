import { create } from "zustand";
import { api, errorText } from "../lib/api";
import type { Settings } from "../lib/types";

type SettingsState = {
  settings: Settings | null;
  loading: boolean;
  error: string | null;
  load: () => Promise<void>;
  save: (patch: Partial<Settings>) => Promise<void>;
};

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: null,
  loading: false,
  error: null,

  load: async () => {
    set({ loading: true, error: null });
    try {
      const settings = await api.getSettings();
      set({ settings, loading: false });
    } catch (err) {
      set({ loading: false, error: errorText(err) });
    }
  },

  save: async (patch) => {
    const current = get().settings;
    if (!current) return;
    const next = { ...current, ...patch };
    try {
      const saved = await api.saveSettings(next);
      set({ settings: saved, error: null });
    } catch (err) {
      set({ error: errorText(err) });
    }
  },
}));
