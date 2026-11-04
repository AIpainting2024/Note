/** 与 Rust 侧 serde(rename_all = "camelCase") 一一对应，改动必须同步 docs/接口契约*.md */

export type Entry = {
  id: number;
  day: string; // YYYY-MM-DD
  content: string;
  isTodo: boolean;
  dueDate: string | null; // YYYY-MM-DD
  dueTime: string | null; // HH:MM
  completedAt: string | null; // YYYY-MM-DDTHH:MM:SS
  isPinned: boolean;
  isDeleted: boolean;
  /** v0.4：归属项目 */
  projectId: number | null;
  /** v0.4：父任务（子任务才有） */
  parentId: number | null;
  projectName: string | null;
  projectColor: string | null;
  /** v0.4：父任务的子任务进度 */
  childTotal: number | null;
  childDone: number | null;
  createdAt: string;
  updatedAt: string;
};

export type EntryPatch = {
  content?: string;
  isTodo?: boolean;
  dueDate?: string | null;
  dueTime?: string | null;
  isPinned?: boolean;
  projectId?: number | null;
};

export type SearchHit = {
  entry: Entry;
  /** 命中位置用 \u0001 \u0002 包裹 */
  snippet: string;
  /** v0.4：命中子任务时为父任务标题 */
  parentContent: string | null;
};

export type Counts = {
  todayEntries: number;
  todayTodos: number;
  pendingTodos: number;
  dueTodayTodos: number;
  overdueTodos: number;
  doneToday: number;
};

export type ProjectColor = "indigo" | "teal" | "amber" | "rose" | "violet" | "slate";

export type ProjectStatus = "active" | "paused" | "done" | "archived";

export type Project = {
  id: number;
  name: string;
  description: string | null;
  color: ProjectColor | null;
  status: ProjectStatus;
  sortOrder: number;
  taskTotal: number;
  taskDone: number;
  taskOverdue: number;
  noteTotal: number;
  createdAt: string;
  updatedAt: string;
};

export type ProjectPatch = {
  name?: string;
  description?: string | null;
  color?: ProjectColor | null;
  status?: ProjectStatus;
  sortOrder?: number;
};

export type Settings = {
  theme: Theme;
  autoBackup: boolean;
  autoBackupKeep: number;
  globalShortcut: string;
  mainShortcut: string;
  searchShortcut: string;
  remindBeforeMinutes: number;
  dailyDigestEnabled: boolean;
  dailyDigestTime: string;
  mutedUntil: string | null;
  dataDir: string;
  lastBackupAt: string | null;
};

export type BackupInfo = {
  path: string;
  sizeBytes: number;
  createdAt: string;
};

export type NotificationKind = "due_soon" | "daily_digest";

export type Notification = {
  id: number;
  kind: NotificationKind;
  entryId: number | null;
  title: string;
  body: string;
  scheduledAt: string;
  emittedAt: string | null;
  readAt: string | null;
  dismissedAt: string | null;
  createdAt: string;
};

export type ShortcutKind = "capture" | "main";

export type TodoGroup = "pending" | "dueToday" | "overdue" | "done";

export type View = "today" | "calendar" | "todos" | "project" | "settings";

export type Theme = "light" | "dark" | "system";

/** 日历视图模式 */
export type CalendarMode = "month" | "week" | "day";

/** 图片附件：正文里以 `att:<id>` 引用，文件存在数据目录 attachments/ 下 */
export type Attachment = {
  id: number;
  entryId: number | null;
  mime: string;
  byteSize: number;
  /** 绝对路径，前端用 convertFileSrc 转成可显示的地址 */
  path: string;
  createdAt: string;
};
