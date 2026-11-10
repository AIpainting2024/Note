//! 与前端共享的数据结构。
//!
//! 全部结构与 `docs/接口契约-v1.md` 第 3 节、`docs/接口契约-v0.3.md` 第 2/3 节、
//! `docs/接口契约-v0.4.md` 第 2 节一一对应，序列化统一使用 camelCase，
//! 字段名改动必须先改契约文件。

use serde::{Deserialize, Deserializer, Serialize};

/// 一条记录（待办只是记录的一个属性）。
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: i64,
    /// 本地日期 YYYY-MM-DD
    pub day: String,
    pub content: String,
    pub is_todo: bool,
    /// YYYY-MM-DD，可空
    pub due_date: Option<String>,
    /// HH:MM（24 小时制），可空；为空表示只有日期没有具体时间
    pub due_time: Option<String>,
    /// YYYY-MM-DDTHH:MM:SS 本地，可空
    pub completed_at: Option<String>,
    pub is_pinned: bool,
    pub is_deleted: bool,
    pub created_at: String,
    pub updated_at: String,
    /// v0.4：归属项目；NULL = 无项目（收件箱）。
    #[sqlx(default)]
    pub project_id: Option<i64>,
    /// v0.4：父任务；只有子任务非空（禁止三级嵌套，父任务自身 `parent_id` 必为 NULL）。
    #[sqlx(default)]
    pub parent_id: Option<i64>,
    /// v0.4：列表 JOIN 带出，前端不再二次查询（无项目时为 None）。
    #[sqlx(default)]
    pub project_name: Option<String>,
    /// v0.4：同上（项目色板 key）。
    #[sqlx(default)]
    pub project_color: Option<String>,
    /// v0.4：父任务的子任务总数；没有子任务时为 None（= 前端不显示进度）。
    #[sqlx(default)]
    pub child_total: Option<i64>,
    /// v0.4：父任务已完成的子任务数；没有子任务时为 None，有子任务时可能为 0。
    #[sqlx(default)]
    pub child_done: Option<i64>,
    /// v0.6：四象限归属（1=重要且紧急 2=重要不紧急 3=紧急不重要 4=不重要不紧急）；None=未划分
    #[sqlx(default)]
    pub quadrant: Option<i64>,
    /// v0.6：所属项目是否保密（列表 JOIN 带出；无项目时 false）。
    /// 锁定时前端据此把这些记录从所有列表剔除。
    #[sqlx(default)]
    pub project_is_secret: bool,
}

/// 记录补丁：字段缺省 = 不改；`due_date` / `due_time` / `project_id` 显式 null = 清空。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EntryPatch {
    pub content: Option<String>,
    pub is_todo: Option<bool>,
    /// 外层 None = 前端没传该字段（不改）；Some(None) = 显式 null（清空）。
    #[serde(default, deserialize_with = "deserialize_explicit_option")]
    pub due_date: Option<Option<String>>,
    /// 外层 None = 前端没传该字段（不改）；Some(None) = 显式 null（清空）。
    #[serde(default, deserialize_with = "deserialize_explicit_option")]
    pub due_time: Option<Option<String>>,
    pub is_pinned: Option<bool>,
    /// v0.4：归属项目。外层 None = 不改；Some(None) = 清空为无项目。
    /// 契约明确：**不允许**通过 patch 修改 `parent_id`（层级只能走 create_child）。
    #[serde(default, deserialize_with = "deserialize_explicit_option")]
    pub project_id: Option<Option<i64>>,
}

/// 项目（v0.4）。项目是最高层，不支持嵌套。
///
/// 统计字段在列表/单条查询时一并算出，前端直接用，口径见契约第 5 节：
/// `task_*` 只算顶层未删除任务，`note_total` 只算非待办记录。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    /// 色板 key：indigo|teal|amber|rose|violet|slate，可空。
    pub color: Option<String>,
    /// active|paused|done|archived
    pub status: String,
    pub sort_order: i64,
    /// 该项目下顶层、未删除任务数（is_todo=1）。
    pub task_total: i64,
    /// 其中已完成。
    pub task_done: i64,
    /// 其中已逾期未完成。
    pub task_overdue: i64,
    /// 该项目下非待办记录数（is_todo=0）。
    pub note_total: i64,
    /// v0.6：保密项目（侧边栏归到「保密项目」区，需密码解锁才可见）
    #[sqlx(default)]
    pub is_secret: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// 项目补丁：字段缺省 = 不改；`description` / `color` 显式 null = 清空。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectPatch {
    pub name: Option<String>,
    /// v0.6：切换保密项目
    #[serde(default)]
    pub is_secret: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_explicit_option")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_explicit_option")]
    pub color: Option<Option<String>>,
    pub status: Option<String>,
    pub sort_order: Option<i64>,
}

/// 项目状态取值（契约第 1 节 DDL 注释 / 第 3 节状态校验）。
pub const PROJECT_STATUSES: [&str; 4] = ["active", "paused", "done", "archived"];
/// 项目色板固定 6 色（契约第 6 节）。
pub const PROJECT_COLORS: [&str; 6] = ["indigo", "teal", "amber", "rose", "violet", "slate"];

/// 把 `Option<Option<T>>` 的“缺省”和“显式 null”区分开。
///
/// serde 对 `Option<Option<T>>` 的默认行为会把 JSON `null` 和字段缺失都当成 `None`，
/// 这里用 `deserialize_with` 保证：字段缺失走 `#[serde(default)]`（外层 None），
/// 字段存在则一定得到一个 `Some(...)`（内层 None 表示显式 null）。
fn deserialize_explicit_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// 搜索结果：`snippet` 中命中片段用 `\u{1}` / `\u{2}` 包裹。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub entry: Entry,
    /// v0.4：命中子任务时为父任务标题（前端显示「父任务：xxx」）；顶层记录为 None。
    pub parent_content: Option<String>,
    pub snippet: String,
}

/// 首页/侧栏统计。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub today_entries: i64,
    pub today_todos: i64,
    pub pending_todos: i64,
    pub due_today_todos: i64,
    pub overdue_todos: i64,
    pub done_today: i64,
}

/// 软件内通知（契约 v0.3 第 3 节）。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: i64,
    /// due_soon | daily_digest
    pub kind: String,
    /// 关联待办；汇总类为 NULL
    pub entry_id: Option<i64>,
    pub title: String,
    pub body: String,
    /// 计划发出时间 YYYY-MM-DDTHH:MM:SS（snooze 时后移）
    pub scheduled_at: String,
    /// 实际已推送给前端的时刻；NULL = 尚未发出
    pub emitted_at: Option<String>,
    pub read_at: Option<String>,
    pub dismissed_at: Option<String>,
    pub created_at: String,
}

pub const DEFAULT_THEME: &str = "system";
/// 侧边栏配色（v0.11）：default / bamboo(竹青) / paper(宣纸) / moon(月白) / indigo(黛蓝) / lotus(藕荷) / tea(茶褐)
pub const DEFAULT_SIDEBAR_THEME: &str = "default";
pub const DEFAULT_AUTO_BACKUP: bool = true;
pub const DEFAULT_AUTO_BACKUP_KEEP: i64 = 14;
/// 全局快速记录快捷键（v0.3 起 = capture）。
pub const DEFAULT_GLOBAL_SHORTCUT: &str = "CommandOrControl+Shift+N";
/// 全局显示主界面快捷键（v0.3 新增 = main）。
pub const DEFAULT_MAIN_SHORTCUT: &str = "CommandOrControl+Shift+M";
/// 应用内搜索快捷键（纯前端，后端只负责存储）。
pub const DEFAULT_SEARCH_SHORTCUT: &str = "CommandOrControl+K";
/// 截止前提醒分钟数；0 = 关闭。
pub const DEFAULT_REMIND_BEFORE_MINUTES: i64 = 30;
pub const DEFAULT_DAILY_DIGEST_ENABLED: bool = true;
pub const DEFAULT_DAILY_DIGEST_TIME: &str = "09:30";

fn default_theme() -> String {
    DEFAULT_THEME.to_string()
}

fn default_sidebar_theme() -> String {
    DEFAULT_SIDEBAR_THEME.to_string()
}

fn default_auto_backup() -> bool {
    DEFAULT_AUTO_BACKUP
}

fn default_auto_backup_keep() -> i64 {
    DEFAULT_AUTO_BACKUP_KEEP
}

fn default_global_shortcut() -> String {
    DEFAULT_GLOBAL_SHORTCUT.to_string()
}

fn default_main_shortcut() -> String {
    DEFAULT_MAIN_SHORTCUT.to_string()
}

fn default_search_shortcut() -> String {
    DEFAULT_SEARCH_SHORTCUT.to_string()
}

fn default_remind_before_minutes() -> i64 {
    DEFAULT_REMIND_BEFORE_MINUTES
}

fn default_daily_digest_enabled() -> bool {
    DEFAULT_DAILY_DIGEST_ENABLED
}

fn default_daily_digest_time() -> String {
    DEFAULT_DAILY_DIGEST_TIME.to_string()
}

/// 应用设置；`data_dir` 只读，由后端填写。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_theme")]
    pub theme: String,
    /// v0.11 侧边栏配色方案
    #[serde(default = "default_sidebar_theme")]
    pub sidebar_theme: String,
    #[serde(default = "default_auto_backup")]
    pub auto_backup: bool,
    #[serde(default = "default_auto_backup_keep")]
    pub auto_backup_keep: i64,
    #[serde(default = "default_global_shortcut")]
    pub global_shortcut: String,
    #[serde(default = "default_main_shortcut")]
    pub main_shortcut: String,
    #[serde(default = "default_search_shortcut")]
    pub search_shortcut: String,
    #[serde(default = "default_remind_before_minutes")]
    pub remind_before_minutes: i64,
    #[serde(default = "default_daily_digest_enabled")]
    pub daily_digest_enabled: bool,
    #[serde(default = "default_daily_digest_time")]
    pub daily_digest_time: String,
    /// 静音截止时刻；> now 时不发任何提醒。
    #[serde(default)]
    pub muted_until: Option<String>,
    #[serde(default)]
    pub data_dir: String,
    #[serde(default)]
    pub last_backup_at: Option<String>,
}

/// 备份文件信息。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: String,
    pub size_bytes: i64,
    pub created_at: String,
}

/// 保密锁状态（v0.6）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretStatus {
    /// 是否已经设过密码
    pub configured: bool,
    /// 本次进程内是否已解锁
    pub unlocked: bool,
    /// 连续输错达到 3 次后，解锁结果里会带上提示
    pub hint: Option<String>,
}

/// 解锁结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockResult {
    pub ok: bool,
    /// 还剩几次机会（未解锁时有效）
    pub attempts_left: i64,
    /// 输错满 3 次才返回密码提示
    pub hint: Option<String>,
}

/// 番茄钟一次专注/休息记录（v0.6）。
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroSession {
    pub id: i64,
    pub entry_id: Option<i64>,
    /// focus | break
    pub kind: String,
    pub duration_seconds: i64,
    /// YYYY-MM-DD
    pub day: String,
    pub started_at: String,
    pub ended_at: String,
    pub completed: bool,
    pub created_at: String,
    /// 关联事项内容（列表 JOIN 带出）
    #[sqlx(default)]
    pub entry_content: Option<String>,
}

/// 番茄钟总览。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroSummary {
    pub today_count: i64,
    pub today_minutes: i64,
    pub week_count: i64,
    pub week_minutes: i64,
    pub total_count: i64,
    pub total_minutes: i64,
    /// 连续专注天数（按有 focus 记录的 day 计算）
    pub streak_days: i64,
}

/// 图片附件（v0.5.1）。正文里以 `att:<id>` 引用，文件在数据目录 `attachments/` 下。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: i64,
    /// 所属记录；粘贴后未保存时为 None
    pub entry_id: Option<i64>,
    pub mime: String,
    pub byte_size: i64,
    /// 绝对路径（前端用 convertFileSrc 转成可显示的 URL）
    pub path: String,
    pub created_at: String,
}
