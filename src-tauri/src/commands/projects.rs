//! 项目（projects）相关命令。
//!
//! 契约见 `docs/接口契约-v0.4.md` 第 2/3 节：
//! - 项目是最高层，不支持嵌套；项目不强制：没挂项目的记录/任务仍在（收件箱）。
//! - 项目统计口径（契约第 5 节）：
//!   `task_total/task_done/task_overdue` **只算顶层任务**（`is_todo=1 AND parent_id IS NULL`），
//!   `note_total` 只算非待办记录（`is_todo=0`）。子任务只体现在 `childDone/childTotal`。
//! - `delete_project`：软删除项目 + 其下 entries 的 `project_id` 置 NULL（**不删任务**）。
//!   `restore_project` 只恢复项目本身，任务需用户重新归属。

use sqlx::{Executor, Sqlite, SqlitePool};
use tauri::State;

use crate::commands::entries::select_sql;
use crate::db::models::{Entry, Project, ProjectPatch, PROJECT_COLORS, PROJECT_STATUSES};
use crate::db::{now_timestamp, today_string};
use crate::AppState;

/// 项目统计子查询（契约第 5 节）。
///
/// 以 `p` 为项目别名，返回四列：`task_total, task_done, task_overdue, note_total`。
/// `today` 作为绑定参数 ?1 传入（顶层任务里“已逾期”的比较基准）。
fn stats_columns() -> &'static str {
    "\
    (SELECT COUNT(*) FROM entries t \
       WHERE t.project_id = p.id AND t.is_todo = 1 AND t.parent_id IS NULL \
         AND t.is_deleted = 0) AS task_total, \
    (SELECT COUNT(*) FROM entries t \
       WHERE t.project_id = p.id AND t.is_todo = 1 AND t.parent_id IS NULL \
         AND t.is_deleted = 0 AND t.completed_at IS NOT NULL) AS task_done, \
    (SELECT COUNT(*) FROM entries t \
       WHERE t.project_id = p.id AND t.is_todo = 1 AND t.parent_id IS NULL \
         AND t.is_deleted = 0 AND t.completed_at IS NULL \
         AND t.due_date IS NOT NULL AND t.due_date < ?1) AS task_overdue, \
    (SELECT COUNT(*) FROM entries n \
       WHERE n.project_id = p.id AND n.is_todo = 0 AND n.is_deleted = 0) AS note_total"
}

/// 项目列（与 `Project` 字段一一对应）。
fn project_sql() -> String {
    format!(
        "SELECT p.id AS id, p.name AS name, p.description AS description, \
                p.color AS color, p.status AS status, p.sort_order AS sort_order, \
                {}, p.created_at AS created_at, p.updated_at AS updated_at \
         FROM projects p",
        stats_columns()
    )
}

/// 校验 `project_id`：None 直接通过；Some 时必须指向存在且未删除的项目。
/// 供 `create_entry` / `update_entry` 复用（契约第 3 节 `create_entry` 参数扩展）。
pub(crate) async fn ensure_project_exists<'e, E>(
    executor: E,
    project_id: Option<i64>,
) -> Result<(), String>
where
    E: Executor<'e, Database = Sqlite>,
{
    let Some(project_id) = project_id else {
        return Ok(());
    };
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM projects WHERE id = ?1 AND is_deleted = 0")
            .bind(project_id)
            .fetch_optional(executor)
            .await
            .map_err(|e| format!("读取项目失败：{e}"))?;
    if found.is_none() {
        return Err("项目不存在或已被删除".to_string());
    }
    Ok(())
}

/// 按 id 读一个项目（含统计字段）。`today` 是“已逾期”的比较基准。
pub(crate) async fn select_project<'e, E>(
    executor: E,
    id: i64,
    today: &str,
) -> Result<Option<Project>, String>
where
    E: Executor<'e, Database = Sqlite>,
{
    let sql = format!("{} WHERE p.id = ?2", project_sql());
    sqlx::query_as::<_, Project>(&sql)
        .bind(today)
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(|e| format!("读取项目失败：{e}"))
}

/// 项目列表（契约第 3 节）：按 `sort_order, created_at`；含统计字段。
/// `include_archived = false` 时过滤掉 `status = 'archived'`（归档项目默认不展示）。
pub async fn list_projects_core(
    pool: &SqlitePool,
    include_archived: bool,
    today: &str,
) -> Result<Vec<Project>, String> {
    let archived_filter = if include_archived {
        ""
    } else {
        "AND p.status <> 'archived'"
    };
    let sql = format!(
        "{} WHERE p.is_deleted = 0 {} ORDER BY p.sort_order, p.created_at, p.id",
        project_sql(),
        archived_filter
    );
    sqlx::query_as::<_, Project>(&sql)
        .bind(today)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("读取项目列表失败：{e}"))
}

/// 校验并规范化项目名称：去空白后必须非空。
fn normalize_project_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err("项目名称不能为空".to_string());
    }
    Ok(name.to_string())
}

/// 校验颜色：None/空白 = 无颜色；否则必须命中固定色板（契约第 6 节）。
fn normalize_color(value: Option<String>) -> Result<Option<String>, String> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if PROJECT_COLORS.contains(&trimmed) {
        Ok(Some(trimmed.to_string()))
    } else {
        Err(format!(
            "不支持的项目颜色：{trimmed}（可选值：{}）",
            PROJECT_COLORS.join(" / ")
        ))
    }
}

/// 校验状态取值：active/paused/done/archived。
fn normalize_status(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if PROJECT_STATUSES.contains(&trimmed) {
        Ok(trimmed.to_string())
    } else {
        Err(format!(
            "不支持的项目状态：{trimmed}（可选值：{}）",
            PROJECT_STATUSES.join(" / ")
        ))
    }
}

/// 空串/纯空白视作没有描述。
fn normalize_description(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim().to_string();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    })
}

/// 新建项目（契约第 3 节）：名称去空白后非空；重名允许（前端提示）。
pub async fn create_project_core(
    pool: &SqlitePool,
    name: &str,
    color: Option<String>,
    description: Option<String>,
) -> Result<Project, String> {
    let name = normalize_project_name(name)?;
    let color = normalize_color(color)?;
    let description = normalize_description(description);
    let now = now_timestamp();
    let today = today_string();

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("数据库事务开启失败：{e}"))?;
    let result = sqlx::query(
        "INSERT INTO projects (name, description, color, status, sort_order, is_deleted, \
         created_at, updated_at) \
         VALUES (?1, ?2, ?3, 'active', 0, 0, ?4, ?4)",
    )
    .bind(&name)
    .bind(description.as_deref())
    .bind(color.as_deref())
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("保存项目失败：{e}"))?;
    let id = result.last_insert_rowid();
    let project = select_project(&mut *tx, id, &today)
        .await?
        .ok_or_else(|| "保存项目失败：写入后无法读回该项目".to_string())?;
    tx.commit()
        .await
        .map_err(|e| format!("保存项目失败：{e}"))?;
    Ok(project)
}

/// 更新项目（契约第 3 节）：改名/颜色/描述/状态/排序；状态校验 ∈ active/paused/done/archived。
pub async fn update_project_core(
    pool: &SqlitePool,
    id: i64,
    patch: ProjectPatch,
) -> Result<Project, String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("数据库事务开启失败：{e}"))?;
    // `Project` 对外不暴露 `is_deleted`（契约第 2 节字段清单），这里单独确认未删除。
    let alive: Option<i64> =
        sqlx::query_scalar("SELECT id FROM projects WHERE id = ?1 AND is_deleted = 0")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("读取项目失败：{e}"))?;
    if alive.is_none() {
        return Err("项目不存在或已被删除".to_string());
    }
    let existing = select_project(&mut *tx, id, &today_string())
        .await?
        .ok_or_else(|| "项目不存在或已被删除".to_string())?;

    let name = match patch.name {
        Some(raw) => normalize_project_name(&raw)?,
        None => existing.name,
    };
    let color = match patch.color {
        Some(value) => normalize_color(value)?,
        None => existing.color,
    };
    let description = match patch.description {
        Some(value) => normalize_description(value),
        None => existing.description,
    };
    let status = match patch.status {
        Some(raw) => normalize_status(&raw)?,
        None => existing.status,
    };
    let sort_order = patch.sort_order.unwrap_or(existing.sort_order);
    let now = now_timestamp();

    sqlx::query(
        "UPDATE projects SET name = ?1, description = ?2, color = ?3, status = ?4, \
         sort_order = ?5, updated_at = ?6 WHERE id = ?7",
    )
    .bind(&name)
    .bind(description.as_deref())
    .bind(color.as_deref())
    .bind(&status)
    .bind(sort_order)
    .bind(&now)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("更新项目失败：{e}"))?;

    let project = select_project(&mut *tx, id, &today_string())
        .await?
        .ok_or_else(|| "更新项目失败：写入后无法读回该项目".to_string())?;
    tx.commit()
        .await
        .map_err(|e| format!("更新项目失败：{e}"))?;
    Ok(project)
}

/// 删除项目（契约第 3 节）：软删除项目 + 其下 entries 的 `project_id` 置 NULL（**不删任务**）。
pub async fn delete_project_core(pool: &SqlitePool, id: i64) -> Result<(), String> {
    let now = now_timestamp();
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("数据库事务开启失败：{e}"))?;
    let result = sqlx::query("UPDATE projects SET is_deleted = 1, updated_at = ?1 WHERE id = ?2")
        .bind(&now)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("删除项目失败：{e}"))?;
    if result.rows_affected() == 0 {
        return Err("项目不存在".to_string());
    }
    // 契约第 1 节第 8 条：任务与记录转到「无项目」，绝不级联删除。
    sqlx::query("UPDATE entries SET project_id = NULL, updated_at = ?1 WHERE project_id = ?2")
        .bind(&now)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("解除项目归属失败：{e}"))?;
    tx.commit()
        .await
        .map_err(|e| format!("删除项目失败：{e}"))?;
    Ok(())
}

/// 恢复项目（契约第 3 节）：只恢复项目本身，任务需用户重新归属。
pub async fn restore_project_core(pool: &SqlitePool, id: i64) -> Result<Project, String> {
    let now = now_timestamp();
    let today = today_string();
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("数据库事务开启失败：{e}"))?;
    let found: Option<i64> = sqlx::query_scalar("SELECT id FROM projects WHERE id = ?1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("读取项目失败：{e}"))?;
    if found.is_none() {
        return Err("项目不存在".to_string());
    }
    sqlx::query("UPDATE projects SET is_deleted = 0, updated_at = ?1 WHERE id = ?2")
        .bind(&now)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("恢复项目失败：{e}"))?;
    let project = select_project(&mut *tx, id, &today)
        .await?
        .ok_or_else(|| "恢复项目失败：写入后无法读回该项目".to_string())?;
    tx.commit()
        .await
        .map_err(|e| format!("恢复项目失败：{e}"))?;
    Ok(project)
}

/// 项目页列表（契约第 3 节 `list_project_entries`）。
///
/// - `only_tasks = true`：该项目下的顶层任务（`is_todo=1 AND parent_id IS NULL AND is_deleted=0`），
///   排序：未完成在前 → 截止日期空值靠后 → `due_date` 升序 → `created_at` 降序。
/// - `only_tasks = false`：该项目下的记录（`is_todo=0 AND is_deleted=0`），按 `day DESC, created_at DESC`。
///
/// 返回的 `Entry` 同样带 `projectName/projectColor/childTotal/childDone`（走统一数据源）。
pub async fn list_project_entries_core(
    pool: &SqlitePool,
    project_id: i64,
    only_tasks: bool,
) -> Result<Vec<Entry>, String> {
    ensure_project_exists(pool, Some(project_id)).await?;
    let sql = if only_tasks {
        select_sql(
            "WHERE project_id = ?1 AND is_todo = 1 AND parent_id IS NULL AND is_deleted = 0 \
             ORDER BY completed_at IS NOT NULL ASC, due_date IS NULL ASC, due_date ASC, \
                      created_at DESC, id DESC",
        )
    } else {
        select_sql(
            "WHERE project_id = ?1 AND is_todo = 0 AND is_deleted = 0 \
             ORDER BY day DESC, created_at DESC, id DESC",
        )
    };
    sqlx::query_as::<_, Entry>(&sql)
        .bind(project_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("读取项目内容失败：{e}"))
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_projects(
    state: State<'_, AppState>,
    include_archived: bool,
) -> Result<Vec<Project>, String> {
    let pool = state.pool.read().await;
    list_projects_core(&pool, include_archived, &today_string()).await
}

#[tauri::command]
pub async fn create_project(
    state: State<'_, AppState>,
    name: String,
    color: Option<String>,
    description: Option<String>,
) -> Result<Project, String> {
    let pool = state.pool.read().await;
    create_project_core(&pool, &name, color, description).await
}

#[tauri::command]
pub async fn update_project(
    state: State<'_, AppState>,
    id: i64,
    patch: ProjectPatch,
) -> Result<Project, String> {
    let pool = state.pool.read().await;
    update_project_core(&pool, id, patch).await
}

#[tauri::command]
pub async fn delete_project(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let pool = state.pool.read().await;
    delete_project_core(&pool, id).await
}

#[tauri::command]
pub async fn restore_project(state: State<'_, AppState>, id: i64) -> Result<Project, String> {
    let pool = state.pool.read().await;
    restore_project_core(&pool, id).await
}

#[tauri::command]
pub async fn list_project_entries(
    state: State<'_, AppState>,
    project_id: i64,
    only_tasks: bool,
) -> Result<Vec<Entry>, String> {
    let pool = state.pool.read().await;
    list_project_entries_core(&pool, project_id, only_tasks).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::entries::{create_entry_full_core, set_entry_completed_core};
    use crate::db::models::EntryPatch;
    use crate::db::{cleanup_test_pool, test_pool};

    async fn add_project(pool: &SqlitePool, name: &str) -> Project {
        create_project_core(pool, name, Some("indigo".into()), None)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn create_update_and_list_projects() {
        let (pool, dir) = test_pool().await;
        let first = add_project(&pool, "机器人风险系统").await;
        let second = create_project_core(
            &pool,
            "数据抓取平台",
            Some("teal".into()),
            Some("抓取任务".into()),
        )
        .await
        .unwrap();
        assert_eq!(first.name, "机器人风险系统");
        assert_eq!(first.color.as_deref(), Some("indigo"));
        assert_eq!(first.status, "active");
        assert_eq!(first.sort_order, 0);
        assert_eq!(first.task_total, 0);
        assert_eq!(first.note_total, 0);
        assert_eq!(second.description.as_deref(), Some("抓取任务"));

        // 按 sort_order, created_at 排序
        let list = list_projects_core(&pool, false, &today_string())
            .await
            .unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, first.id);

        // 改名 + 颜色 + 状态 + 排序
        let mut patch = ProjectPatch::default();
        patch.name = Some("机器人风险系统 v2".into());
        patch.color = Some(Some("rose".into()));
        patch.status = Some("paused".into());
        patch.sort_order = Some(5);
        let updated = update_project_core(&pool, first.id, patch).await.unwrap();
        assert_eq!(updated.name, "机器人风险系统 v2");
        assert_eq!(updated.color.as_deref(), Some("rose"));
        assert_eq!(updated.status, "paused");
        assert_eq!(updated.sort_order, 5);

        // 列表按新 sort_order 重排
        let list = list_projects_core(&pool, false, &today_string())
            .await
            .unwrap();
        assert_eq!(list[0].id, second.id);
        assert_eq!(list[1].id, first.id);

        // 非法状态
        let mut patch = ProjectPatch::default();
        patch.status = Some("nope".into());
        assert!(update_project_core(&pool, first.id, patch).await.is_err());
        // 非法颜色
        let mut patch = ProjectPatch::default();
        patch.color = Some(Some("rainbow".into()));
        assert!(update_project_core(&pool, first.id, patch).await.is_err());
        // 空名称
        let mut patch = ProjectPatch::default();
        patch.name = Some("   ".into());
        assert!(update_project_core(&pool, first.id, patch).await.is_err());

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn create_project_rejects_empty_name_and_bad_color() {
        let (pool, dir) = test_pool().await;
        let err = create_project_core(&pool, "  ", None, None)
            .await
            .unwrap_err();
        assert!(err.contains("不能为空"));
        let err = create_project_core(&pool, "项目", Some("rainbow".into()), None)
            .await
            .unwrap_err();
        assert!(err.contains("颜色"));

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn archived_projects_hidden_unless_requested() {
        let (pool, dir) = test_pool().await;
        let active = add_project(&pool, "进行中").await;
        let archived = add_project(&pool, "已归档").await;
        let mut patch = ProjectPatch::default();
        patch.status = Some("archived".into());
        update_project_core(&pool, archived.id, patch).await.unwrap();

        let visible = list_projects_core(&pool, false, &today_string())
            .await
            .unwrap();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, active.id);

        let all = list_projects_core(&pool, true, &today_string())
            .await
            .unwrap();
        assert_eq!(all.len(), 2);

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn project_stats_only_count_top_level_tasks() {
        let (pool, dir) = test_pool().await;
        let project = add_project(&pool, "统计项目").await;
        let today = today_string();

        // 两个顶层任务：一个未完成逾期、一个已完成
        let overdue = create_entry_full_core(
            &pool,
            "逾期任务",
            true,
            Some("2000-01-01".into()),
            None,
            Some(project.id),
        )
        .await
        .unwrap();
        let done = create_entry_full_core(
            &pool,
            "完成任务",
            true,
            Some(today.clone()),
            None,
            Some(project.id),
        )
        .await
        .unwrap();
        set_entry_completed_core(&pool, done.id, true).await.unwrap();
        // 一条记录
        create_entry_full_core(&pool, "项目记录", false, None, None, Some(project.id))
            .await
            .unwrap();

        // 给逾期任务加一个子任务（子任务不计入项目统计）
        crate::commands::children::create_child_core(&pool, overdue.id, "子任务")
            .await
            .unwrap();

        let project = select_project(&pool, project.id, &today)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(project.task_total, 2, "只算顶层任务，子任务不计入");
        assert_eq!(project.task_done, 1);
        assert_eq!(project.task_overdue, 1);
        assert_eq!(project.note_total, 1);

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn delete_project_keeps_entries_and_clears_project() {
        let (pool, dir) = test_pool().await;
        let project = add_project(&pool, "待删除项目").await;
        let task = create_entry_full_core(&pool, "任务", true, None, None, Some(project.id))
            .await
            .unwrap();
        let child = crate::commands::children::create_child_core(&pool, task.id, "子任务")
            .await
            .unwrap();
        let note = create_entry_full_core(&pool, "记录", false, None, None, Some(project.id))
            .await
            .unwrap();

        delete_project_core(&pool, project.id).await.unwrap();

        // 项目被软删除
        let projects = list_projects_core(&pool, true, &today_string())
            .await
            .unwrap();
        assert!(projects.is_empty(), "软删除的项目不在任何列表里");

        // 任务与记录都还在，只是 project_id 变 NULL
        for id in [task.id, child.id, note.id] {
            let entry = crate::commands::entries::select_entry(&pool, id)
                .await
                .unwrap()
                .expect("任务不应被删除");
            assert!(entry.project_id.is_none());
            assert!(!entry.is_deleted);
        }

        // 项目已删除时不能重新归属
        let mut patch = EntryPatch::default();
        patch.project_id = Some(Some(project.id));
        let err = crate::commands::entries::update_entry_core(&pool, task.id, patch)
            .await
            .unwrap_err();
        assert!(err.contains("项目不存在"));

        // 恢复项目后可重新归属
        restore_project_core(&pool, project.id).await.unwrap();
        let mut patch = EntryPatch::default();
        patch.project_id = Some(Some(project.id));
        let relinked = crate::commands::entries::update_entry_core(&pool, task.id, patch)
            .await
            .unwrap();
        assert_eq!(relinked.project_id, Some(project.id));

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn list_project_entries_tasks_and_notes() {
        let (pool, dir) = test_pool().await;
        let project = add_project(&pool, "项目页").await;
        let other = add_project(&pool, "别的项目").await;

        let unfinished = create_entry_full_core(
            &pool,
            "未完成任务",
            true,
            Some("2030-01-01".into()),
            None,
            Some(project.id),
        )
        .await
        .unwrap();
        let finished =
            create_entry_full_core(&pool, "已完成任务", true, None, None, Some(project.id))
                .await
                .unwrap();
        set_entry_completed_core(&pool, finished.id, true).await.unwrap();
        let _note = create_entry_full_core(&pool, "项目记录", false, None, None, Some(project.id))
            .await
            .unwrap();
        // 别的项目的任务不应出现
        create_entry_full_core(&pool, "别的任务", true, None, None, Some(other.id))
            .await
            .unwrap();
        // 无项目任务不应出现
        create_entry_full_core(&pool, "无项目任务", true, None, None, None)
            .await
            .unwrap();
        // 子任务不应出现在项目任务列表里
        crate::commands::children::create_child_core(&pool, unfinished.id, "子任务")
            .await
            .unwrap();

        let tasks = list_project_entries_core(&pool, project.id, true)
            .await
            .unwrap();
        let ids: Vec<i64> = tasks.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![unfinished.id, finished.id], "未完成在前");
        assert!(tasks.iter().all(|e| e.parent_id.is_none()));

        let notes = list_project_entries_core(&pool, project.id, false)
            .await
            .unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].content, "项目记录");
        assert_eq!(notes[0].project_name.as_deref(), Some("项目页"));

        // 项目不存在
        assert!(list_project_entries_core(&pool, 9999, true).await.is_err());

        cleanup_test_pool(pool, dir).await;
    }
}
