//! 提醒调度器（软件内通知，不接 Windows 系统通知）。
//!
//! 契约见 `docs/接口契约-v0.3.md` 第 4 节：启动时执行一次 `reminder_tick`，之后每 60 秒一次。
//! 每轮顺序：
//! 1. 重新读取 Settings（用户可能刚改过），`mutedUntil > now` 时跳过生成；
//! 2. 生成 `due_soon` / `daily_digest`（`INSERT OR IGNORE`，靠 `dedupe_key` 去重）；
//! 3. 把 `scheduled_at <= now AND emitted_at IS NULL` 的行置 `emitted_at` 并返回给调用方广播；
//! 4. 删除 `created_at` 早于 30 天的通知。
//!
//! tick 主体 `reminder_tick` 接收 `&SqlitePool` 与固定的 `now`，便于单元测试不依赖真实时间；
//! `AppHandle` 只出现在 `reminder_tick_app`（广播事件）与 `start_reminder_scheduler`（循环）里。

use std::path::Path;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, NaiveDateTime};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::notifications::NOTIFICATION_COLUMNS;
use crate::commands::settings::load_settings;
use crate::db::models::{Notification, Settings};
use crate::db::{
    display_month_day, due_datetime, format_date, format_timestamp, now_datetime, parse_date,
    parse_time, parse_timestamp,
};
use crate::AppState;

/// 调度器发出新通知时广播给前端的事件名（契约第 6 节）。
pub const REMINDER_EVENT: &str = "notification-created";
/// 调度周期：60 秒。
pub const TICK_INTERVAL_SECS: u64 = 60;
/// 通知保留天数（契约第 4 节第 4 步）。
pub const CLEANUP_DAYS: i64 = 30;
/// 生成窗口额外向前放宽 60 秒：tick 是 60 秒粒度，正好落在边界上的待办不能被漏掉。
pub const DUE_SOON_GRACE_SECS: i64 = 60;
/// 通知类型。
pub const KIND_DUE_SOON: &str = "due_soon";
pub const KIND_DAILY_DIGEST: &str = "daily_digest";
/// 每日汇总正文里每条最多列出的待办数量。
const DIGEST_MAX_LISTED: usize = 3;

/// 执行一轮提醒检查，返回本轮新发出的通知（按 id 升序）。
pub async fn reminder_tick(
    pool: &SqlitePool,
    data_dir: &Path,
    now: NaiveDateTime,
) -> Result<Vec<Notification>, String> {
    // 1. 每轮重新读设置：用户可能刚改了提醒开关 / 静音时间。
    let settings = load_settings(pool, data_dir).await?;
    let muted = settings
        .muted_until
        .as_deref()
        .and_then(parse_timestamp)
        .is_some_and(|until| until > now);

    // 契约第 4 节：静音期间「本轮只做第 4 步（清理）」，既不生成也不发出。
    if muted {
        cleanup_old_notifications(pool, now).await?;
        return Ok(Vec::new());
    }

    if settings.remind_before_minutes > 0 {
        create_due_soon_notifications(pool, &settings, now).await?;
    }
    if settings.daily_digest_enabled {
        create_daily_digest_notification(pool, &settings, now).await?;
    }

    // 3. 发出到点的通知。
    let emitted = emit_pending_notifications(pool, now).await?;

    // 4. 清理。
    cleanup_old_notifications(pool, now).await?;
    Ok(emitted)
}

/// 生成「截止前提醒」：只针对未完成、未删除、同时有 `due_date` 和 `due_time` 的待办。
async fn create_due_soon_notifications(
    pool: &SqlitePool,
    settings: &Settings,
    now: NaiveDateTime,
) -> Result<(), String> {
    let window_end = now
        + ChronoDuration::seconds(settings.remind_before_minutes * 60 + DUE_SOON_GRACE_SECS);
    let rows: Vec<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, content, due_date, due_time FROM entries \
         WHERE is_todo = 1 AND is_deleted = 0 AND completed_at IS NULL AND parent_id IS NULL \
           AND due_date IS NOT NULL AND due_time IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("读取到期待办失败：{e}"))?;

    let scheduled_at = format_timestamp(now);
    for (entry_id, content, due_date, due_time) in rows {
        // due_time 为空的待办只进每日汇总，不触发「截止前」提醒。
        let (Some(due_date), Some(due_time)) = (due_date, due_time) else {
            continue;
        };
        let Some(due_at) = due_datetime(&due_date, &due_time) else {
            continue;
        };
        // 窗口 [now, now + remindBefore + 60s]；已逾期（due_at < now）不生成。
        if due_at < now || due_at > window_end {
            continue;
        }
        let remaining_secs = (due_at - now).num_seconds().max(0);
        let minutes = ((remaining_secs + 59) / 60).max(1);
        let title = format!("「{content}」还有 {minutes} 分钟到期");
        let body = due_soon_body(&due_date, &due_time, now);
        // 同一条待办改期后 dedupe_key 变化，会生成新通知（旧行保留，属正常历史）。
        let dedupe_key = format!("due_soon:{entry_id}:{due_date}T{due_time}");
        insert_notification(
            pool,
            KIND_DUE_SOON,
            Some(entry_id),
            &title,
            &body,
            &dedupe_key,
            &scheduled_at,
        )
        .await?;
    }
    Ok(())
}

/// 「截止前」通知正文：`今天 15:00 到期 · 11月4日`（非今天时用对应的日期描述）。
fn due_soon_body(due_date: &str, due_time: &str, now: NaiveDateTime) -> String {
    let Some(date) = parse_date(due_date) else {
        return format!("{due_date} {due_time} 到期");
    };
    let today = now.date();
    let day_gap = (date - today).num_days();
    let date_part = display_month_day(date);
    if day_gap == 0 {
        format!("今天 {due_time} 到期 · {date_part}")
    } else if day_gap == 1 {
        format!("明天 {due_time} 到期 · {date_part}")
    } else {
        format!("{date_part} {due_time} 到期")
    }
}

/// 生成每日汇总：当天 `dailyDigestTime` 到点后生成一次，`dedupe_key = digest:<YYYY-MM-DD>`。
async fn create_daily_digest_notification(
    pool: &SqlitePool,
    settings: &Settings,
    now: NaiveDateTime,
) -> Result<(), String> {
    let today = now.date();
    let Some(digest_time) = parse_time(&settings.daily_digest_time) else {
        // 设置读取时已过滤非法值，这里只是防御。
        return Ok(());
    };
    if now < today.and_time(digest_time) {
        return Ok(());
    }
    let today_str = format_date(today);
    let dedupe_key = format!("digest:{today_str}");

    // 今天已经生成过（含用户已忽略/已读）就不再生成。
    let existing: Option<i64> =
        sqlx::query_scalar("SELECT id FROM notifications WHERE dedupe_key = ?1")
            .bind(&dedupe_key)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("检查每日汇总是否已生成失败：{e}"))?;
    if existing.is_some() {
        return Ok(());
    }

    // 昨日及更早逾期未完成（不含今天）。v0.4：子任务永不提醒（parent_id IS NULL）。
    let overdue: Vec<String> = sqlx::query_scalar(
        "SELECT content FROM entries \
         WHERE is_todo = 1 AND is_deleted = 0 AND completed_at IS NULL AND parent_id IS NULL \
           AND due_date IS NOT NULL AND due_date < ?1 \
         ORDER BY due_date ASC, created_at ASC, id ASC",
    )
    .bind(&today_str)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("读取逾期待办失败：{e}"))?;
    // 今日到期未完成；有具体时间的按时间排前面。v0.4：同样只算顶层任务。
    let due_today: Vec<String> = sqlx::query_scalar(
        "SELECT content FROM entries \
         WHERE is_todo = 1 AND is_deleted = 0 AND completed_at IS NULL AND parent_id IS NULL \
           AND due_date = ?1 \
         ORDER BY due_time IS NULL ASC, due_time ASC, created_at ASC, id ASC",
    )
    .bind(&today_str)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("读取今日到期待办失败：{e}"))?;

    let total = overdue.len() + due_today.len();
    if total == 0 {
        return Ok(());
    }
    let title = format!("今天有 {total} 件事要收尾");
    let body = digest_body(&overdue, &due_today);
    insert_notification(
        pool,
        KIND_DAILY_DIGEST,
        None,
        &title,
        &body,
        &dedupe_key,
        &format_timestamp(now),
    )
    .await
}

/// 每日汇总正文：`昨日逾期 X 项：A、B　·　今日到期 Y 项：C`（各自最多列 3 条）。
fn digest_body(overdue: &[String], due_today: &[String]) -> String {
    let mut parts = Vec::new();
    if !overdue.is_empty() {
        parts.push(format!(
            "昨日逾期 {} 项：{}",
            overdue.len(),
            names_preview(overdue)
        ));
    }
    if !due_today.is_empty() {
        parts.push(format!(
            "今日到期 {} 项：{}",
            due_today.len(),
            names_preview(due_today)
        ));
    }
    parts.join("\u{3000}·\u{3000}")
}

fn names_preview(items: &[String]) -> String {
    let mut out = items
        .iter()
        .take(DIGEST_MAX_LISTED)
        .cloned()
        .collect::<Vec<_>>()
        .join("、");
    if items.len() > DIGEST_MAX_LISTED {
        out.push_str(&format!(" 等 {} 项", items.len()));
    }
    out
}

/// 插入通知（唯一索引 + INSERT OR IGNORE 实现去重）。
#[allow(clippy::too_many_arguments)]
async fn insert_notification(
    pool: &SqlitePool,
    kind: &str,
    entry_id: Option<i64>,
    title: &str,
    body: &str,
    dedupe_key: &str,
    scheduled_at: &str,
) -> Result<(), String> {
    sqlx::query(
        "INSERT OR IGNORE INTO notifications \
         (kind, entry_id, title, body, dedupe_key, scheduled_at, emitted_at, read_at, \
          dismissed_at, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, NULL, ?6)",
    )
    .bind(kind)
    .bind(entry_id)
    .bind(title)
    .bind(body)
    .bind(dedupe_key)
    .bind(scheduled_at)
    .execute(pool)
    .await
    .map_err(|e| format!("生成通知失败：{e}"))?;
    Ok(())
}

/// 发出到点的通知：先置 `emitted_at = now`，再把该批行查出来返回给调用方广播。
async fn emit_pending_notifications(
    pool: &SqlitePool,
    now: NaiveDateTime,
) -> Result<Vec<Notification>, String> {
    let now_ts = format_timestamp(now);
    let sql = format!(
        "UPDATE notifications SET emitted_at = ?1 \
         WHERE scheduled_at <= ?1 AND emitted_at IS NULL \
         RETURNING {NOTIFICATION_COLUMNS}"
    );
    sqlx::query_as::<_, Notification>(&sql)
        .bind(&now_ts)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("发出通知失败：{e}"))
}

/// 删除 `created_at` 早于 30 天的通知。
async fn cleanup_old_notifications(
    pool: &SqlitePool,
    now: NaiveDateTime,
) -> Result<(), String> {
    let cutoff = format_timestamp(now - ChronoDuration::days(CLEANUP_DAYS));
    sqlx::query("DELETE FROM notifications WHERE created_at < ?1")
        .bind(&cutoff)
        .execute(pool)
        .await
        .map_err(|e| format!("清理旧通知失败：{e}"))?;
    Ok(())
}

/// 读库、跑一轮 tick，并把新发出的通知广播给前端。
pub async fn reminder_tick_app(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let data_dir = state.paths.data_dir.clone();
    let notifications = {
        let pool = state.pool.read().await;
        reminder_tick(&pool, &data_dir, now_datetime()).await?
    };
    for notification in notifications {
        if let Err(err) = app.emit(REMINDER_EVENT, notification) {
            eprintln!("[WorkNote] 广播 {REMINDER_EVENT} 事件失败：{err}");
        }
    }
    Ok(())
}

/// 启动调度器：立刻执行一次，之后每 60 秒执行一次（失败只打日志，不阻断应用）。
pub fn start_reminder_scheduler(app: AppHandle) {
    let _ = tauri::async_runtime::spawn(async move {
        if let Err(err) = reminder_tick_app(&app).await {
            eprintln!("[WorkNote] 启动时提醒检查失败：{err}");
        }
        loop {
            tokio::time::sleep(Duration::from_secs(TICK_INTERVAL_SECS)).await;
            if let Err(err) = reminder_tick_app(&app).await {
                eprintln!("[WorkNote] 提醒检查失败：{err}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::entries::{
        create_entry_core, create_entry_with_due_time_core, set_entry_completed_core,
    };
    use crate::commands::notifications::{
        dismiss_core, get_notification_core, list_notifications_core, mark_read_core,
        snooze_core, unread_notifications_core,
    };
    use crate::db::{cleanup_test_pool, set_setting, test_pool};

    fn at(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    async fn count_notifications(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM notifications")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    /// 只测「截止前提醒」的用例里关掉每日汇总，避免 09:30 之后额外多出一条 digest。
    async fn disable_daily_digest(pool: &SqlitePool) {
        set_setting(pool, "daily_digest_enabled", "false")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn due_soon_is_generated_emitted_and_never_duplicated() {
        let (pool, dir) = test_pool().await;
        disable_daily_digest(&pool).await;
        let now = at(2026, 11, 4, 10, 0);
        let entry = create_entry_with_due_time_core(
            &pool,
            "本周周报",
            true,
            Some("2026-11-04".into()),
            Some("10:20".into()),
        )
        .await
        .unwrap();

        let emitted = reminder_tick(&pool, &dir, now).await.unwrap();
        assert_eq!(emitted.len(), 1);
        let notification = &emitted[0];
        assert_eq!(notification.kind, KIND_DUE_SOON);
        assert_eq!(notification.entry_id, Some(entry.id));
        assert_eq!(notification.title, "「本周周报」还有 20 分钟到期");
        assert_eq!(notification.body, "今天 10:20 到期 · 11月4日");
        assert_eq!(notification.scheduled_at, "2026-11-04T10:00:00");
        assert_eq!(notification.emitted_at.as_deref(), Some("2026-11-04T10:00:00"));
        assert_eq!(notification.read_at, None);
        assert_eq!(notification.dismissed_at, None);

        // 同一分钟 / 同一窗口再跑一次：去重不新增，也不再重复发出。
        let again = reminder_tick(&pool, &dir, now).await.unwrap();
        assert!(again.is_empty());
        assert_eq!(count_notifications(&pool).await, 1);

        // 改期后 dedupe_key 变化，会生成新的一行（旧行保留为历史）。
        let mut patch = crate::db::models::EntryPatch::default();
        patch.due_time = Some(Some("11:00".into()));
        crate::commands::entries::update_entry_core(&pool, entry.id, patch)
            .await
            .unwrap();
        let later = at(2026, 11, 4, 10, 40);
        let emitted = reminder_tick(&pool, &dir, later).await.unwrap();
        assert_eq!(emitted.len(), 1);
        assert_eq!(count_notifications(&pool).await, 2);

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn due_soon_ignores_date_only_far_future_completed_and_deleted() {
        let (pool, dir) = test_pool().await;
        disable_daily_digest(&pool).await;
        let now = at(2026, 11, 4, 10, 0);

        // 只有日期没有时间：只进每日汇总，不产生「截止前」提醒。
        create_entry_core(&pool, "今天到期没时间", true, Some("2026-11-04".into()))
            .await
            .unwrap();
        // 超出提醒窗口。
        create_entry_with_due_time_core(
            &pool,
            "下午的活",
            true,
            Some("2026-11-04".into()),
            Some("15:00".into()),
        )
        .await
        .unwrap();
        // 已完成的待办不提醒。
        let done = create_entry_with_due_time_core(
            &pool,
            "已完成",
            true,
            Some("2026-11-04".into()),
            Some("10:10".into()),
        )
        .await
        .unwrap();
        set_entry_completed_core(&pool, done.id, true).await.unwrap();
        // 已删除的待办不提醒。
        let deleted = create_entry_with_due_time_core(
            &pool,
            "已删除",
            true,
            Some("2026-11-04".into()),
            Some("10:10".into()),
        )
        .await
        .unwrap();
        crate::commands::entries::delete_entry_core(&pool, deleted.id)
            .await
            .unwrap();

        let emitted = reminder_tick(&pool, &dir, now).await.unwrap();
        assert!(
            emitted.iter().all(|n| n.kind != KIND_DUE_SOON),
            "不该产生截止前提醒：{emitted:?}"
        );
        assert_eq!(count_notifications(&pool).await, 0);

        // remindBeforeMinutes = 0（关闭）：即使进入窗口也不生成。
        set_setting(&pool, "remind_before_minutes", "0").await.unwrap();
        create_entry_with_due_time_core(
            &pool,
            "关闭提醒后的待办",
            true,
            Some("2026-11-04".into()),
            Some("10:20".into()),
        )
        .await
        .unwrap();
        let emitted = reminder_tick(&pool, &dir, now).await.unwrap();
        assert!(emitted.is_empty());
        assert_eq!(count_notifications(&pool).await, 0);

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn due_soon_window_includes_tomorrow_and_caps_grace() {
        let (pool, dir) = test_pool().await;
        disable_daily_digest(&pool).await;
        let now = at(2026, 11, 4, 10, 0);
        set_setting(&pool, "remind_before_minutes", "1440")
            .await
            .unwrap();
        create_entry_with_due_time_core(
            &pool,
            "明天上午",
            true,
            Some("2026-11-05".into()),
            Some("09:00".into()),
        )
        .await
        .unwrap();

        let emitted = reminder_tick(&pool, &dir, now).await.unwrap();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].title, "「明天上午」还有 1380 分钟到期");
        assert_eq!(emitted[0].body, "明天 09:00 到期 · 11月5日");

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn snooze_moves_schedule_clears_emitted_and_reemits_same_row() {
        let (pool, dir) = test_pool().await;
        disable_daily_digest(&pool).await;
        let now = at(2026, 11, 4, 10, 0);
        create_entry_with_due_time_core(
            &pool,
            "要稍后提醒",
            true,
            Some("2026-11-04".into()),
            Some("10:20".into()),
        )
        .await
        .unwrap();

        let emitted = reminder_tick(&pool, &dir, now).await.unwrap();
        assert_eq!(emitted.len(), 1);
        let first = emitted[0].clone();

        snooze_core(&pool, first.id, 10, now).await.unwrap();
        let row = get_notification_core(&pool, first.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.scheduled_at, "2026-11-04T10:10:00");
        assert_eq!(row.emitted_at, None);
        assert_eq!(row.dismissed_at, None);

        // 未到点：不发出。
        let before = reminder_tick(&pool, &dir, at(2026, 11, 4, 10, 5))
            .await
            .unwrap();
        assert!(before.is_empty());

        // 到点：同一行重新发出（不新建行）。
        let after = reminder_tick(&pool, &dir, at(2026, 11, 4, 10, 11))
            .await
            .unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].id, first.id);
        assert!(after[0].emitted_at.is_some());
        assert_eq!(count_notifications(&pool).await, 1);

        // 非法分钟数被拒绝。
        assert!(snooze_core(&pool, first.id, 0, now).await.is_err());
        assert!(snooze_core(&pool, first.id, -5, now).await.is_err());
        assert!(snooze_core(&pool, 999_999, 5, now).await.is_err());

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn muted_until_skips_generation_but_still_cleans_up() {
        let (pool, dir) = test_pool().await;
        disable_daily_digest(&pool).await;
        let now = at(2026, 11, 4, 10, 0);
        set_setting(&pool, "muted_until", "2026-11-04T23:59:59")
            .await
            .unwrap();
        create_entry_with_due_time_core(
            &pool,
            "静音期间到期",
            true,
            Some("2026-11-04".into()),
            Some("10:20".into()),
        )
        .await
        .unwrap();
        // 一条 40 天前的旧通知：静音时清理仍然要执行。
        sqlx::query(
            "INSERT INTO notifications \
             (kind, entry_id, title, body, dedupe_key, scheduled_at, emitted_at, read_at, \
              dismissed_at, created_at) \
             VALUES ('daily_digest', NULL, '旧', '旧', 'digest:old', '2026-09-01T09:30:00', \
                     '2026-09-01T09:30:00', NULL, NULL, '2026-09-01T09:30:00')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let emitted = reminder_tick(&pool, &dir, now).await.unwrap();
        assert!(emitted.is_empty(), "静音期间不应发出任何提醒");
        assert_eq!(count_notifications(&pool).await, 0, "旧通知应被清理");

        // 静音截止之后恢复正常：静音期间到期的那条与新待办都会产生提醒。
        set_setting(&pool, "muted_until", "2026-11-04T09:00:00")
            .await
            .unwrap();
        create_entry_with_due_time_core(
            &pool,
            "静音结束后的待办",
            true,
            Some("2026-11-04".into()),
            Some("10:30".into()),
        )
        .await
        .unwrap();
        let emitted = reminder_tick(&pool, &dir, now).await.unwrap();
        assert_eq!(emitted.len(), 2);
        assert!(emitted.iter().all(|n| n.kind == KIND_DUE_SOON));
        assert!(
            emitted
                .iter()
                .any(|n| n.title.contains("静音结束后的待办")),
            "解除静音后应补发提醒：{emitted:?}"
        );

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn daily_digest_generated_once_after_configured_time() {
        let (pool, dir) = test_pool().await;
        // 昨日及更早逾期 2 项 + 今日到期 2 项（其中一条已完成、一条已删除不计数）。
        create_entry_core(&pool, "逾期A", true, Some("2026-11-03".into()))
            .await
            .unwrap();
        create_entry_core(&pool, "逾期B", true, Some("2026-11-01".into()))
            .await
            .unwrap();
        create_entry_core(&pool, "今日C", true, Some("2026-11-04".into()))
            .await
            .unwrap();
        create_entry_core(&pool, "今日D", true, Some("2026-11-04".into()))
            .await
            .unwrap();
        let done = create_entry_core(&pool, "今日已完成", true, Some("2026-11-04".into()))
            .await
            .unwrap();
        set_entry_completed_core(&pool, done.id, true).await.unwrap();
        let deleted = create_entry_core(&pool, "今日已删除", true, Some("2026-11-04".into()))
            .await
            .unwrap();
        crate::commands::entries::delete_entry_core(&pool, deleted.id)
            .await
            .unwrap();

        // 08:00 还没到 09:30，不生成每日汇总。
        let early = reminder_tick(&pool, &dir, at(2026, 11, 4, 8, 0))
            .await
            .unwrap();
        assert!(early.is_empty());

        // 09:31 到点：生成一条汇总并立即发出。
        let emitted = reminder_tick(&pool, &dir, at(2026, 11, 4, 9, 31))
            .await
            .unwrap();
        assert_eq!(emitted.len(), 1);
        let digest = &emitted[0];
        assert_eq!(digest.kind, KIND_DAILY_DIGEST);
        assert_eq!(digest.entry_id, None);
        assert_eq!(digest.title, "今天有 4 件事要收尾");
        assert!(digest.body.contains("昨日逾期 2 项："), "body={}", digest.body);
        assert!(digest.body.contains("今日到期 2 项："), "body={}", digest.body);
        assert!(digest.body.contains('\u{3000}'), "两段之间用全角空格分隔");

        // 同一天再跑：不再生成（dedupe）。
        let again = reminder_tick(&pool, &dir, at(2026, 11, 4, 10, 0))
            .await
            .unwrap();
        assert!(again.is_empty());
        assert_eq!(count_notifications(&pool).await, 1);

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn daily_digest_lists_at_most_three_names() {
        let (pool, dir) = test_pool().await;
        for index in 0..5 {
            create_entry_core(&pool, &format!("逾期{index}"), true, Some("2026-11-03".into()))
                .await
                .unwrap();
        }
        let emitted = reminder_tick(&pool, &dir, at(2026, 11, 4, 12, 0))
            .await
            .unwrap();
        assert_eq!(emitted.len(), 1);
        let body = &emitted[0].body;
        assert!(body.starts_with("昨日逾期 5 项："), "body={body}");
        assert!(body.contains("逾期0、逾期1、逾期2"), "body={body}");
        assert!(!body.contains("逾期3"), "最多列 3 条：{body}");
        assert!(body.contains("等 5 项"), "body={body}");

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn notification_commands_follow_contract() {
        let (pool, dir) = test_pool().await;
        disable_daily_digest(&pool).await;
        let now = at(2026, 11, 4, 10, 0);
        create_entry_with_due_time_core(
            &pool,
            "读通知",
            true,
            Some("2026-11-04".into()),
            Some("10:20".into()),
        )
        .await
        .unwrap();
        // 一条尚未发出的通知：列表与未读数都不应包含它。
        sqlx::query(
            "INSERT INTO notifications \
             (kind, entry_id, title, body, dedupe_key, scheduled_at, emitted_at, read_at, \
              dismissed_at, created_at) \
             VALUES ('due_soon', NULL, '未发出', '未发出', 'due_soon:999:2026-11-04T23:59', \
                     '2026-11-04T23:59:00', NULL, NULL, NULL, '2026-11-04T10:00:00')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let emitted = reminder_tick(&pool, &dir, now).await.unwrap();
        assert_eq!(emitted.len(), 1);
        let id = emitted[0].id;

        // 默认 limit = 50，且只返回已发出未忽略的通知。
        let list = list_notifications_core(&pool, None).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
        assert_eq!(unread_notifications_core(&pool).await.unwrap(), 1);

        // 标记已读：未读数归零，read_at 不被后续调用覆盖。
        mark_read_core(&pool, id).await.unwrap();
        let read = get_notification_core(&pool, id).await.unwrap().unwrap();
        assert!(read.read_at.is_some());
        let read_at = read.read_at.clone();
        mark_read_core(&pool, id).await.unwrap();
        let read_again = get_notification_core(&pool, id).await.unwrap().unwrap();
        assert_eq!(read_again.read_at, read_at);
        assert_eq!(unread_notifications_core(&pool).await.unwrap(), 0);

        // 忽略后从列表消失。
        dismiss_core(&pool, id).await.unwrap();
        assert!(list_notifications_core(&pool, None).await.unwrap().is_empty());
        assert_eq!(unread_notifications_core(&pool).await.unwrap(), 0);

        // 不存在的通知返回中文错误。
        let err = mark_read_core(&pool, 424_242).await.unwrap_err();
        assert!(err.contains("通知不存在"));
        assert!(dismiss_core(&pool, 424_242).await.is_err());

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn old_notifications_are_cleaned_after_30_days() {
        let (pool, dir) = test_pool().await;
        let now = at(2026, 11, 4, 10, 0);
        set_setting(&pool, "muted_until", "2026-11-04T23:59:59")
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO notifications \
             (kind, entry_id, title, body, dedupe_key, scheduled_at, emitted_at, read_at, \
              dismissed_at, created_at) \
             VALUES ('due_soon', NULL, '31 天前', 'x', 'due_soon:1:2026-09-01T09:00', \
                     '2026-10-04T10:00:00', '2026-10-04T10:00:00', NULL, NULL, \
                     '2026-10-04T10:00:00'), \
                    ('due_soon', NULL, '29 天前', 'y', 'due_soon:2:2026-10-06T09:00', \
                     '2026-10-06T10:00:00', '2026-10-06T10:00:00', NULL, NULL, \
                     '2026-10-06T10:00:00')",
        )
        .execute(&pool)
        .await
        .unwrap();

        reminder_tick(&pool, &dir, now).await.unwrap();
        let remaining: Vec<String> = sqlx::query_scalar("SELECT title FROM notifications")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(remaining, vec!["29 天前".to_string()]);

        cleanup_test_pool(pool, dir).await;
    }

    // -----------------------------------------------------------------------
    // v0.4：子任务永不提醒
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn due_soon_ignores_children() {
        let (pool, dir) = test_pool().await;
        disable_daily_digest(&pool).await;
        let now = at(2026, 11, 4, 10, 0);
        let parent = create_entry_with_due_time_core(
            &pool,
            "父任务",
            true,
            Some("2026-11-04".into()),
            Some("10:20".into()),
        )
        .await
        .unwrap();
        let child = crate::commands::children::create_child_core(&pool, parent.id, "子任务")
            .await
            .unwrap();
        // 子任务本不该有截止时间；直接改库模拟脏数据，提醒仍不得为它生成通知。
        sqlx::query(
            "UPDATE entries SET due_date = '2026-11-04', due_time = '10:20' WHERE id = ?1",
        )
        .bind(child.id)
        .execute(&pool)
        .await
        .unwrap();

        let emitted = reminder_tick(&pool, &dir, now).await.unwrap();
        assert_eq!(emitted.len(), 1, "只为父任务生成提醒：{emitted:?}");
        assert_eq!(emitted[0].entry_id, Some(parent.id));

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn daily_digest_ignores_children() {
        let (pool, dir) = test_pool().await;
        // 父任务：今日到期 1 项；子任务模拟“昨日逾期”，不得计入汇总。
        let parent = create_entry_core(&pool, "父任务", true, Some("2026-11-04".into()))
            .await
            .unwrap();
        let child = crate::commands::children::create_child_core(&pool, parent.id, "子任务")
            .await
            .unwrap();
        sqlx::query("UPDATE entries SET due_date = '2026-11-03' WHERE id = ?1")
            .bind(child.id)
            .execute(&pool)
            .await
            .unwrap();

        let emitted = reminder_tick(&pool, &dir, at(2026, 11, 4, 12, 0))
            .await
            .unwrap();
        assert_eq!(emitted.len(), 1);
        let digest = &emitted[0];
        assert_eq!(digest.kind, KIND_DAILY_DIGEST);
        assert_eq!(digest.title, "今天有 1 件事要收尾");
        assert!(!digest.body.contains("子任务"), "body={}", digest.body);

        cleanup_test_pool(pool, dir).await;
    }
}
