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
  /** v0.6：四象限归属（null = 未划分） */
  quadrant: number | null;
  /** v0.6：所属项目是否保密（锁定时前端据此隐藏） */
  projectIsSecret: boolean;
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
  /** v0.6：保密项目（归到侧边栏「保密项目」区） */
  isSecret: boolean;
  createdAt: string;
  updatedAt: string;
};

export type ProjectPatch = {
  name?: string;
  description?: string | null;
  color?: ProjectColor | null;
  status?: ProjectStatus;
  sortOrder?: number;
  isSecret?: boolean;
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

export type View =
  | "today"
  | "calendar"
  | "todos"
  | "inbox"
  | "project"
  | "quadrant"
  | "pomodoro"
  | "settings";

/** 四象限等级：1=重要且紧急 2=重要不紧急 3=紧急不重要 4=不重要不紧急 */
export type Quadrant = 1 | 2 | 3 | 4;

/** 保密锁状态 */
export type SecretStatus = {
  configured: boolean;
  unlocked: boolean;
  hint: string | null;
};

export type UnlockResult = {
  ok: boolean;
  attemptsLeft: number;
  hint: string | null;
};

/** 番茄钟记录 */
export type PomodoroSession = {
  id: number;
  entryId: number | null;
  kind: "focus" | "break" | string;
  durationSeconds: number;
  day: string;
  startedAt: string;
  endedAt: string;
  completed: boolean;
  createdAt: string;
  entryContent: string | null;
};

export type PomodoroSummary = {
  todayCount: number;
  todayMinutes: number;
  weekCount: number;
  weekMinutes: number;
  totalCount: number;
  totalMinutes: number;
  streakDays: number;
};

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
