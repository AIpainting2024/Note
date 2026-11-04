//! 待办视图与统计命令。
//!
//! 分组语义见 docs/接口契约-v1.md 第 4 节「语义细则」：
//! - pending：未完成全部，按 due_date 升序（空值排后）、再按 created_at 升序
//! - dueToday：due_date = 今天且未完成
//! - overdue：due_date < 今天且未完成（**无 due_date 永不算逾期**）
//! - done：已完成，按 completed_at 倒序，限 200 条
//!
//! v0.4（契约第 4 节）：四个分组全部只返回**顶层任务**（`parent_id IS NULL`），
//! 子任务只在任务详情侧栏出现，不进入待办/今日列表。

use sqlx::SqlitePool;
use tauri::State;

use crate::commands::entries::select_sql;
use crate::db::models::{Counts, Entry};
use crate::db::today_string;
use crate::AppState;

pub async fn list_todos_core(pool: &SqlitePool, group: &str) -> Result<Vec<Entry>, String> {
    let sql = match group {
        // v0.4 契约第 5 节：顶层任务列表排序 = 置顶优先 → 手工排序 → 截止日期/时间 → 创建倒序。
        "pending" => select_sql(
            "WHERE is_todo = 1 AND is_deleted = 0 AND completed_at IS NULL AND parent_id IS NULL \
             ORDER BY is_pinned DESC, sort_order ASC, due_date IS NULL, due_date, \
                      due_time IS NULL, due_time, created_at DESC, id DESC",
        ),
        "dueToday" => {
            let sql = select_sql(
                "WHERE is_todo = 1 AND is_deleted = 0 AND completed_at IS NULL \
                   AND parent_id IS NULL AND due_date = ?1 \
                 ORDER BY due_time IS NULL ASC, due_time ASC, created_at ASC, id ASC",
            );
            return sqlx::query_as::<_, Entry>(&sql)
                .bind(today_string())
                .fetch_all(pool)
                .await
                .map_err(|e| format!("读取今日到期待办失败：{e}"));
        }
        "overdue" => {
            let sql = select_sql(
                "WHERE is_todo = 1 AND is_deleted = 0 AND completed_at IS NULL \
                   AND parent_id IS NULL \
                   AND due_date IS NOT NULL AND due_date < ?1 \
                 ORDER BY due_date ASC, due_time IS NULL ASC, due_time ASC, created_at ASC, id ASC",
            );
            return sqlx::query_as::<_, Entry>(&sql)
                .bind(today_string())
                .fetch_all(pool)
                .await
                .map_err(|e| format!("读取逾期待办失败：{e}"));
        }
        "done" => select_sql(
            "WHERE is_todo = 1 AND is_deleted = 0 AND completed_at IS NOT NULL \
               AND parent_id IS NULL \
             ORDER BY completed_at DESC, id DESC LIMIT 200",
        ),
        other => {
            return Err(format!(
                "未知的待办分组：{other}（可选值 pending / dueToday / overdue / done）"
            ))
        }
    };

    sqlx::query_as::<_, Entry>(&sql)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("读取 {group} 待办失败：{e}"))
}

/// 统计口径见契约第 4 节。v0.4：全部只统计顶层记录（子任务不重复计数）。
pub async fn get_counts_core(pool: &SqlitePool, today: &str) -> Result<Counts, String> {
    let sql = "\
        SELECT \
          (SELECT COUNT(*) FROM entries \
             WHERE day = ?1 AND is_deleted = 0 AND parent_id IS NULL) AS today_entries, \
          (SELECT COUNT(*) FROM entries \
             WHERE day = ?1 AND is_todo = 1 AND is_deleted = 0 AND parent_id IS NULL) AS today_todos, \
          (SELECT COUNT(*) FROM entries \
             WHERE is_todo = 1 AND is_deleted = 0 AND completed_at IS NULL \
               AND parent_id IS NULL) AS pending_todos, \
          (SELECT COUNT(*) FROM entries \
             WHERE is_todo = 1 AND is_deleted = 0 AND completed_at IS NULL \
               AND parent_id IS NULL AND due_date = ?1) AS due_today_todos, \
          (SELECT COUNT(*) FROM entries \
             WHERE is_todo = 1 AND is_deleted = 0 AND completed_at IS NULL \
               AND parent_id IS NULL \
               AND due_date IS NOT NULL AND due_date < ?1) AS overdue_todos, \
          (SELECT COUNT(*) FROM entries \
             WHERE is_deleted = 0 AND completed_at IS NOT NULL AND parent_id IS NULL \
               AND substr(completed_at, 1, 10) = ?1) AS done_today";

    sqlx::query_as::<_, Counts>(sql)
        .bind(today)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("读取统计数据失败：{e}"))
}

#[tauri::command]
pub async fn list_todos(state: State<'_, AppState>, group: String) -> Result<Vec<Entry>, String> {
    let pool = state.pool.read().await;
    list_todos_core(&pool, &group).await
}

#[tauri::command]
pub async fn get_counts(state: State<'_, AppState>) -> Result<Counts, String> {
    let pool = state.pool.read().await;
    get_counts_core(&pool, &today_string()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::entries::{
        create_entry_core, create_entry_with_due_time_core, delete_entry_core,
        set_entry_completed_core,
    };
    use crate::db::{cleanup_test_pool, test_pool};

    async fn add_todo(
        pool: &SqlitePool,
        content: &str,
        due: Option<&str>,
    ) -> crate::db::models::Entry {
        create_entry_core(pool, content, true, due.map(|s| s.to_string()))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn pending_order_puts_due_first_and_null_last() {
        let (pool, dir) = test_pool().await;
        let no_due = add_todo(&pool, "无截止", None).await;
        let later = add_todo(&pool, "晚一点", Some("2026-12-31")).await;
        let sooner = add_todo(&pool, "早一点", Some("2026-11-10")).await;

        let pending = list_todos_core(&pool, "pending").await.unwrap();
        assert_eq!(pending.len(), 3);
        assert_eq!(pending[0].id, sooner.id);
        assert_eq!(pending[1].id, later.id);
        assert_eq!(pending[2].id, no_due.id);

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn overdue_never_includes_entries_without_due_date() {
        let (pool, dir) = test_pool().await;
        let with_due = add_todo(&pool, "逾期", Some("2000-01-01")).await;
        let no_due = add_todo(&pool, "无截止", None).await;
        // 已完成的旧待办不算逾期
        let done_old = add_todo(&pool, "已完成旧待办", Some("2000-01-01")).await;
        set_entry_completed_core(&pool, done_old.id, true).await.unwrap();
        // 已删除的待办不算逾期
        let deleted = add_todo(&pool, "已删除", Some("2000-01-01")).await;
        delete_entry_core(&pool, deleted.id).await.unwrap();

        let overdue = list_todos_core(&pool, "overdue").await.unwrap();
        assert_eq!(overdue.len(), 1);
        assert_eq!(overdue[0].id, with_due.id);
        assert!(!overdue.iter().any(|e| e.id == no_due.id));

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn due_today_and_done_groups() {
        let (pool, dir) = test_pool().await;
        let today = today_string();
        let due_today = add_todo(&pool, "今天到期", Some(&today)).await;
        let done = add_todo(&pool, "完成它", None).await;
        set_entry_completed_core(&pool, done.id, true).await.unwrap();

        let today_list = list_todos_core(&pool, "dueToday").await.unwrap();
        assert_eq!(today_list.len(), 1);
        assert_eq!(today_list[0].id, due_today.id);

        let done_list = list_todos_core(&pool, "done").await.unwrap();
        assert_eq!(done_list.len(), 1);
        assert_eq!(done_list[0].id, done.id);

        let err = list_todos_core(&pool, "nope").await.unwrap_err();
        assert!(err.contains("未知的待办分组"));

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn pending_order_puts_due_time_before_date_only() {
        let (pool, dir) = test_pool().await;
        let later = add_todo(&pool, "晚日期", Some("2026-12-31")).await;
        let no_time = add_todo(&pool, "同一天无时间", Some("2026-11-10")).await;
        let afternoon = create_entry_with_due_time_core(
            &pool,
            "同一天 15:00",
            true,
            Some("2026-11-10".into()),
            Some("15:00".into()),
        )
        .await
        .unwrap();
        let morning = create_entry_with_due_time_core(
            &pool,
            "同一天 09:00",
            true,
            Some("2026-11-10".into()),
            Some("09:00".into()),
        )
        .await
        .unwrap();

        let pending = list_todos_core(&pool, "pending").await.unwrap();
        let ids: Vec<i64> = pending.iter().map(|e| e.id).collect();
        assert_eq!(
            ids,
            vec![morning.id, afternoon.id, no_time.id, later.id],
            "先按日期，再按时间升序，只有日期的排在当天最后"
        );

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn counts_follow_contract() {
        let (pool, dir) = test_pool().await;
        let today = today_string();
        let _entry = create_entry_core(&pool, "普通记录", false, None).await.unwrap();
        add_todo(&pool, "今天到期", Some(&today)).await;
        add_todo(&pool, "逾期", Some("2000-01-01")).await;
        add_todo(&pool, "无截止", None).await;
        let done_today = add_todo(&pool, "今天完成", None).await;
        set_entry_completed_core(&pool, done_today.id, true).await.unwrap();

        let counts = get_counts_core(&pool, &today).await.unwrap();
        assert_eq!(counts.today_entries, 5);
        assert_eq!(counts.today_todos, 4);
        assert_eq!(counts.pending_todos, 3); // 今天到期 + 逾期 + 无截止
        assert_eq!(counts.due_today_todos, 1);
        assert_eq!(counts.overdue_todos, 1);
        assert_eq!(counts.done_today, 1);

        cleanup_test_pool(pool, dir).await;
    }

    // -----------------------------------------------------------------------
    // v0.4：子任务不进入待办/今日列表，也不参与统计
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn todo_groups_and_counts_exclude_children() {
        let (pool, dir) = test_pool().await;
        let today = today_string();
        // 父任务：今天到期
        let parent = add_todo(&pool, "父任务", Some(&today)).await;
        let child = crate::commands::children::create_child_core(&pool, parent.id, "子任务")
            .await
            .unwrap();
        // 子任务本不该有截止时间；直接改库模拟脏数据，确认各分组都不会带出它。
        sqlx::query(
            "UPDATE entries SET due_date = '2000-01-01', due_time = '08:00' WHERE id = ?1",
        )
        .bind(child.id)
        .execute(&pool)
        .await
        .unwrap();

        for group in ["pending", "dueToday", "overdue", "done"] {
            let todos = list_todos_core(&pool, group).await.unwrap();
            assert!(
                todos.iter().all(|e| e.id != child.id),
                "分组 {group} 不应包含子任务"
            );
        }

        let counts = get_counts_core(&pool, &today).await.unwrap();
        assert_eq!(counts.today_entries, 1);
        assert_eq!(counts.today_todos, 1);
        assert_eq!(counts.pending_todos, 1);
        assert_eq!(counts.due_today_todos, 1);
        assert_eq!(counts.overdue_todos, 0);
        assert_eq!(counts.done_today, 0);

        cleanup_test_pool(pool, dir).await;
    }
}
