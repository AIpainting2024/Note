import { create } from "zustand";
import { api, errorText } from "../lib/api";
import type { Project, ProjectPatch } from "../lib/types";
import { useUI } from "./ui";

type State = {
  items: Project[];
  loading: boolean;
  error: string | null;

  load: () => Promise<void>;
  byId: (id: number | null | undefined) => Project | undefined;
  findByName: (name: string) => Project | undefined;
  create: (
    name: string,
    opts?: { color?: string | null; description?: string | null; isSecret?: boolean },
  ) => Promise<Project | null>;
  update: (id: number, patch: ProjectPatch) => Promise<void>;
  remove: (id: number) => Promise<void>;
};

export const useProjects = create<State>((set, get) => ({
  items: [],
  loading: false,
  error: null,

  load: async () => {
    set({ loading: true });
    try {
      const items = await api.listProjects(true);
      set({ items, loading: false, error: null });
    } catch (err) {
      set({ loading: false, error: errorText(err) });
    }
  },

  byId: (id) => (id == null ? undefined : get().items.find((p) => p.id === id)),

  findByName: (name) => {
    const target = name.trim().toLowerCase();
    if (!target) return undefined;
    const items = get().items.filter((p) => p.status !== "archived");
    const exact = items.find((p) => p.name.trim().toLowerCase() === target);
    if (exact) return exact;
    const prefix = items.filter((p) => p.name.toLowerCase().startsWith(target));
    return prefix.length === 1 ? prefix[0] : undefined;
  },

  create: async (name, opts) => {
    try {
      const project = await api.createProject(name, opts);
      await get().load();
      return project;
    } catch (err) {
      set({ error: errorText(err) });
      return null;
    }
  },

  update: async (id, patch) => {
    try {
      await api.updateProject(id, patch);
      await get().load();
      useUI.getState().bumpRevision();
    } catch (err) {
      set({ error: errorText(err) });
    }
  },

  remove: async (id) => {
    try {
      await api.deleteProject(id);
      await get().load();
      useUI.getState().bumpRevision();
    } catch (err) {
      set({ error: errorText(err) });
    }
  },
}));
