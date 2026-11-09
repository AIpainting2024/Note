//! 子任务（任务层级）命令。
//!
//! 契约见 `docs/接口契约-v0.4.md` 第 1/3 节：
//! - `parent_id` 只能指向 `is_todo=1 AND parent_id IS NULL AND is_deleted=0` 的记录（禁止三级嵌套）；
//! - 子任务强制 `is_todo=1`，`day` = 今天，继承父任务 `project_id`；
//! - `create_child` 不接受截止时间（子任务不带截止/提醒）；
//! - `list_children` 含已完成，按 `sort_order, created_at`；
//! - `reorder_children` 校验 ids 全属该父任务，按数组顺序写 `sort_order`。

use std::collections::HashSet;

use sqlx::SqlitePool;
use tauri::State;

use crate::commands::entries::{normalize_content, select_entry, select_sql};
use crate::db::models::Entry;
use crate::db::{now_timestamp, today_string};
use crate::AppState;

/// 父任务校验：存在、未删除、是待办、且自身没有父任务（禁止三级嵌套）。
async fn ensure_can_have_children(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    parent_id: i64,
) -> Result<Entry, String> {
    let parent = select_entry(&mut **tx, parent_id)
        .await?
        .ok_or_else(|| "父任务不存在或已被删除".to_string())?;
    if parent.is_deleted {
        return Err("父任务不存在或已被删除".to_string());
    }
    if parent.parent_id.is_some() {
        return Err("子任务下不能再添加子任务（最多支持两级任务）".to_string());
    }
    if !parent.is_todo {
        return Err("只有待办任务可以添加子任务".to_string());
    }
    Ok(parent)
}

/// 子任务列表：含已完成。v0.9 起已完成沉底，已完成内部按完成时间倒序；
/// 未完成子任务保持手动拖动顺序（`sort_order`）。
pub async fn list_children_core(pool: &SqlitePool, parent_id: i64) -> Result<Vec<Entry>, String> {
    let sql = select_sql(
        "WHERE parent_id = ?1 AND is_deleted = 0 ORDER BY sort_order, created_at, id",
    );
    let mut children = sqlx::query_as::<_, Entry>(&sql)
        .bind(parent_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("读取子任务失败：{e}"))?;
    crate::commands::entries::sort_completed_last(&mut children);
    Ok(children)
}

/// 新增子任务：强制 `is_todo=1`、`day`=今天、继承父任务 `project_id`，排在最后。
pub async fn create_child_core(
    pool: &SqlitePool,
    parent_id: i64,
    content: &str,
) -> Result<Entry, String> {
    let content = normalize_content(content)?;
    let day = today_string();
    let now = now_timestamp();

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("数据库事务开启失败：{e}"))?;
    let parent = ensure_can_have_children(&mut tx, parent_id).await?;

    // 新子任务排在已有子任务之后（正在拖拽排序时也成立）。
    let next_sort_order: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM entries WHERE parent_id = ?1",
    )
    .bind(parent_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| format!("读取子任务排序失败：{e}"))?;

    let result = sqlx::query(
        "INSERT INTO entries \
         (day, content, is_todo, due_date, due_time, completed_at, is_pinned, is_deleted, \
          project_id, parent_id, sort_order, created_at, updated_at) \
         VALUES (?1, ?2, 1, NULL, NULL, NULL, 0, 0, ?3, ?4, ?5, ?6, ?6)",
    )
    .bind(&day)
    .bind(&content)
    .bind(parent.project_id)
    .bind(parent_id)
    .bind(next_sort_order)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("保存子任务失败：{e}"))?;

    let id = result.last_insert_rowid();
    // 新增子任务后父任务必然「未完成」，这里把父任务的完成态同步回去（v0.8）
    crate::commands::entries::sync_parent_completion(&mut tx, parent_id, &now).await?;
    let entry = select_entry(&mut *tx, id)
        .await?
        .ok_or_else(|| "保存子任务失败：写入后无法读回该记录".to_string())?;
    tx.commit()
        .await
        .map_err(|e| format!("保存子任务失败：{e}"))?;
    Ok(entry)
}

/// 拖动排序：校验 ids 全属该父任务，按数组顺序写 `sort_order`（从 0 开始）。
pub async fn reorder_children_core(
    pool: &SqlitePool,
    parent_id: i64,
    ordered_ids: Vec<i64>,
) -> Result<(), String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("数据库事务开启失败：{e}"))?;
    let parent = select_entry(&mut *tx, parent_id)
        .await?
        .ok_or_else(|| "父任务不存在或已被删除".to_string())?;
    if parent.is_deleted {
        return Err("父任务不存在或已被删除".to_string());
    }

    // 该父任务当前的子任务集合（用于校验 ids 是否全部归属于它）。
    let existing: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM entries WHERE parent_id = ?1 AND is_deleted = 0")
            .bind(parent_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| format!("读取子任务失败：{e}"))?;
    let existing: HashSet<i64> = existing.into_iter().collect();

    let mut seen = HashSet::new();
    for id in &ordered_ids {
        if !existing.contains(id) {
            return Err(format!(
                "子任务排序失败：{id} 不是该任务的子任务"
            ));
        }
        if !seen.insert(*id) {
            return Err(format!("子任务排序失败：{id} 重复出现"));
        }
    }

    let now = now_timestamp();
    for (index, id) in ordered_ids.iter().enumerate() {
        sqlx::query(
            "UPDATE entries SET sort_order = ?1, updated_at = ?2 \
             WHERE id = ?3 AND parent_id = ?4",
        )
        .bind(index as i64)
        .bind(&now)
        .bind(id)
        .bind(parent_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("更新子任务排序失败：{e}"))?;
    }
    tx.commit()
        .await
        .map_err(|e| format!("更新子任务排序失败：{e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_children(
    state: State<'_, AppState>,
    parent_id: i64,
) -> Result<Vec<Entry>, String> {
    let pool = state.pool.read().await;
    list_children_core(&pool, parent_id).await
}

#[tauri::command]
pub async fn create_child(
    state: State<'_, AppState>,
    parent_id: i64,
    content: String,
) -> Result<Entry, String> {
    let pool = state.pool.read().await;
    create_child_core(&pool, parent_id, &content).await
}

#[tauri::command]
pub async fn reorder_children(
    state: State<'_, AppState>,
    parent_id: i64,
    ordered_ids: Vec<i64>,
) -> Result<(), String> {
    let pool = state.pool.read().await;
    reorder_children_core(&pool, parent_id, ordered_ids).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::entries::{
        create_entry_core, create_entry_full_core, delete_entry_core, restore_entry_core,
        set_entry_completed_core, update_entry_core,
    };
    use crate::db::models::EntryPatch;
    use crate::db::{cleanup_test_pool, test_pool};

    async fn add_parent(pool: &SqlitePool, content: &str) -> Entry {
        create_entry_core(pool, content, true, None).await.unwrap()
    }

    #[tokio::test]
    async fn create_child_inherits_project_and_forces_todo() {
        let (pool, dir) = test_pool().await;
        let project = crate::commands::projects::create_project_core(
            &pool,
            "项目A",
            None,
            None,
        )
        .await
        .unwrap();
        let parent = create_entry_full_core(&pool, "父任务", true, None, None, Some(project.id))
            .await
            .unwrap();

        let child = create_child_core(&pool, parent.id, "  子任务一  ")
            .await
            .unwrap();
        assert_eq!(child.content, "子任务一");
        assert_eq!(child.parent_id, Some(parent.id));
        assert!(child.is_todo, "子任务强制 is_todo=1");
        assert_eq!(child.project_id, Some(project.id), "继承父任务的项目");
        assert_eq!(child.day, today_string());
        assert_eq!(child.due_date, None);
        assert_eq!(child.due_time, None);

        // 父任务上的进度字段
        let parent = select_entry(&pool, parent.id).await.unwrap().unwrap();
        assert_eq!(parent.child_total, Some(1));
        assert_eq!(parent.child_done, Some(0));

        let second = create_child_core(&pool, parent.id, "子任务二").await.unwrap();
        let sort_order: i64 =
            sqlx::query_scalar("SELECT sort_order FROM entries WHERE id = ?1")
                .bind(second.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(sort_order, 1, "新子任务排在最后");

        // 内容不能为空
        let err = create_child_core(&pool, parent.id, "   ").await.unwrap_err();
        assert!(err.contains("不能为空"));

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn child_due_time_is_rejected_with_chinese_error() {
        let (pool, dir) = test_pool().await;
        let parent = add_parent(&pool, "带日期的任务").await;
        let child = create_child_core(&pool, parent.id, "子任务").await.unwrap();

        let mut patch = EntryPatch::default();
        patch.due_date = Some(Some("2026-12-01".into()));
        let err = update_entry_core(&pool, child.id, patch).await.unwrap_err();
        assert_eq!(err, "子任务不支持截止时间");

        let mut patch = EntryPatch::default();
        patch.due_time = Some(Some("09:00".into()));
        let err = update_entry_core(&pool, child.id, patch).await.unwrap_err();
        assert_eq!(err, "子任务不支持截止时间");

        // 子任务也不能改成普通记录
        let mut patch = EntryPatch::default();
        patch.is_todo = Some(false);
        assert!(update_entry_core(&pool, child.id, patch).await.is_err());

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn three_level_nesting_is_rejected() {
        let (pool, dir) = test_pool().await;
        let parent = add_parent(&pool, "父任务").await;
        let child = create_child_core(&pool, parent.id, "子任务").await.unwrap();

        let err = create_child_core(&pool, child.id, "孙任务").await.unwrap_err();
        assert!(
            err.contains("子任务下不能再添加子任务"),
            "错误信息应为中文：{err}"
        );

        // 父任务必须是待办：普通记录不能有子任务
        let note = create_entry_core(&pool, "普通记录", false, None).await.unwrap();
        let err = create_child_core(&pool, note.id, "子任务").await.unwrap_err();
        assert!(err.contains("只有待办任务"));

        // 父任务不存在
        let err = create_child_core(&pool, 9999, "子任务").await.unwrap_err();
        assert!(err.contains("不存在"));

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn parent_delete_and_restore_cascade_children() {
        let (pool, dir) = test_pool().await;
        let parent = create_entry_full_core(&pool, "父任务", true, None, None, None)
            .await
            .unwrap();
        let first = create_child_core(&pool, parent.id, "子一").await.unwrap();
        let second = create_child_core(&pool, parent.id, "子二").await.unwrap();

        delete_entry_core(&pool, parent.id).await.unwrap();
        for child in [&first, &second] {
            let row = select_entry(&pool, child.id).await.unwrap().unwrap();
            assert!(row.is_deleted, "父任务删除时子任务一并软删除");
        }
        // 回收站只列父任务，不单独列子任务
        let trash = crate::commands::entries::list_trash_core(&pool).await.unwrap();
        assert_eq!(trash.len(), 1);
        assert_eq!(trash[0].id, parent.id);

        restore_entry_core(&pool, parent.id).await.unwrap();
        for child in [&first, &second] {
            let row = select_entry(&pool, child.id).await.unwrap().unwrap();
            assert!(!row.is_deleted, "父任务恢复时子任务一并恢复");
        }
        assert_eq!(
            list_children_core(&pool, parent.id).await.unwrap().len(),
            2
        );

        // 彻底删除父任务时子任务行一并物理删除（否则外键约束会失败）
        delete_entry_core(&pool, parent.id).await.unwrap();
        crate::commands::entries::purge_entry_core(&pool, parent.id)
            .await
            .unwrap();
        for child in [&first, &second] {
            assert!(select_entry(&pool, child.id).await.unwrap().is_none());
        }

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn parent_project_change_syncs_children() {
        let (pool, dir) = test_pool().await;
        let first = crate::commands::projects::create_project_core(&pool, "项目一", None, None)
            .await
            .unwrap();
        let second = crate::commands::projects::create_project_core(&pool, "项目二", None, None)
            .await
            .unwrap();
        let parent = create_entry_full_core(&pool, "父任务", true, None, None, Some(first.id))
            .await
            .unwrap();
        let child = create_child_core(&pool, parent.id, "子任务").await.unwrap();
        assert_eq!(child.project_id, Some(first.id));

        let mut patch = EntryPatch::default();
        patch.project_id = Some(Some(second.id));
        update_entry_core(&pool, parent.id, patch).await.unwrap();

        let child = select_entry(&pool, child.id).await.unwrap().unwrap();
        assert_eq!(child.project_id, Some(second.id), "子任务跟随父任务改项目");

        // 清空项目也应同步
        let mut patch = EntryPatch::default();
        patch.project_id = Some(None);
        update_entry_core(&pool, parent.id, patch).await.unwrap();
        let child = select_entry(&pool, child.id).await.unwrap().unwrap();
        assert_eq!(child.project_id, None);

        // 子任务不能单独改项目
        let mut patch = EntryPatch::default();
        patch.project_id = Some(Some(first.id));
        let err = update_entry_core(&pool, child.id, patch).await.unwrap_err();
        assert!(err.contains("跟随父任务"), "错误信息应为中文：{err}");

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn reorder_children_writes_order_and_rejects_foreign_ids() {
        let (pool, dir) = test_pool().await;
        let parent = add_parent(&pool, "父任务").await;
        let other = add_parent(&pool, "另一个父任务").await;
        let first = create_child_core(&pool, parent.id, "子一").await.unwrap();
        let second = create_child_core(&pool, parent.id, "子二").await.unwrap();
        let third = create_child_core(&pool, parent.id, "子三").await.unwrap();

        reorder_children_core(&pool, parent.id, vec![third.id, first.id, second.id])
            .await
            .unwrap();
        let list = list_children_core(&pool, parent.id).await.unwrap();
        let ids: Vec<i64> = list.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![third.id, first.id, second.id]);

        // 不支持部分重排的越权 id：别的父任务下的子任务
        let foreign = create_child_core(&pool, other.id, "别人的子任务").await.unwrap();
        let err = reorder_children_core(&pool, parent.id, vec![first.id, foreign.id])
            .await
            .unwrap_err();
        assert!(err.contains("不是该任务的子任务"), "错误信息应为中文：{err}");

        // 重复 id
        let err = reorder_children_core(&pool, parent.id, vec![first.id, first.id])
            .await
            .unwrap_err();
        assert!(err.contains("重复"));

        // 父任务不存在
        assert!(reorder_children_core(&pool, 9999, vec![]).await.is_err());

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn list_children_includes_completed_and_excludes_deleted() {
        let (pool, dir) = test_pool().await;
        let parent = add_parent(&pool, "父任务").await;
        let active = create_child_core(&pool, parent.id, "未完成").await.unwrap();
        let done = create_child_core(&pool, parent.id, "已完成").await.unwrap();
        let removed = create_child_core(&pool, parent.id, "已删除").await.unwrap();
        set_entry_completed_core(&pool, done.id, true).await.unwrap();
        delete_entry_core(&pool, removed.id).await.unwrap();

        let list = list_children_core(&pool, parent.id).await.unwrap();
        let ids: Vec<i64> = list.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![active.id, done.id], "含已完成，不含已删除");

        let parent = select_entry(&pool, parent.id).await.unwrap().unwrap();
        assert_eq!(parent.child_total, Some(2));
        assert_eq!(parent.child_done, Some(1));

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn task_with_children_cannot_become_note() {
        let (pool, dir) = test_pool().await;
        let parent = add_parent(&pool, "父任务").await;
        create_child_core(&pool, parent.id, "子任务").await.unwrap();

        let mut patch = EntryPatch::default();
        patch.is_todo = Some(false);
        let err = update_entry_core(&pool, parent.id, patch).await.unwrap_err();
        assert!(err.contains("还有子任务"), "错误信息应为中文：{err}");

        // 没有子任务的任务可以转成记录
        let lone = add_parent(&pool, "单任务").await;
        let mut patch = EntryPatch::default();
        patch.is_todo = Some(false);
        let converted = update_entry_core(&pool, lone.id, patch).await.unwrap();
        assert!(!converted.is_todo);

        cleanup_test_pool(pool, dir).await;
    }
    #[tokio::test]
    async fn parent_completion_follows_children() {
        let (pool, dir) = test_pool().await;
        let parent = add_parent(&pool, "父任务").await;
        let child = create_child_core(&pool, parent.id, "子任务一").await.unwrap();

        // 有未完成子任务时，父任务不允许打勾
        let err = set_entry_completed_core(&pool, parent.id, true)
            .await
            .unwrap_err();
        assert!(err.contains("子任务"), "应提示先完成子任务，实际：{err}");

        // 子任务全部完成 → 父任务自动打勾
        set_entry_completed_core(&pool, child.id, true).await.unwrap();
        let parent_after = select_entry(&pool, parent.id).await.unwrap().unwrap();
        assert!(
            parent_after.completed_at.is_some(),
            "子任务全部完成后父任务应自动完成"
        );

        // 取消一个子任务 → 父任务自动回到未完成
        set_entry_completed_core(&pool, child.id, false).await.unwrap();
        let parent_after = select_entry(&pool, parent.id).await.unwrap().unwrap();
        assert!(
            parent_after.completed_at.is_none(),
            "有子任务未完成时父任务应为未完成"
        );

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn adding_child_reopens_completed_parent() {
        let (pool, dir) = test_pool().await;
        let parent = add_parent(&pool, "父任务").await;

        // 没有子任务时可以直接完成
        let done = set_entry_completed_core(&pool, parent.id, true).await.unwrap();
        assert!(done.completed_at.is_some());

        // 新增子任务后父任务回到未完成
        create_child_core(&pool, parent.id, "新子任务").await.unwrap();
        let parent_after = select_entry(&pool, parent.id).await.unwrap().unwrap();
        assert!(
            parent_after.completed_at.is_none(),
            "新增子任务后父任务应回到未完成"
        );

        cleanup_test_pool(pool, dir).await;
    }
}
