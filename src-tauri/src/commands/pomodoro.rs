//! 番茄钟（v0.6）：记录每次专注/休息，并给出今日 / 本周 / 累计与连续天数。
//!
//! 计时在前端跑，结束后把结果写进来；后端只负责存与统计，不常驻计时线程。

use std::collections::HashSet;

use tauri::State;

use crate::db::models::{PomodoroSession, PomodoroSummary};
use crate::db::{now_timestamp, today_string};
use crate::AppState;

const KIND_FOCUS: &str = "focus";

/// 保存一次番茄（completed=false 表示中途放弃，仍记录，用于统计真实投入）。
#[tauri::command]
pub async fn pomodoro_save(
    state: State<'_, AppState>,
    kind: Option<String>,
    duration_seconds: i64,
    started_at: String,
    ended_at: String,
    completed: Option<bool>,
    entry_id: Option<i64>,
) -> Result<PomodoroSession, String> {
    if duration_seconds <= 0 {
        return Err("时长必须大于 0".to_string());
    }
    if duration_seconds > 12 * 3600 {
        return Err("单次时长超过 12 小时，请检查计时".to_string());
    }
    let kind = kind.unwrap_or_else(|| KIND_FOCUS.to_string());
    if kind != "focus" && kind != "break" {
        return Err(format!("未知类型：{kind}（focus / break）"));
    }
    let completed = completed.unwrap_or(true);
    // day 取开始时间的日期部分（本地日期字符串）
    let day = started_at
        .get(0..10)
        .filter(|part| part.len() == 10 && part.as_bytes()[4] == b'-')
        .map(|part| part.to_string())
        .unwrap_or_else(today_string);
    let created_at = now_timestamp();

    let pool = state.pool.read().await;
    let result = sqlx::query(
        "INSERT INTO pomodoro_sessions \
         (entry_id, kind, duration_seconds, day, started_at, ended_at, completed, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )
    .bind(entry_id)
    .bind(&kind)
    .bind(duration_seconds)
    .bind(&day)
    .bind(&started_at)
    .bind(&ended_at)
    .bind(completed)
    .bind(&created_at)
    .execute(&*pool)
    .await
    .map_err(|e| format!("保存番茄记录失败：{e}"))?;

    Ok(PomodoroSession {
        id: result.last_insert_rowid(),
        entry_id,
        kind,
        duration_seconds,
        day,
        started_at,
        ended_at,
        completed,
        created_at,
        entry_content: None,
    })
}

/// 今日 / 最近 7 天 / 累计 的专注次数与分钟数，外加连续专注天数。
#[tauri::command]
pub async fn pomodoro_summary(state: State<'_, AppState>) -> Result<PomodoroSummary, String> {
    let pool = state.pool.read().await;
    let today = today_string();

    let (today_count, today_seconds): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(duration_seconds), 0) FROM pomodoro_sessions \
         WHERE kind = 'focus' AND day = ?1",
    )
    .bind(&today)
    .fetch_one(&*pool)
    .await
    .map_err(|e| format!("统计今日番茄失败：{e}"))?;

    let from = (chrono::Local::now().date_naive() - chrono::Duration::days(6))
        .format("%Y-%m-%d")
        .to_string();
    let (week_count, week_seconds): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(duration_seconds), 0) FROM pomodoro_sessions \
         WHERE kind = 'focus' AND day >= ?1",
    )
    .bind(&from)
    .fetch_one(&*pool)
    .await
    .map_err(|e| format!("统计本周番茄失败：{e}"))?;

    let (total_count, total_seconds): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(duration_seconds), 0) FROM pomodoro_sessions \
         WHERE kind = 'focus'",
    )
    .fetch_one(&*pool)
    .await
    .map_err(|e| format!("统计累计番茄失败：{e}"))?;

    // 连续天数：有专注记录的日期集合，从今天（或昨天）往前数
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT day FROM pomodoro_sessions WHERE kind = 'focus' ORDER BY day DESC LIMIT 400",
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| format!("读取番茄日期失败：{e}"))?;
    let days: HashSet<String> = rows.into_iter().map(|(day,)| day).collect();

    let today_date = chrono::Local::now().date_naive();
    let mut cursor = if days.contains(&today_date.format("%Y-%m-%d").to_string()) {
        today_date
    } else {
        today_date - chrono::Duration::days(1)
    };
    let mut streak = 0i64;
    while days.contains(&cursor.format("%Y-%m-%d").to_string()) {
        streak += 1;
        cursor -= chrono::Duration::days(1);
    }

    Ok(PomodoroSummary {
        today_count,
        today_minutes: today_seconds / 60,
        week_count,
        week_minutes: week_seconds / 60,
        total_count,
        total_minutes: total_seconds / 60,
        streak_days: streak,
    })
}

/// 最近的番茄记录（默认 50 条）。
#[tauri::command]
pub async fn pomodoro_list(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<PomodoroSession>, String> {
    let pool = state.pool.read().await;
    let limit = limit.unwrap_or(50).clamp(1, 500);
    let rows: Vec<PomodoroSession> = sqlx::query_as(
        "SELECT s.id, s.entry_id, s.kind, s.duration_seconds, s.day, s.started_at, s.ended_at, \
                s.completed, s.created_at, e.content AS entry_content \
         FROM pomodoro_sessions s LEFT JOIN entries e ON e.id = s.entry_id \
         ORDER BY s.id DESC LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(&*pool)
    .await
    .map_err(|e| format!("读取番茄记录失败：{e}"))?;
    Ok(rows)
}

/// 清空番茄记录（设置页的「重置统计」）。
#[tauri::command]
pub async fn pomodoro_clear(state: State<'_, AppState>) -> Result<i64, String> {
    let pool = state.pool.read().await;
    let result = sqlx::query("DELETE FROM pomodoro_sessions")
        .execute(&*pool)
        .await
        .map_err(|e| format!("清空番茄记录失败：{e}"))?;
    Ok(result.rows_affected() as i64)
}
