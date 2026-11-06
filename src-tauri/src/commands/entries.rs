//! 记录（entries）相关命令。
//!
//! 命令名、参数名、返回结构严格对齐 docs/接口契约-v1.md 第 4 节：
//! 参数由 Tauri 自动从 JS 的 camelCase 映射到 Rust 的 snake_case。

use sqlx::{Executor, Sqlite, SqlitePool};
use tauri::{AppHandle, State, WebviewWindow};

use crate::db::models::{Entry, EntryPatch};
use crate::db::{
    normalize_due_date, normalize_due_time, now_timestamp, parse_date, today_string,
};
use crate::{emit_entries_changed, AppState, QUICK_WINDOW_LABEL};

/// 查询记录时统一使用的列，顺序与 `Entry` 字段一一对应。
/// v0.3 起包含 `due_time`；v0.4 起包含 `project_id` / `parent_id` 与列表附带字段
/// （`project_name` / `project_color` / `child_total` / `child_done`）。
/// 末尾的 `sort_order` 不属于 `Entry` 结构（`FromRow` 会忽略多余列），但列表排序要用。
/// 漏掉任何一个实际字段都会导致 `FromRow` 解码失败——所有 SELECT 必须走 `select_sql`。
pub const ENTRY_COLUMNS: &str = "id, day, content, is_todo, due_date, due_time, completed_at, \
                                 is_pinned, is_deleted, created_at, updated_at, \
                                 project_id, parent_id, project_name, project_color, \
                                 child_total, child_done, sort_order, quadrant, project_is_secret";

/// 记录查询的“数据源”（v0.4）。
///
/// 外面包一层派生表，让外层 `WHERE` / `ORDER BY` 里的 `id` / `created_at` / `is_deleted`
/// 没有歧义（`projects` 表也有同名列），各列表查询因此可以继续写不带表别名的条件。
///
/// - `project_name` / `project_color`：LEFT JOIN 项目带出，避免前端二次查询；
/// - `child_total` / `child_done`：父任务的子任务进度。没有子任务时为 NULL
///   （契约：`childTotal` 只在“顶层且有子任务时”有值），有子任务时 `child_done` 可为 0。
///   只统计未删除的子任务，所以父任务进回收站后进度自然消失。
pub const ENTRY_SOURCE: &str = "\
    SELECT e.id AS id, e.day AS day, e.content AS content, e.is_todo AS is_todo, \
           e.due_date AS due_date, e.due_time AS due_time, e.completed_at AS completed_at, \
           e.is_pinned AS is_pinned, e.is_deleted AS is_deleted, e.created_at AS created_at, \
           e.updated_at AS updated_at, e.project_id AS project_id, e.parent_id AS parent_id, \
           e.quadrant AS quadrant, COALESCE(p.is_secret, 0) AS project_is_secret, \
           p.name AS project_name, p.color AS project_color, \
           NULLIF((SELECT COUNT(*) FROM entries c \
                   WHERE c.parent_id = e.id AND c.is_deleted = 0), 0) AS child_total, \
           CASE WHEN (SELECT COUNT(*) FROM entries c \
                      WHERE c.parent_id = e.id AND c.is_deleted = 0) > 0 \
                THEN (SELECT COUNT(*) FROM entries c \
                      WHERE c.parent_id = e.id AND c.is_deleted = 0 \
                        AND c.completed_at IS NOT NULL) \
                ELSE NULL END AS child_done, \
           e.sort_order AS sort_order \
    FROM entries e LEFT JOIN projects p ON p.id = e.project_id AND p.is_deleted = 0";

/// 拼一个 `SELECT ... FROM (记录数据源) <suffix>` 语句。
pub fn select_sql(suffix: &str) -> String {
    format!("SELECT {ENTRY_COLUMNS} FROM ({ENTRY_SOURCE}) {suffix}")
}

/// 内容不能为空（前端也会拦，但后端必须自己校验，错误可直接展示）。
pub fn normalize_content(raw: &str) -> Result<String, String> {
    let content = raw.trim();
    if content.is_empty() {
        return Err("记录内容不能为空".to_string());
    }
    Ok(content.to_string())
}

/// 按 id 读一条记录（含项目与子任务进度），也用于层级校验。
pub(crate) async fn select_entry<'e, E>(executor: E, id: i64) -> Result<Option<Entry>, String>
where
    E: Executor<'e, Database = Sqlite>,
{
    let sql = select_sql("WHERE id = ?1");
    sqlx::query_as::<_, Entry>(&sql)
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(|e| format!("读取记录失败：{e}"))
}

/// 某一天的顶层记录：置顶优先，其余按创建时间倒序。
///
/// v0.4：子任务不进入当天记录列表（`parent_id IS NULL`），避免父子重复出现。
pub async fn list_entries_core(pool: &SqlitePool, day: &str) -> Result<Vec<Entry>, String> {
    let sql = select_sql(
        "WHERE day = ?1 AND is_deleted = 0 AND parent_id IS NULL \
         ORDER BY is_pinned DESC, created_at DESC, id DESC",
    );
    sqlx::query_as::<_, Entry>(&sql)
        .bind(day)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("读取 {day} 的记录失败：{e}"))
}

/// 新建记录（v1 兼容入口：不带具体截止时间）。仅测试使用，
/// 业务命令走 `create_entry_full_core`。
#[cfg(test)]
pub async fn create_entry_core(
    pool: &SqlitePool,
    content: &str,
    is_todo: bool,
    due_date: Option<String>,
) -> Result<Entry, String> {
    create_entry_full_core(pool, content, is_todo, due_date, None, None).await
}

/// 新建记录（v0.3 兼容入口：不带项目）。仅测试使用。
#[cfg(test)]
pub async fn create_entry_with_due_time_core(
    pool: &SqlitePool,
    content: &str,
    is_todo: bool,
    due_date: Option<String>,
    due_time: Option<String>,
) -> Result<Entry, String> {
    create_entry_full_core(pool, content, is_todo, due_date, due_time, None).await
}

/// 新建记录：`day` 取当前本地日期，创建/更新时间自动；支持 `due_date + due_time` 与 `project_id`。
pub async fn create_entry_full_core(
    pool: &SqlitePool,
    content: &str,
    is_todo: bool,
    due_date: Option<String>,
    due_time: Option<String>,
    project_id: Option<i64>,
) -> Result<Entry, String> {
    let content = normalize_content(content)?;
    let due_date = normalize_due_date(due_date)?;
    let due_time = normalize_due_time(due_time)?;
    // 只有时间没有日期没有意义：截止时刻无法确定（契约 v0.3 第 1.1 节）。
    if due_time.is_some() && due_date.is_none() {
        return Err("设置截止时间前请先设置截止日期".to_string());
    }
    let day = today_string();
    let now = now_timestamp();

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("数据库事务开启失败：{e}"))?;
    // v0.4：快速输入 `#项目名` 直接带项目建条目；项目必须存在且未删除。
    crate::commands::projects::ensure_project_exists(&mut *tx, project_id).await?;
    let result = sqlx::query(
        "INSERT INTO entries \
         (day, content, is_todo, due_date, due_time, completed_at, is_pinned, is_deleted, \
          project_id, parent_id, sort_order, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, 0, 0, ?6, NULL, 0, ?7, ?7)",
    )
    .bind(&day)
    .bind(&content)
    .bind(is_todo)
    .bind(due_date.as_deref())
    .bind(due_time.as_deref())
    .bind(project_id)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("保存记录失败：{e}"))?;

    let id = result.last_insert_rowid();
    let entry = select_entry(&mut *tx, id)
        .await?
        .ok_or_else(|| "保存记录失败：写入后无法读回该记录".to_string())?;
    tx.commit()
        .await
        .map_err(|e| format!("保存记录失败：{e}"))?;
    Ok(entry)
}

/// 该记录是否有未删除的子任务（用于禁止“有子任务的任务改成普通记录”）。
pub(crate) async fn has_children<'e, E>(executor: E, id: i64) -> Result<bool, String>
where
    E: Executor<'e, Database = Sqlite>,
{
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM entries WHERE parent_id = ?1 AND is_deleted = 0",
    )
    .bind(id)
    .fetch_one(executor)
    .await
    .map_err(|e| format!("读取子任务失败：{e}"))?;
    Ok(count > 0)
}

/// 局部更新记录：字段缺省不改，`due_date` / `due_time` / `project_id` 显式 null 清空。
///
/// v0.4 层级约束（契约第 1 节）：
/// - 子任务不接受 `dueDate` / `dueTime` → 中文错误「子任务不支持截止时间」；
/// - 子任务不能改成非待办，也不能单独改项目（项目跟随父任务）；
/// - 父任务改项目时，其子任务的 `project_id` 一并同步；
/// - 有子任务的任务不能改成普通记录（否则 `parent_id` 会指向非待办记录）。
pub async fn update_entry_core(
    pool: &SqlitePool,
    id: i64,
    patch: EntryPatch,
) -> Result<Entry, String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("数据库事务开启失败：{e}"))?;

    let existing = select_entry(&mut *tx, id)
        .await?
        .ok_or_else(|| "记录不存在或已被删除".to_string())?;
    let is_child = existing.parent_id.is_some();

    if is_child {
        // 契约第 1 节第 3 条：对子任务设置截止日期/时间一律报错。
        let sets_due_date =
            matches!(&patch.due_date, Some(Some(value)) if !value.trim().is_empty());
        let sets_due_time =
            matches!(&patch.due_time, Some(Some(value)) if !value.trim().is_empty());
        if sets_due_date || sets_due_time {
            return Err("子任务不支持截止时间".to_string());
        }
        if patch.is_todo == Some(false) {
            return Err("子任务必须是待办任务".to_string());
        }
        // 子任务的项目由父任务决定；允许“原值回传”这种无变化补丁，真正改项目则报错。
        if let Some(target) = patch.project_id
            && target != existing.project_id
        {
            return Err("子任务的项目跟随父任务，请修改父任务".to_string());
        }
    } else if patch.is_todo == Some(false) && has_children(&mut *tx, id).await? {
        return Err("该任务下还有子任务，不能改成普通记录".to_string());
    }

    let content = match patch.content {
        Some(raw) => normalize_content(&raw)?,
        None => existing.content,
    };
    let is_todo = patch.is_todo.unwrap_or(existing.is_todo);
    let due_date = match patch.due_date {
        // 显式传了该字段（含 null）：null / 空串都表示清空
        Some(value) => normalize_due_date(value)?,
        None => existing.due_date,
    };
    let due_time_patch = patch.due_time;
    // 前端是否显式传了 dueTime（含 null / 空串都算“传了”）。
    let due_time_was_set = matches!(&due_time_patch, Some(Some(_)));
    let mut due_time = match due_time_patch {
        Some(value) => normalize_due_time(value)?,
        None => existing.due_time,
    };
    if due_date.is_none() {
        // 只设时间不设日期属于无效输入；反过来清掉日期时时间一并清掉。
        if due_time_was_set && due_time.is_some() {
            return Err("设置截止时间前请先设置截止日期".to_string());
        }
        due_time = None;
    }
    let is_pinned = patch.is_pinned.unwrap_or(existing.is_pinned);
    let project_id = match patch.project_id {
        Some(value) => value,
        None => existing.project_id,
    };
    if !is_child {
        crate::commands::projects::ensure_project_exists(&mut *tx, project_id).await?;
    }
    let now = now_timestamp();

    sqlx::query(
        "UPDATE entries SET content = ?1, is_todo = ?2, due_date = ?3, due_time = ?4, \
         is_pinned = ?5, project_id = ?6, updated_at = ?7 WHERE id = ?8",
    )
    .bind(&content)
    .bind(is_todo)
    .bind(due_date.as_deref())
    .bind(due_time.as_deref())
    .bind(is_pinned)
    .bind(project_id)
    .bind(&now)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("更新记录失败：{e}"))?;

    // 契约第 1 节第 6 条：父任务 project_id 变更 → 子任务同步为相同 project_id。
    if !is_child && project_id != existing.project_id {
        sqlx::query("UPDATE entries SET project_id = ?1, updated_at = ?2 WHERE parent_id = ?3")
            .bind(project_id)
            .bind(&now)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("同步子任务项目失败：{e}"))?;
    }

    let entry = select_entry(&mut *tx, id)
        .await?
        .ok_or_else(|| "更新记录失败：写入后无法读回该记录".to_string())?;
    tx.commit()
        .await
        .map_err(|e| format!("更新记录失败：{e}"))?;
    Ok(entry)
}

/// 勾选/取消完成：写入或清空 `completed_at`。
///
/// v0.8 父子联动（用户确认口径）：
/// - 勾选**有子任务**的父任务时，若还有未完成子任务 → 直接报错，不允许勾；
/// - 子任务状态变化后重算父任务：全部完成 → 父任务自动打勾；有未完成 → 父任务自动取消打勾。
pub async fn set_entry_completed_core(
    pool: &SqlitePool,
    id: i64,
    completed: bool,
) -> Result<Entry, String> {
    let now = now_timestamp();
    let completed_at = if completed { Some(now.clone()) } else { None };

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("数据库事务开启失败：{e}"))?;

    let parent_id: Option<i64> =
        sqlx::query_scalar("SELECT parent_id FROM entries WHERE id = ?1 AND is_deleted = 0")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("读取记录失败：{e}"))?
            .ok_or_else(|| "记录不存在或已被删除".to_string())?;

    if completed {
        let unfinished: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM entries \
             WHERE parent_id = ?1 AND is_deleted = 0 AND completed_at IS NULL",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| format!("统计子任务失败：{e}"))?;
        if unfinished > 0 {
            return Err(format!(
                "还有 {unfinished} 个子任务未完成，全部完成后这个任务才算完成"
            ));
        }
    }

    let result = sqlx::query(
        "UPDATE entries SET completed_at = ?1, updated_at = ?2 WHERE id = ?3 AND is_deleted = 0",
    )
    .bind(completed_at.as_deref())
    .bind(&now)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("更新完成状态失败：{e}"))?;
    if result.rows_affected() == 0 {
        return Err("记录不存在或已被删除".to_string());
    }

    // 改的是子任务 → 父任务完成态跟着重算
    if let Some(pid) = parent_id {
        sync_parent_completion(&mut tx, pid, &now).await?;
    }

    let entry = select_entry(&mut *tx, id)
        .await?
        .ok_or_else(|| "更新完成状态失败：写入后无法读回该记录".to_string())?;
    tx.commit()
        .await
        .map_err(|e| format!("更新完成状态失败：{e}"))?;
    Ok(entry)
}

/// 重算父任务完成态：子任务全完成 → 打勾；否则 → 取消打勾（v0.8）。
///
/// 供子任务的勾选 / 新增 / 删除复用；父任务没有子任务时不动。
pub(crate) async fn sync_parent_completion(
    conn: &mut sqlx::SqliteConnection,
    parent_id: i64,
    now: &str,
) -> Result<(), String> {
    let (total, done): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(completed_at IS NOT NULL), 0) FROM entries \
         WHERE parent_id = ?1 AND is_deleted = 0",
    )
    .bind(parent_id)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| format!("统计子任务完成情况失败：{e}"))?;

    if total == 0 {
        return Ok(());
    }

    let completed_at: Option<String> = if done == total {
        Some(now.to_string())
    } else {
        None
    };
    sqlx::query(
        "UPDATE entries SET completed_at = ?1, updated_at = ?2 WHERE id = ?3 AND is_deleted = 0",
    )
    .bind(completed_at.as_deref())
    .bind(now)
    .bind(parent_id)
    .execute(&mut *conn)
    .await
    .map_err(|e| format!("同步父任务完成状态失败：{e}"))?;
    Ok(())
}

/// 软删除。v0.4：父任务软删除时子任务一并软删除（契约第 1 节第 5 条）。
pub async fn delete_entry_core(pool: &SqlitePool, id: i64) -> Result<(), String> {
    let now = now_timestamp();
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("数据库事务开启失败：{e}"))?;
    // 删的是子任务时，删完要重算父任务完成态（v0.8）
    let parent_id: Option<i64> =
        sqlx::query_scalar("SELECT parent_id FROM entries WHERE id = ?1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("读取记录失败：{e}"))?
            .flatten();
    let result = sqlx::query("UPDATE entries SET is_deleted = 1, updated_at = ?1 WHERE id = ?2")
        .bind(&now)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("删除记录失败：{e}"))?;
    if result.rows_affected() == 0 {
        return Err("记录不存在".to_string());
    }
    // 子任务跟着父任务进回收站，但回收站列表只列顶层记录（不单独展示子任务）。
    sqlx::query("UPDATE entries SET is_deleted = 1, updated_at = ?1 WHERE parent_id = ?2")
        .bind(&now)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("删除子任务失败：{e}"))?;
    if let Some(pid) = parent_id {
        sync_parent_completion(&mut tx, pid, &now).await?;
    }
    tx.commit()
        .await
        .map_err(|e| format!("删除记录失败：{e}"))?;
    Ok(())
}

/// 从回收站恢复。v0.4：父任务恢复时子任务一并恢复（契约第 1 节第 5 条）。
pub async fn restore_entry_core(pool: &SqlitePool, id: i64) -> Result<Entry, String> {
    let now = now_timestamp();
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("数据库事务开启失败：{e}"))?;
    let result = sqlx::query("UPDATE entries SET is_deleted = 0, updated_at = ?1 WHERE id = ?2")
        .bind(&now)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("恢复记录失败：{e}"))?;
    if result.rows_affected() == 0 {
        return Err("记录不存在".to_string());
    }
    // 恢复父任务时子任务一并恢复；恢复子任务时该语句自然不命中任何行。
    sqlx::query("UPDATE entries SET is_deleted = 0, updated_at = ?1 WHERE parent_id = ?2")
        .bind(&now)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("恢复子任务失败：{e}"))?;
    let entry = select_entry(&mut *tx, id)
        .await?
        .ok_or_else(|| "恢复记录失败：写入后无法读回该记录".to_string())?;
    tx.commit()
        .await
        .map_err(|e| format!("恢复记录失败：{e}"))?;
    Ok(entry)
}

/// 物理删除（仅回收站内的记录）。
///
/// v0.4：`entries.parent_id` 有外键指向 `entries.id`，先把子任务行物理删掉再删父任务，
/// 否则会因外键约束失败（子任务此时也已在回收站里）。
pub async fn purge_entry_core(pool: &SqlitePool, id: i64) -> Result<(), String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("数据库事务开启失败：{e}"))?;
    let existing = select_entry(&mut *tx, id)
        .await?
        .ok_or_else(|| "记录不存在".to_string())?;
    if !existing.is_deleted {
        return Err("只能彻底删除回收站中的记录".to_string());
    }
    sqlx::query("DELETE FROM entries WHERE parent_id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("彻底删除子任务失败：{e}"))?;
    sqlx::query("DELETE FROM entries WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("彻底删除记录失败：{e}"))?;
    tx.commit()
        .await
        .map_err(|e| format!("彻底删除记录失败：{e}"))?;
    Ok(())
}

/// 回收站列表：按更新时间倒序。v0.4 只列顶层被删记录（子任务随父任务删除，不单独展示）。
pub async fn list_trash_core(pool: &SqlitePool) -> Result<Vec<Entry>, String> {
    let sql = select_sql(
        "WHERE is_deleted = 1 AND parent_id IS NULL ORDER BY updated_at DESC, id DESC",
    );
    sqlx::query_as::<_, Entry>(&sql)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("读取回收站失败：{e}"))
}

/// start~end（含两端）之间存在未删除记录的日期，升序去重。
/// v0.4：子任务不算“有记录的一天”（`parent_id IS NULL`）。
pub async fn list_marked_days_core(
    pool: &SqlitePool,
    start: &str,
    end: &str,
) -> Result<Vec<String>, String> {
    sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT day FROM entries \
         WHERE is_deleted = 0 AND parent_id IS NULL AND day BETWEEN ?1 AND ?2 ORDER BY day",
    )
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("读取有记录的日期失败：{e}"))
}

/// 校验区间端点日期格式（错误信息可直接展示给用户）。
fn validate_range_date(value: &str, label: &str) -> Result<(), String> {
    if parse_date(value).is_none() {
        return Err(format!("{label}格式无效（应为 YYYY-MM-DD）：{}", value.trim()));
    }
    Ok(())
}

/// 日历日/周视图：`due_date` 落在 [start, end]（含两端）的顶层待办，**包含已完成**，
/// 不含已删除、非待办记录与子任务（v0.4：日历只显示顶层任务）；
/// 按 `due_date`、`due_time` 升序（无具体时间的排后）。
pub async fn list_todos_range_core(
    pool: &SqlitePool,
    start: &str,
    end: &str,
) -> Result<Vec<Entry>, String> {
    let start = start.trim();
    let end = end.trim();
    validate_range_date(start, "开始日期")?;
    validate_range_date(end, "结束日期")?;
    if start > end {
        return Err("开始日期不能晚于结束日期".to_string());
    }

    let sql = select_sql(
        "WHERE is_todo = 1 AND is_deleted = 0 AND parent_id IS NULL \
           AND due_date BETWEEN ?1 AND ?2 \
         ORDER BY due_date ASC, due_time IS NULL ASC, due_time ASC, created_at ASC, id ASC",
    );
    sqlx::query_as::<_, Entry>(&sql)
        .bind(start)
        .bind(end)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("读取 {start} ~ {end} 的待办失败：{e}"))
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_entries(
    state: State<'_, AppState>,
    day: String,
) -> Result<Vec<Entry>, String> {
    let pool = state.pool.read().await;
    list_entries_core(&pool, &day).await
}

#[tauri::command]
pub async fn create_entry(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
    content: String,
    is_todo: Option<bool>,
    due_date: Option<String>,
    due_time: Option<String>,
    project_id: Option<i64>,
) -> Result<Entry, String> {
    let entry = {
        let pool = state.pool.read().await;
        create_entry_full_core(
            &pool,
            &content,
            is_todo.unwrap_or(false),
            due_date,
            due_time,
            project_id,
        )
        .await?
    };

    // 快捷小窗保存后通知主窗口刷新（契约第 6 节）。
    if window.label() == QUICK_WINDOW_LABEL {
        emit_entries_changed(&app);
    }
    Ok(entry)
}

#[tauri::command]
pub async fn update_entry(
    state: State<'_, AppState>,
    id: i64,
    patch: EntryPatch,
) -> Result<Entry, String> {
    let pool = state.pool.read().await;
    update_entry_core(&pool, id, patch).await
}

#[tauri::command]
pub async fn set_entry_completed(
    state: State<'_, AppState>,
    id: i64,
    completed: bool,
) -> Result<Entry, String> {
    let pool = state.pool.read().await;
    set_entry_completed_core(&pool, id, completed).await
}

#[tauri::command]
pub async fn delete_entry(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let pool = state.pool.read().await;
    delete_entry_core(&pool, id).await
}

#[tauri::command]
pub async fn restore_entry(state: State<'_, AppState>, id: i64) -> Result<Entry, String> {
    let pool = state.pool.read().await;
    restore_entry_core(&pool, id).await
}

#[tauri::command]
pub async fn purge_entry(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let pool = state.pool.read().await;
    purge_entry_core(&pool, id).await
}

#[tauri::command]
pub async fn list_trash(state: State<'_, AppState>) -> Result<Vec<Entry>, String> {
    let pool = state.pool.read().await;
    list_trash_core(&pool).await
}

#[tauri::command]
pub async fn list_marked_days(
    state: State<'_, AppState>,
    start: String,
    end: String,
) -> Result<Vec<String>, String> {
    let pool = state.pool.read().await;
    list_marked_days_core(&pool, &start, &end).await
}

/// 日历日/周视图用：区间内（含两端）到期的待办，包含已完成。
#[tauri::command]
pub async fn list_todos_range(
    state: State<'_, AppState>,
    start: String,
    end: String,
) -> Result<Vec<Entry>, String> {
    let pool = state.pool.read().await;
    list_todos_range_core(&pool, &start, &end).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{cleanup_test_pool, test_pool};

    async fn created(pool: &SqlitePool, content: &str) -> Entry {
        create_entry_core(pool, content, false, None).await.unwrap()
    }

    #[tokio::test]
    async fn create_and_list_orders_pinned_first() {
        let (pool, dir) = test_pool().await;
        let first = created(&pool, "第一条").await;
        let second = created(&pool, "第二条").await;
        let third = created(&pool, "第三条").await;

        // 置顶第三条：应排在最前，其余按 created_at 倒序
        let mut patch = EntryPatch::default();
        patch.is_pinned = Some(true);
        update_entry_core(&pool, third.id, patch).await.unwrap();

        let list = list_entries_core(&pool, &today_string()).await.unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].id, third.id);
        assert_eq!(list[1].id, second.id);
        assert_eq!(list[2].id, first.id);
        assert!(list[0].is_pinned);
        assert_eq!(list[0].day, today_string());

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn empty_content_is_rejected() {
        let (pool, dir) = test_pool().await;
        let err = create_entry_core(&pool, "   ", false, None).await.unwrap_err();
        assert!(err.contains("不能为空"), "错误信息应为中文：{err}");
        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn due_date_validation_and_clearing() {
        let (pool, dir) = test_pool().await;
        let entry = create_entry_core(&pool, "带截止", true, Some("2026-11-10".into()))
            .await
            .unwrap();
        assert_eq!(entry.due_date.as_deref(), Some("2026-11-10"));

        // 非法日期
        let err = create_entry_core(&pool, "x", true, Some("2026/11/10".into()))
            .await
            .unwrap_err();
        assert!(err.contains("YYYY-MM-DD"));

        // 显式 null 清空
        let mut patch = EntryPatch::default();
        patch.due_date = Some(None);
        let cleared = update_entry_core(&pool, entry.id, patch).await.unwrap();
        assert_eq!(cleared.due_date, None);

        // 缺省不改
        let mut patch = EntryPatch::default();
        patch.content = Some("改名".into());
        let renamed = update_entry_core(&pool, entry.id, patch).await.unwrap();
        assert_eq!(renamed.content, "改名");
        assert_eq!(renamed.due_date, None);
        assert!(renamed.updated_at >= renamed.created_at);

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn patch_deserializes_missing_vs_explicit_null() {
        let patch: EntryPatch = serde_json::from_str("{\"content\":\"只改内容\"}").unwrap();
        assert_eq!(patch.due_date, None); // 缺省 = 不改
        let patch: EntryPatch = serde_json::from_str("{\"dueDate\":null}").unwrap();
        assert_eq!(patch.due_date, Some(None)); // 显式 null = 清空
        let patch: EntryPatch = serde_json::from_str("{\"dueDate\":\"2026-12-01\"}").unwrap();
        assert_eq!(patch.due_date, Some(Some("2026-12-01".into())));
    }

    #[tokio::test]
    async fn soft_delete_restore_and_purge() {
        let (pool, dir) = test_pool().await;
        let entry = created(&pool, "待删除").await;

        delete_entry_core(&pool, entry.id).await.unwrap();
        // 当天列表不再出现，回收站可见
        assert!(list_entries_core(&pool, &today_string()).await.unwrap().is_empty());
        let trash = list_trash_core(&pool).await.unwrap();
        assert_eq!(trash.len(), 1);
        assert!(trash[0].is_deleted);

        // 未在回收站的记录不能物理删除
        let err = purge_entry_core(&pool, entry.id).await;
        assert!(err.is_ok(), "回收站内记录应可物理删除");

        let entry2 = created(&pool, "恢复我").await;
        delete_entry_core(&pool, entry2.id).await.unwrap();
        let restored = restore_entry_core(&pool, entry2.id).await.unwrap();
        assert!(!restored.is_deleted);
        assert_eq!(list_entries_core(&pool, &today_string()).await.unwrap().len(), 1);

        let entry3 = created(&pool, "普通记录").await;
        let err = purge_entry_core(&pool, entry3.id).await.unwrap_err();
        assert!(err.contains("回收站"));

        // 物理删除后彻底消失
        delete_entry_core(&pool, entry3.id).await.unwrap();
        purge_entry_core(&pool, entry3.id).await.unwrap();
        assert!(list_trash_core(&pool).await.unwrap().is_empty());

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn completed_at_written_and_cleared() {
        let (pool, dir) = test_pool().await;
        let entry = create_entry_core(&pool, "待办", true, None).await.unwrap();
        let done = set_entry_completed_core(&pool, entry.id, true).await.unwrap();
        assert!(done.completed_at.is_some());
        let undone = set_entry_completed_core(&pool, entry.id, false).await.unwrap();
        assert!(undone.completed_at.is_none());
        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn marked_days_are_distinct_and_sorted() {
        let (pool, dir) = test_pool().await;
        let a = created(&pool, "今天").await;
        // 手动插入历史日期记录 + 一条已删除记录
        sqlx::query(
            "INSERT INTO entries (day, content, is_todo, is_pinned, is_deleted, created_at, updated_at) \
             VALUES ('2026-10-01', '旧', 0, 0, 0, '2026-10-01T09:00:00', '2026-10-01T09:00:00'), \
                    ('2026-10-03', '删', 0, 0, 1, '2026-10-03T09:00:00', '2026-10-03T09:00:00'), \
                    ('2026-10-05', '另一条', 0, 0, 0, '2026-10-05T09:00:00', '2026-10-05T09:00:00')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let days = list_marked_days_core(&pool, "2000-01-01", "2099-12-31")
            .await
            .unwrap();
        let today = today_string();
        let mut expected = vec![
            "2026-10-01".to_string(),
            "2026-10-05".to_string(),
            today,
        ];
        expected.sort();
        assert_eq!(days, expected);
        // 已删除的记录不出现
        assert!(!days.contains(&"2026-10-03".to_string()));
        // 边界外不出现
        assert!(
            list_marked_days_core(&pool, "2026-09-01", "2026-09-30")
                .await
                .unwrap()
                .is_empty()
        );
        let _ = a;

        cleanup_test_pool(pool, dir).await;
    }

    // -----------------------------------------------------------------------
    // v0.3：due_time 与日历区间查询
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn due_time_is_stored_validated_and_cleared() {
        let (pool, dir) = test_pool().await;
        // 日期 + 时间
        let entry = create_entry_with_due_time_core(
            &pool,
            "带时间",
            true,
            Some("2026-11-10".into()),
            Some("15:30".into()),
        )
        .await
        .unwrap();
        assert_eq!(entry.due_date.as_deref(), Some("2026-11-10"));
        assert_eq!(entry.due_time.as_deref(), Some("15:30"));

        // 只有时间没有日期：拒绝
        let err = create_entry_with_due_time_core(&pool, "只有时间", true, None, Some("15:30".into()))
            .await
            .unwrap_err();
        assert!(err.contains("截止日期"), "错误信息应为中文：{err}");

        // 非法时间格式：拒绝
        let err = create_entry_with_due_time_core(
            &pool,
            "坏时间",
            true,
            Some("2026-11-10".into()),
            Some("25:99".into()),
        )
        .await
        .unwrap_err();
        assert!(err.contains("HH:MM"));

        // 缺省 patch 不改 due_time；显式 null 清空
        let mut patch = EntryPatch::default();
        patch.content = Some("改内容".into());
        let unchanged = update_entry_core(&pool, entry.id, patch).await.unwrap();
        assert_eq!(unchanged.due_time.as_deref(), Some("15:30"));

        let mut patch = EntryPatch::default();
        patch.due_time = Some(None);
        let cleared = update_entry_core(&pool, entry.id, patch).await.unwrap();
        assert_eq!(cleared.due_time, None);
        assert_eq!(cleared.due_date.as_deref(), Some("2026-11-10"));

        // 显式清空日期时时间一并清空
        let mut patch = EntryPatch::default();
        patch.due_date = Some(None);
        let dated = create_entry_with_due_time_core(
            &pool,
            "再建一条",
            true,
            Some("2026-11-11".into()),
            Some("08:00".into()),
        )
        .await
        .unwrap();
        let cleared = update_entry_core(&pool, dated.id, patch).await.unwrap();
        assert_eq!(cleared.due_date, None);
        assert_eq!(cleared.due_time, None);

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn patch_deserializes_due_time_missing_vs_explicit_null() {
        let patch: EntryPatch = serde_json::from_str("{\"content\":\"只改内容\"}").unwrap();
        assert_eq!(patch.due_time, None); // 缺省 = 不改
        let patch: EntryPatch = serde_json::from_str("{\"dueTime\":null}").unwrap();
        assert_eq!(patch.due_time, Some(None)); // 显式 null = 清空
        let patch: EntryPatch = serde_json::from_str("{\"dueTime\":\"09:05\"}").unwrap();
        assert_eq!(patch.due_time, Some(Some("09:05".into())));
    }

    #[tokio::test]
    async fn list_todos_range_includes_both_ends_and_completed() {
        let (pool, dir) = test_pool().await;
        let first = create_entry_with_due_time_core(
            &pool,
            "区间起点",
            true,
            Some("2026-11-01".into()),
            None,
        )
        .await
        .unwrap();
        let last = create_entry_with_due_time_core(
            &pool,
            "区间终点",
            true,
            Some("2026-11-07".into()),
            Some("18:00".into()),
        )
        .await
        .unwrap();
        // 区间外
        create_entry_with_due_time_core(
            &pool,
            "区间前",
            true,
            Some("2026-10-31".into()),
            None,
        )
        .await
        .unwrap();
        create_entry_with_due_time_core(
            &pool,
            "区间后",
            true,
            Some("2026-11-08".into()),
            None,
        )
        .await
        .unwrap();
        // 非待办 / 已删除 / 无 due_date 都不出现在日历里
        create_entry_core(&pool, "普通记录", false, Some("2026-11-03".into()))
            .await
            .unwrap();
        let deleted = create_entry_core(&pool, "删掉的待办", true, Some("2026-11-04".into()))
            .await
            .unwrap();
        delete_entry_core(&pool, deleted.id).await.unwrap();
        create_entry_core(&pool, "无截止待办", true, None).await.unwrap();
        // 已完成也要包含
        let done = create_entry_core(&pool, "已完成的待办", true, Some("2026-11-02".into()))
            .await
            .unwrap();
        set_entry_completed_core(&pool, done.id, true).await.unwrap();

        let range = list_todos_range_core(&pool, "2026-11-01", "2026-11-07")
            .await
            .unwrap();
        let ids: Vec<i64> = range.iter().map(|e| e.id).collect();
        assert!(ids.contains(&first.id));
        assert!(ids.contains(&last.id));
        assert!(ids.contains(&done.id));
        assert!(!range.iter().any(|e| e.content == "区间前"));
        assert!(!range.iter().any(|e| e.content == "区间后"));
        assert!(!range.iter().any(|e| e.content == "普通记录"));
        assert!(!range.iter().any(|e| e.content == "删掉的待办"));
        assert!(!range.iter().any(|e| e.content == "无截止待办"));

        // 排序：日期升序；同一天有时间在前、无时间在后
        let same_day_no_time =
            create_entry_core(&pool, "同日无时间", true, Some("2026-11-05".into()))
                .await
                .unwrap();
        let _same_day_timed = create_entry_with_due_time_core(
            &pool,
            "同日 09:00",
            true,
            Some("2026-11-05".into()),
            Some("09:00".into()),
        )
        .await
        .unwrap();
        let range = list_todos_range_core(&pool, "2026-11-01", "2026-11-07")
            .await
            .unwrap();
        let dates: Vec<&str> = range.iter().map(|e| e.due_date.as_deref().unwrap()).collect();
        let mut sorted = dates.clone();
        sorted.sort();
        assert_eq!(dates, sorted);
        let same_day: Vec<&Entry> = range
            .iter()
            .filter(|e| e.due_date.as_deref() == Some("2026-11-05"))
            .collect();
        assert_eq!(same_day.len(), 2);
        assert_eq!(same_day[0].due_time.as_deref(), Some("09:00"));
        assert_eq!(same_day[1].id, same_day_no_time.id);

        // 非法区间
        assert!(list_todos_range_core(&pool, "2026-11-01", "2026/11/07").await.is_err());
        assert!(list_todos_range_core(&pool, "2026-11-08", "2026-11-01").await.is_err());

        cleanup_test_pool(pool, dir).await;
    }

    // -----------------------------------------------------------------------
    // v0.4：项目字段与任务层级
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn list_entries_carries_project_and_child_progress() {
        let (pool, dir) = test_pool().await;
        let project = crate::commands::projects::create_project_core(
            &pool,
            "项目A",
            Some("amber".into()),
            None,
        )
        .await
        .unwrap();
        let parent = create_entry_full_core(&pool, "父任务", true, None, None, Some(project.id))
            .await
            .unwrap();
        let first = crate::commands::children::create_child_core(&pool, parent.id, "子一")
            .await
            .unwrap();
        let _second = crate::commands::children::create_child_core(&pool, parent.id, "子二")
            .await
            .unwrap();
        set_entry_completed_core(&pool, first.id, true).await.unwrap();

        let list = list_entries_core(&pool, &today_string()).await.unwrap();
        assert_eq!(list.len(), 1, "子任务不出现在当天记录列表里");
        let top = &list[0];
        assert_eq!(top.id, parent.id);
        assert_eq!(top.project_id, Some(project.id));
        assert_eq!(top.project_name.as_deref(), Some("项目A"));
        assert_eq!(top.project_color.as_deref(), Some("amber"));
        assert_eq!(top.child_total, Some(2));
        assert_eq!(top.child_done, Some(1));
        assert_eq!(top.parent_id, None);

        // 没有子任务、没有项目的记录：四个字段都是 None
        let plain = create_entry_full_core(&pool, "普通记录", false, None, None, None)
            .await
            .unwrap();
        let list = list_entries_core(&pool, &today_string()).await.unwrap();
        let plain_row = list.iter().find(|e| e.id == plain.id).unwrap();
        assert_eq!(plain_row.project_id, None);
        assert_eq!(plain_row.project_name, None);
        assert_eq!(plain_row.child_total, None);
        assert_eq!(plain_row.child_done, None);

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn children_are_excluded_from_top_level_queries() {
        let (pool, dir) = test_pool().await;
        let today = today_string();
        let parent = create_entry_full_core(&pool, "父任务", true, Some(today.clone()), None, None)
            .await
            .unwrap();
        let child = crate::commands::children::create_child_core(&pool, parent.id, "子任务")
            .await
            .unwrap();
        // 子任务本不该有截止时间；直接改库模拟脏数据，验证顶层查询/提醒都排除它。
        sqlx::query(
            "UPDATE entries SET due_date = '2000-01-01', due_time = '09:00', day = '2000-01-01' \
             WHERE id = ?1",
        )
        .bind(child.id)
        .execute(&pool)
        .await
        .unwrap();

        // 当天记录
        let list = list_entries_core(&pool, &today).await.unwrap();
        assert!(list.iter().all(|e| e.id != child.id));

        // 待办四个分组
        for group in ["pending", "dueToday", "overdue", "done"] {
            let todos = crate::commands::todos::list_todos_core(&pool, group)
                .await
                .unwrap();
            assert!(
                todos.iter().all(|e| e.id != child.id),
                "待办分组 {group} 不应包含子任务"
            );
        }

        // 日历（子任务的“逾期”不应出现）
        let range = list_todos_range_core(&pool, "1999-01-01", "2001-01-01")
            .await
            .unwrap();
        assert!(range.iter().all(|e| e.id != child.id));

        // 有记录的日期：子任务不算“有记录的一天”
        let days = list_marked_days_core(&pool, "1999-01-01", "2001-01-01")
            .await
            .unwrap();
        assert!(!days.contains(&"2000-01-01".to_string()));

        // 统计
        let counts = crate::commands::todos::get_counts_core(&pool, &today)
            .await
            .unwrap();
        assert_eq!(counts.today_entries, 1);
        assert_eq!(counts.today_todos, 1);
        assert_eq!(counts.pending_todos, 1);
        assert_eq!(counts.overdue_todos, 0, "子任务的逾期不计数");
        assert_eq!(counts.done_today, 0);

        // 回收站只列父任务
        delete_entry_core(&pool, parent.id).await.unwrap();
        let trash = list_trash_core(&pool).await.unwrap();
        assert_eq!(trash.len(), 1);
        assert_eq!(trash[0].id, parent.id);

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn create_entry_validates_project_id() {
        let (pool, dir) = test_pool().await;
        let project = crate::commands::projects::create_project_core(&pool, "有效项目", None, None)
            .await
            .unwrap();
        let with_project =
            create_entry_full_core(&pool, "带项目", false, None, None, Some(project.id))
                .await
                .unwrap();
        assert_eq!(with_project.project_id, Some(project.id));
        assert_eq!(with_project.project_name.as_deref(), Some("有效项目"));

        // 不存在的项目
        let err = create_entry_full_core(&pool, "坏项目", false, None, None, Some(9999))
            .await
            .unwrap_err();
        assert!(err.contains("项目不存在"), "错误信息应为中文：{err}");

        // 已删除的项目
        crate::commands::projects::delete_project_core(&pool, project.id)
            .await
            .unwrap();
        let err =
            create_entry_full_core(&pool, "删除后的项目", false, None, None, Some(project.id))
                .await
                .unwrap_err();
        assert!(err.contains("项目不存在"));

        cleanup_test_pool(pool, dir).await;
    }
}
