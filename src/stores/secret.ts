import { create } from "zustand";
import { api } from "../lib/api";
import type { SecretStatus } from "../lib/types";
import { useUI } from "./ui";

type SecretStore = {
  status: SecretStatus;
  loading: boolean;
  /** 最近一次解锁失败的结果（含提示） */
  lastError: string | null;
  hint: string | null;

  load: () => Promise<void>;
  unlock: (password: string) => Promise<{ ok: boolean; attemptsLeft: number }>;
  lock: () => Promise<void>;
  setPassword: (password: string, hint: string) => Promise<void>;
  changePassword: (oldPassword: string, newPassword: string, newHint: string) => Promise<void>;
};

const initial: SecretStatus = { configured: false, unlocked: false, hint: null };

export const useSecret = create<SecretStore>((set, get) => ({
  status: initial,
  loading: false,
  lastError: null,
  hint: null,

  load: async () => {
    set({ loading: true });
    try {
      const status = await api.secretStatus();
      set({ status, loading: false });
    } catch {
      set({ loading: false });
    }
  },

  unlock: async (password) => {
    const result = await api.unlockSecret(password);
    set({
      status: { ...get().status, unlocked: result.ok },
      lastError: result.ok ? null : `密码不对，还剩 ${result.attemptsLeft} 次机会`,
      hint: result.hint,
    });
    if (result.ok) {
      // 解锁后列表里要能看到保密项目
      useUI.getState().bumpRevision();
    }
    return { ok: result.ok, attemptsLeft: result.attemptsLeft };
  },

  lock: async () => {
    await api.lockSecret();
    set({ status: { ...get().status, unlocked: false }, hint: null, lastError: null });
    useUI.getState().bumpRevision();
  },

  setPassword: async (password, hint) => {
    const status = await api.setSecretPassword(password, hint);
    set({ status, hint: null, lastError: null });
    useUI.getState().bumpRevision();
  },

  changePassword: async (oldPassword, newPassword, newHint) => {
    await api.changeSecretPassword(oldPassword, newPassword, newHint);
  },
}));
