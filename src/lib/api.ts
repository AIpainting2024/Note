import { invoke } from "@tauri-apps/api/core";
import type {
  Attachment,
  BackupInfo,
  Counts,
  Entry,
  EntryPatch,
  Notification,
  PomodoroSession,
  PomodoroSummary,
  Project,
  ProjectPatch,
  SearchHit,
  SecretStatus,
  Settings,
  ShortcutKind,
  TodoGroup,
  UnlockResult,
} from "./types";

/** 所有后端调用集中在这里：页面不直接 invoke，便于统一错误处理与类型约束。 */
export const api = {
  // ---- 记录 / 任务 ----
  listEntries: (day: string) => invoke<Entry[]>("list_entries", { day }),

  createEntry: (
    content: string,
    opts?: { isTodo?: boolean; dueDate?: string | null; projectId?: number | null },
  ) =>
    invoke<Entry>("create_entry", {
      content,
      isTodo: opts?.isTodo ?? false,
      dueDate: opts?.dueDate ?? null,
      projectId: opts?.projectId ?? null,
    }),

  updateEntry: (id: number, patch: EntryPatch) => invoke<Entry>("update_entry", { id, patch }),

  setEntryCompleted: (id: number, completed: boolean) =>
    invoke<Entry>("set_entry_completed", { id, completed }),

  deleteEntry: (id: number) => invoke<void>("delete_entry", { id }),

  restoreEntry: (id: number) => invoke<Entry>("restore_entry", { id }),

  purgeEntry: (id: number) => invoke<void>("purge_entry", { id }),

  listTrash: () => invoke<Entry[]>("list_trash"),

  listTodos: (group: TodoGroup) => invoke<Entry[]>("list_todos", { group }),

  listTodosRange: (start: string, end: string) =>
    invoke<Entry[]>("list_todos_range", { start, end }),

  searchEntries: (query: string, limit?: number) =>
    invoke<SearchHit[]>("search_entries", { query, limit: limit ?? 50 }),

  getCounts: () => invoke<Counts>("get_counts"),

  listMarkedDays: (start: string, end: string) =>
    invoke<string[]>("list_marked_days", { start, end }),

  // ---- 项目（v0.4）----
  listProjects: (includeArchived = true) =>
    invoke<Project[]>("list_projects", { includeArchived }),

  createProject: (name: string, opts?: { color?: string | null; description?: string | null }) =>
    invoke<Project>("create_project", {
      name,
      color: opts?.color ?? null,
      description: opts?.description ?? null,
    }),

  updateProject: (id: number, patch: ProjectPatch) =>
    invoke<Project>("update_project", { id, patch }),

  deleteProject: (id: number) => invoke<void>("delete_project", { id }),

  restoreProject: (id: number) => invoke<Project>("restore_project", { id }),

  listProjectEntries: (projectId: number, onlyTasks: boolean) =>
    invoke<Entry[]>("list_project_entries", { projectId, onlyTasks }),

  // ---- 子任务（v0.4）----
  listChildren: (parentId: number) => invoke<Entry[]>("list_children", { parentId }),

  createChild: (parentId: number, content: string) =>
    invoke<Entry>("create_child", { parentId, content }),

  reorderChildren: (parentId: number, orderedIds: number[]) =>
    invoke<void>("reorder_children", { parentId, orderedIds }),

  // ---- 设置 ----
  getSettings: () => invoke<Settings>("get_settings"),

  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),

  setShortcut: (kind: ShortcutKind, accelerator: string) =>
    invoke<Settings>("set_shortcut", { kind, accelerator }),

  // ---- 软件内通知 ----
  notificationsList: (limit?: number) =>
    invoke<Notification[]>("notifications_list", { limit: limit ?? 50 }),

  notificationsUnread: () => invoke<number>("notifications_unread"),

  notificationMarkRead: (id: number) => invoke<void>("notification_mark_read", { id }),

  notificationDismiss: (id: number) => invoke<void>("notification_dismiss", { id }),

  notificationSnooze: (id: number, minutes: number) =>
    invoke<void>("notification_snooze", { id, minutes }),

  muteRemindersToday: () => invoke<Settings>("mute_reminders_today"),

  // ---- 数据管理 ----
  backupNow: () => invoke<BackupInfo>("backup_now"),

  listBackups: () => invoke<BackupInfo[]>("list_backups"),

  restoreBackup: (path: string) => invoke<void>("restore_backup", { path }),

  exportJson: (path: string) => invoke<void>("export_json", { path }),

  exportCsv: (path: string) => invoke<void>("export_csv", { path }),

  openDataDir: () => invoke<void>("open_data_dir"),

  // ---- 图片附件（v0.5.1）----
  /** 粘贴图片：前端把 File 读成 base64 传过来；entryId 为空表示先挂着等保存后认领 */
  saveAttachment: (entryId: number | null, mime: string, dataBase64: string) =>
    invoke<Attachment>("save_attachment", { entryId, mime, dataBase64 }),

  /** 选择本地文件上传 */
  attachFile: (entryId: number | null, filePath: string) =>
    invoke<Attachment>("attach_file", { entryId, filePath }),

  listAttachments: (entryId: number) => invoke<Attachment[]>("list_attachments", { entryId }),

  /** 保存记录后，把正文里 att:<id> 指向的图片挂到这条记录上 */
  claimAttachments: (entryId: number, content: string) =>
    invoke<number>("claim_attachments", { entryId, content }),

  deleteAttachment: (id: number) => invoke<void>("delete_attachment", { id }),

  // ---- 保密项目（v0.6）----
  secretStatus: () => invoke<SecretStatus>("secret_status"),
  setSecretPassword: (password: string, hint: string) =>
    invoke<SecretStatus>("set_secret_password", { password, hint }),
  changeSecretPassword: (oldPassword: string, newPassword: string, newHint: string) =>
    invoke<void>("change_secret_password", { oldPassword, newPassword, newHint }),
  unlockSecret: (password: string) => invoke<UnlockResult>("unlock_secret", { password }),
  lockSecret: () => invoke<void>("lock_secret"),

  // ---- 四象限（v0.6）----
  setEntryQuadrant: (id: number, quadrant: number | null) =>
    invoke<Entry>("set_entry_quadrant", { id, quadrant }),
  listQuadrantEntries: () => invoke<Entry[]>("list_quadrant_entries"),
  listUnassignedEntries: () => invoke<Entry[]>("list_unassigned_entries"),

  // ---- 番茄钟（v0.6）----
  pomodoroSave: (payload: {
    kind: string;
    durationSeconds: number;
    startedAt: string;
    endedAt: string;
    completed: boolean;
    entryId: number | null;
  }) => invoke<PomodoroSession>("pomodoro_save", payload),
  pomodoroSummary: () => invoke<PomodoroSummary>("pomodoro_summary"),
  pomodoroList: (limit?: number) => invoke<PomodoroSession[]>("pomodoro_list", { limit }),
  pomodoroClear: () => invoke<number>("pomodoro_clear"),
};

/** 统一把 invoke 抛出的错误转成可展示文案 */
export function errorText(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return "操作失败，请重试";
}
