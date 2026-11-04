//! 四象限（v0.6）：重要/紧急两维、四个等级。
//!
//! 1 = 重要且紧急　2 = 重要不紧急　3 = 紧急不重要　4 = 不重要不紧急
//! 只针对公开任务：保密项目下的记录不进入四象限（与日历口径一致）。

use tauri::State;

use crate::commands::entries::{select_sql, select_entry};
use crate::db::models::Entry;
use crate::db::{now_timestamp, today_string};
use crate::AppState;

const VALID: [i64; 4] = [1, 2, 3, 4];

#[tauri::command]
pub async fn set_entry_quadrant(
    state: State<'_, AppState>,
    id: i64,
    quadrant: Option<i64>,
) -> Result<Entry, String> {
    if let Some(q) = quadrant {
        if !VALID.contains(&q) {
            return Err(format!("象限只能是 1~4（收到 {q}）"));
        }
    }
    let pool = state.pool.read().await;
    let now = now_timestamp();
    let result = sqlx::query(
        "UPDATE entries SET quadrant = ?1, updated_at = ?2 WHERE id = ?3 AND is_deleted = 0",
    )
    .bind(quadrant)
    .bind(&now)
    .bind(id)
    .execute(&*pool)
    .await
    .map_err(|e| format!("设置象限失败：{e}"))?;
    if result.rows_affected() == 0 {
        return Err("记录不存在或已删除".to_string());
    }
    select_entry(&*pool, id)
        .await?
        .ok_or_else(|| "设置象限后读回记录失败".to_string())
}

/// 四象限里的全部事项：已划分象限、未删除、顶层、且不属于保密项目。
#[tauri::command]
pub async fn list_quadrant_entries(state: State<'_, AppState>) -> Result<Vec<Entry>, String> {
    let pool = state.pool.read().await;
    let sql = select_sql(
        "WHERE is_deleted = 0 AND quadrant IS NOT NULL AND parent_id IS NULL \
           AND COALESCE(project_is_secret, 0) = 0 \
         ORDER BY quadrant ASC, is_pinned DESC, updated_at DESC",
    );
    sqlx::query_as::<_, Entry>(&sql)
        .fetch_all(&*pool)
        .await
        .map_err(|e| format!("读取四象限失败：{e}"))
}

/// 未划分象限的候选（公开、未完成的顶层任务），供前端“拖进象限”挑选。
#[tauri::command]
pub async fn list_unassigned_entries(state: State<'_, AppState>) -> Result<Vec<Entry>, String> {
    let pool = state.pool.read().await;
    let sql = select_sql(
        "WHERE is_deleted = 0 AND quadrant IS NULL AND parent_id IS NULL \
           AND is_todo = 1 AND completed_at IS NULL \
           AND COALESCE(project_is_secret, 0) = 0 \
         ORDER BY is_pinned DESC, updated_at DESC LIMIT 200",
    );
    let _ = today_string();
    sqlx::query_as::<_, Entry>(&sql)
        .fetch_all(&*pool)
        .await
        .map_err(|e| format!("读取待划分事项失败：{e}"))
}
