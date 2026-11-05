//! 视图类查询（v0.7）：日历区间、收件箱。
//!
//! - 日历：格子按「录入日期（day）」+「截止日期（due_date）」两个维度取数，
//!   以录入时间为主；**不含子任务**（子任务只体现在父任务进度里）；
//!   保密项目的记录永远不进日历。
//! - 收件箱：没有归属项目（project_id IS NULL）的顶层记录与任务。

use tauri::State;

use crate::commands::entries::select_sql;
use crate::db::models::Entry;
use crate::AppState;

/// 日历区间取数：返回区间内「录入」或「截止」的记录/任务（顶层，不含子任务、不含保密项目）。
#[tauri::command]
pub async fn list_calendar_range(
    state: State<'_, AppState>,
    start: String,
    end: String,
) -> Result<Vec<Entry>, String> {
    let pool = state.pool.read().await;
    let sql = select_sql(
        "WHERE is_deleted = 0 AND parent_id IS NULL \
           AND COALESCE(project_is_secret, 0) = 0 \
           AND ((due_date IS NOT NULL AND due_date BETWEEN ?1 AND ?2) \
                OR (day BETWEEN ?1 AND ?2)) \
         ORDER BY day ASC, created_at ASC, id ASC",
    );
    sqlx::query_as::<_, Entry>(&sql)
        .bind(&start)
        .bind(&end)
        .fetch_all(&*pool)
        .await
        .map_err(|e| format!("读取日历失败：{e}"))
}

/// 收件箱：没有归属项目的顶层记录/任务（新的在前）。
#[tauri::command]
pub async fn list_inbox_entries(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<Entry>, String> {
    let pool = state.pool.read().await;
    let limit = limit.unwrap_or(200).clamp(1, 500);
    let sql = select_sql(
        "WHERE is_deleted = 0 AND parent_id IS NULL AND project_id IS NULL \
         ORDER BY day DESC, created_at DESC, id DESC LIMIT ?1",
    );
    sqlx::query_as::<_, Entry>(&sql)
        .bind(limit)
        .fetch_all(&*pool)
        .await
        .map_err(|e| format!("读取收件箱失败：{e}"))
}

#[cfg(test)]
mod tests {
    use crate::commands::entries::{select_sql, ENTRY_COLUMNS};

    /// 只校验 SQL 形态：派生表里的列必须都被外层引用得到（防止手滑改列名）
    #[test]
    fn calendar_sql_uses_known_columns() {
        let sql = select_sql("WHERE is_deleted = 0 AND parent_id IS NULL ORDER BY day ASC");
        assert!(sql.contains("FROM ("), "必须是派生表形式");
        assert!(sql.starts_with("SELECT "));
        for column in ["day", "due_date", "parent_id", "project_is_secret"] {
            assert!(ENTRY_COLUMNS.contains(column), "列清单里缺少 {column}");
        }
    }
}
