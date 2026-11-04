//! 软件内通知（通知中心）命令。
//!
//! 契约见 `docs/接口契约-v0.3.md` 第 3 节：
//! - `list` / `unread` 只看**已发出且未忽略**的通知；
//! - `mark_read` 已有 `read_at` 时不覆盖；
//! - `snooze` 不新建行，只把原行 `scheduled_at` 后移并清空 `emitted_at` / `dismissed_at`，
//!   到点后调度器会重新置 `emitted_at` 并再次广播 `notification-created`。

use chrono::Duration;
use sqlx::SqlitePool;
use tauri::State;

use crate::db::models::Notification;
use crate::db::{format_timestamp, now_datetime, now_timestamp};
use crate::AppState;

/// `notifications_list` 默认返回条数（契约第 3 节）。
pub const DEFAULT_NOTIFICATIONS_LIMIT: u32 = 50;
/// 上限，防止前端传入巨大值把整张表读出来。
pub const MAX_NOTIFICATIONS_LIMIT: u32 = 500;

/// 查询通知时统一使用的列，顺序与 `Notification` 字段一一对应。
pub const NOTIFICATION_COLUMNS: &str = "id, kind, entry_id, title, body, scheduled_at, \
                                       emitted_at, read_at, dismissed_at, created_at";

/// 通知中心列表：已发出且未忽略，按创建时间倒序（新的在前）。
pub async fn list_notifications_core(
    pool: &SqlitePool,
    limit: Option<u32>,
) -> Result<Vec<Notification>, String> {
    let limit = limit
        .unwrap_or(DEFAULT_NOTIFICATIONS_LIMIT)
        .clamp(1, MAX_NOTIFICATIONS_LIMIT) as i64;
    let sql = format!(
        "SELECT {NOTIFICATION_COLUMNS} FROM notifications \
         WHERE emitted_at IS NOT NULL AND dismissed_at IS NULL \
         ORDER BY created_at DESC, id DESC LIMIT ?1"
    );
    sqlx::query_as::<_, Notification>(&sql)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("读取通知列表失败：{e}"))
}

/// 未读数：已发出、未忽略、未读。
pub async fn unread_notifications_core(pool: &SqlitePool) -> Result<u32, String> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM notifications \
         WHERE emitted_at IS NOT NULL AND dismissed_at IS NULL AND read_at IS NULL",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| format!("读取未读通知数失败：{e}"))?;
    Ok(count.max(0) as u32)
}

/// 标记已读；`read_at` 已有值时不覆盖。
pub async fn mark_read_core(pool: &SqlitePool, id: i64) -> Result<(), String> {
    let now = now_timestamp();
    let result = sqlx::query(
        "UPDATE notifications SET read_at = COALESCE(read_at, ?1) WHERE id = ?2",
    )
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| format!("标记通知已读失败：{e}"))?;
    if result.rows_affected() == 0 {
        return Err("通知不存在或已被删除".to_string());
    }
    Ok(())
}

/// 忽略（移除）通知；重复忽略保持第一次的时间。
pub async fn dismiss_core(pool: &SqlitePool, id: i64) -> Result<(), String> {
    let now = now_timestamp();
    let result = sqlx::query(
        "UPDATE notifications SET dismissed_at = COALESCE(dismissed_at, ?1) WHERE id = ?2",
    )
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| format!("忽略通知失败：{e}"))?;
    if result.rows_affected() == 0 {
        return Err("通知不存在或已被删除".to_string());
    }
    Ok(())
}

/// 稍后提醒：把 `scheduled_at` 后移 `minutes` 分钟，清空 `emitted_at` 与 `dismissed_at`。
/// 不新建行——到点后调度器重新发出同一行（去重索引保证不会重复膨胀）。
pub async fn snooze_core(
    pool: &SqlitePool,
    id: i64,
    minutes: i64,
    now: chrono::NaiveDateTime,
) -> Result<(), String> {
    if minutes <= 0 {
        return Err("稍后提醒时间必须大于 0 分钟".to_string());
    }
    let scheduled_at = format_timestamp(now + Duration::minutes(minutes));
    let result = sqlx::query(
        "UPDATE notifications \
         SET scheduled_at = ?1, emitted_at = NULL, dismissed_at = NULL \
         WHERE id = ?2",
    )
    .bind(&scheduled_at)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| format!("稍后提醒设置失败：{e}"))?;
    if result.rows_affected() == 0 {
        return Err("通知不存在或已被删除".to_string());
    }
    Ok(())
}

/// 测试/诊断辅助：读取单条通知（含未发出的行）。
#[cfg(test)]
pub async fn get_notification_core(
    pool: &SqlitePool,
    id: i64,
) -> Result<Option<Notification>, String> {
    let sql = format!("SELECT {NOTIFICATION_COLUMNS} FROM notifications WHERE id = ?1");
    sqlx::query_as::<_, Notification>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("读取通知失败：{e}"))
}

#[tauri::command]
pub async fn notifications_list(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<Notification>, String> {
    let pool = state.pool.read().await;
    list_notifications_core(&pool, limit).await
}

#[tauri::command]
pub async fn notifications_unread(state: State<'_, AppState>) -> Result<u32, String> {
    let pool = state.pool.read().await;
    unread_notifications_core(&pool).await
}

#[tauri::command]
pub async fn notification_mark_read(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let pool = state.pool.read().await;
    mark_read_core(&pool, id).await
}

#[tauri::command]
pub async fn notification_dismiss(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let pool = state.pool.read().await;
    dismiss_core(&pool, id).await
}

#[tauri::command]
pub async fn notification_snooze(
    state: State<'_, AppState>,
    id: i64,
    minutes: i64,
) -> Result<(), String> {
    let pool = state.pool.read().await;
    snooze_core(&pool, id, minutes, now_datetime()).await
}
