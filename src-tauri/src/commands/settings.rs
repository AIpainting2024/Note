//! 设置读写命令。
//!
//! `data_dir` 为只读字段：`save_settings` 忽略前端传入值，始终由后端填写。
//! v0.3 新增：`mainShortcut` / `searchShortcut` / `remindBeforeMinutes` / `dailyDigestEnabled` /
//! `dailyDigestTime` / `mutedUntil`，字段名与 `docs/接口契约-v0.3.md` 第 2 节一致。

use std::collections::HashMap;
use std::path::Path;

use sqlx::SqlitePool;
use tauri::State;

use crate::db::models::{
    Settings, DEFAULT_AUTO_BACKUP_KEEP, DEFAULT_DAILY_DIGEST_ENABLED, DEFAULT_DAILY_DIGEST_TIME,
    DEFAULT_GLOBAL_SHORTCUT, DEFAULT_MAIN_SHORTCUT, DEFAULT_REMIND_BEFORE_MINUTES,
    DEFAULT_SEARCH_SHORTCUT, DEFAULT_THEME,
};
use crate::db::{load_settings_map, parse_time, parse_timestamp, set_setting, tomorrow_midnight};
use crate::AppState;

/// 允许的主题值。
pub const THEMES: [&str; 3] = ["light", "dark", "system"];
/// 自动备份保留份数的上限，防止用户误填天文数字。
pub const MAX_BACKUP_KEEP: i64 = 365;
/// `remindBeforeMinutes` 的合法取值；0 = 关闭截止前提醒。
pub const ALLOWED_REMIND_MINUTES: [i64; 5] = [0, 15, 30, 60, 1440];

fn map_bool(map: &HashMap<String, String>, key: &str, default: bool) -> bool {
    match map.get(key).map(|v| v.trim()) {
        Some("true") | Some("1") => true,
        Some("false") | Some("0") => false,
        _ => default,
    }
}

/// 从 `app_settings` 读取设置并补齐默认值。
pub async fn load_settings(pool: &SqlitePool, data_dir: &Path) -> Result<Settings, String> {
    let map = load_settings_map(pool).await?;
    let theme = map
        .get("theme")
        .map(|v| v.trim().to_string())
        .filter(|v| THEMES.contains(&v.as_str()))
        .unwrap_or_else(|| DEFAULT_THEME.to_string());
    let auto_backup = map_bool(&map, "auto_backup", crate::db::models::DEFAULT_AUTO_BACKUP);
    let auto_backup_keep = map
        .get("auto_backup_keep")
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(DEFAULT_AUTO_BACKUP_KEEP)
        .min(MAX_BACKUP_KEEP);
    let shortcut = |key: &str, default: &str| {
        map.get(key)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| default.to_string())
    };
    let global_shortcut = shortcut("global_shortcut", DEFAULT_GLOBAL_SHORTCUT);
    let main_shortcut = shortcut("main_shortcut", DEFAULT_MAIN_SHORTCUT);
    let search_shortcut = shortcut("search_shortcut", DEFAULT_SEARCH_SHORTCUT);
    let remind_before_minutes = map
        .get("remind_before_minutes")
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|v| ALLOWED_REMIND_MINUTES.contains(v))
        .unwrap_or(DEFAULT_REMIND_BEFORE_MINUTES);
    let daily_digest_enabled = map_bool(&map, "daily_digest_enabled", DEFAULT_DAILY_DIGEST_ENABLED);
    let daily_digest_time = map
        .get("daily_digest_time")
        .map(|v| v.trim().to_string())
        .filter(|v| parse_time(v).is_some())
        .unwrap_or_else(|| DEFAULT_DAILY_DIGEST_TIME.to_string());
    // 静音截止时刻：空值 / 非法时间戳都视作未静音。
    let muted_until = map
        .get("muted_until")
        .map(|v| v.trim().to_string())
        .filter(|v| parse_timestamp(v).is_some());
    let last_backup_at = map.get("last_backup_at").cloned().filter(|v| !v.trim().is_empty());

    Ok(Settings {
        theme,
        auto_backup,
        auto_backup_keep,
        global_shortcut,
        main_shortcut,
        search_shortcut,
        remind_before_minutes,
        daily_digest_enabled,
        daily_digest_time,
        muted_until,
        data_dir: data_dir.display().to_string(),
        last_backup_at,
    })
}

/// 保存设置并返回落库后的最新值（`data_dir` 用后端值覆盖前端传入值）。
pub async fn save_settings_core(
    pool: &SqlitePool,
    settings: Settings,
    data_dir: &Path,
) -> Result<Settings, String> {
    let theme = settings.theme.trim().to_string();
    if !THEMES.contains(&theme.as_str()) {
        return Err(format!("主题值无效：{theme}（可选 light / dark / system）"));
    }
    let auto_backup_keep = settings.auto_backup_keep.clamp(1, MAX_BACKUP_KEEP);
    let global_shortcut = settings.global_shortcut.trim().to_string();
    if global_shortcut.is_empty() {
        return Err("全局快捷键不能为空".to_string());
    }
    let main_shortcut = settings.main_shortcut.trim().to_string();
    if main_shortcut.is_empty() {
        return Err("主界面快捷键不能为空".to_string());
    }
    let search_shortcut = settings.search_shortcut.trim().to_string();
    if search_shortcut.is_empty() {
        return Err("搜索快捷键不能为空".to_string());
    }
    if !ALLOWED_REMIND_MINUTES.contains(&settings.remind_before_minutes) {
        return Err(format!(
            "截止前提醒时间无效：{}（可选 0 / 15 / 30 / 60 / 1440 分钟）",
            settings.remind_before_minutes
        ));
    }
    let daily_digest_time = settings.daily_digest_time.trim().to_string();
    if parse_time(&daily_digest_time).is_none() {
        return Err(format!(
            "每日汇总时间格式无效（应为 HH:MM）：{daily_digest_time}"
        ));
    }
    let muted_until = settings
        .muted_until
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    if let Some(value) = &muted_until
        && parse_timestamp(value).is_none()
    {
        return Err(format!("静音截止时间格式无效：{value}"));
    }

    let pairs: [(&str, String); 9] = [
        ("theme", theme),
        ("auto_backup", if settings.auto_backup { "true" } else { "false" }.to_string()),
        ("auto_backup_keep", auto_backup_keep.to_string()),
        ("global_shortcut", global_shortcut),
        ("main_shortcut", main_shortcut),
        ("search_shortcut", search_shortcut),
        ("remind_before_minutes", settings.remind_before_minutes.to_string()),
        (
            "daily_digest_enabled",
            if settings.daily_digest_enabled { "true" } else { "false" }.to_string(),
        ),
        ("daily_digest_time", daily_digest_time),
    ];

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("数据库事务开启失败：{e}"))?;
    for (key, value) in pairs.iter() {
        sqlx::query(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(*key)
        .bind(value)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("保存设置失败：{e}"))?;
    }
    // mutedUntil 可空：显式 null 时删除该键，而不是存空串（保持“无静音”语义干净）。
    match &muted_until {
        Some(value) => {
            sqlx::query(
                "INSERT INTO app_settings (key, value) VALUES ('muted_until', ?1) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .bind(value)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("保存设置失败：{e}"))?;
        }
        None => {
            sqlx::query("DELETE FROM app_settings WHERE key = 'muted_until'")
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("保存设置失败：{e}"))?;
        }
    }
    tx.commit()
        .await
        .map_err(|e| format!("保存设置失败：{e}"))?;

    load_settings(pool, data_dir).await
}

/// 静音到今天结束（明天 00:00），返回新 Settings。
pub async fn mute_reminders_today_core(
    pool: &SqlitePool,
    data_dir: &Path,
) -> Result<Settings, String> {
    let muted_until = tomorrow_midnight();
    set_setting(pool, "muted_until", &muted_until).await?;
    load_settings(pool, data_dir).await
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let pool = state.pool.read().await;
    load_settings(&pool, &state.paths.data_dir).await
}

#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    let pool = state.pool.read().await;
    save_settings_core(&pool, settings, &state.paths.data_dir).await
}

/// 一键静音今天的全部提醒：`mutedUntil` 设为明天 00:00。
#[tauri::command]
pub async fn mute_reminders_today(state: State<'_, AppState>) -> Result<Settings, String> {
    let pool = state.pool.read().await;
    mute_reminders_today_core(&pool, &state.paths.data_dir).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{cleanup_test_pool, test_pool};

    fn settings_fixture() -> Settings {
        Settings {
            theme: "dark".into(),
            auto_backup: false,
            auto_backup_keep: 30,
            global_shortcut: "CommandOrControl+Alt+W".into(),
            main_shortcut: "CommandOrControl+Alt+M".into(),
            search_shortcut: "CommandOrControl+Alt+K".into(),
            remind_before_minutes: 15,
            daily_digest_enabled: false,
            daily_digest_time: "08:15".into(),
            muted_until: None,
            data_dir: "前端乱填的路径".into(),
            last_backup_at: Some("2026-01-01T00:00:00".into()),
        }
    }

    #[tokio::test]
    async fn defaults_are_seeded() {
        let (pool, dir) = test_pool().await;
        let settings = load_settings(&pool, &dir).await.unwrap();
        assert_eq!(settings.theme, "system");
        assert!(settings.auto_backup);
        assert_eq!(settings.auto_backup_keep, 14);
        assert_eq!(settings.global_shortcut, "CommandOrControl+Shift+N");
        // v0.3 新增字段的默认值（契约第 2 节）
        assert_eq!(settings.main_shortcut, "CommandOrControl+Shift+M");
        assert_eq!(settings.search_shortcut, "CommandOrControl+K");
        assert_eq!(settings.remind_before_minutes, 30);
        assert!(settings.daily_digest_enabled);
        assert_eq!(settings.daily_digest_time, "09:30");
        assert_eq!(settings.muted_until, None);
        assert_eq!(settings.data_dir, dir.display().to_string());
        assert_eq!(settings.last_backup_at, None);
        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn save_roundtrip_ignores_data_dir_and_keeps_last_backup() {
        let (pool, dir) = test_pool().await;
        // 模拟已有 last_backup_at
        crate::db::set_setting(&pool, "last_backup_at", "2026-11-01T10:00:00")
            .await
            .unwrap();

        let mut fixture = settings_fixture();
        fixture.last_backup_at = None; // 前端没有回传也不应该清掉
        let saved = save_settings_core(&pool, fixture, &dir).await.unwrap();
        assert_eq!(saved.theme, "dark");
        assert!(!saved.auto_backup);
        assert_eq!(saved.auto_backup_keep, 30);
        assert_eq!(saved.global_shortcut, "CommandOrControl+Alt+W");
        assert_eq!(saved.main_shortcut, "CommandOrControl+Alt+M");
        assert_eq!(saved.search_shortcut, "CommandOrControl+Alt+K");
        assert_eq!(saved.remind_before_minutes, 15);
        assert!(!saved.daily_digest_enabled);
        assert_eq!(saved.daily_digest_time, "08:15");
        assert_eq!(saved.data_dir, dir.display().to_string());
        assert_eq!(saved.last_backup_at.as_deref(), Some("2026-11-01T10:00:00"));

        let reloaded = load_settings(&pool, &dir).await.unwrap();
        assert_eq!(reloaded.theme, "dark");
        assert_eq!(reloaded.auto_backup_keep, 30);
        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn invalid_values_are_rejected_or_clamped() {
        let (pool, dir) = test_pool().await;

        let mut fixture = settings_fixture();
        fixture.theme = "purple".into();
        let err = save_settings_core(&pool, fixture, &dir).await.unwrap_err();
        assert!(err.contains("主题值无效"));

        let mut fixture = settings_fixture();
        fixture.global_shortcut = "   ".into();
        let err = save_settings_core(&pool, fixture, &dir).await.unwrap_err();
        assert!(err.contains("不能为空"));

        let mut fixture = settings_fixture();
        fixture.auto_backup_keep = 0;
        let saved = save_settings_core(&pool, fixture, &dir).await.unwrap();
        assert_eq!(saved.auto_backup_keep, 1);

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn v03_values_are_validated() {
        let (pool, dir) = test_pool().await;

        // remindBeforeMinutes 只接受 0/15/30/60/1440
        let mut fixture = settings_fixture();
        fixture.remind_before_minutes = 45;
        let err = save_settings_core(&pool, fixture, &dir).await.unwrap_err();
        assert!(err.contains("截止前提醒时间无效"), "错误信息应为中文：{err}");

        for minutes in ALLOWED_REMIND_MINUTES {
            let mut fixture = settings_fixture();
            fixture.remind_before_minutes = minutes;
            let saved = save_settings_core(&pool, fixture, &dir).await.unwrap();
            assert_eq!(saved.remind_before_minutes, minutes);
        }

        // dailyDigestTime 必须 HH:MM
        let mut fixture = settings_fixture();
        fixture.daily_digest_time = "9 点".into();
        let err = save_settings_core(&pool, fixture, &dir).await.unwrap_err();
        assert!(err.contains("HH:MM"));

        // 主界面 / 搜索快捷键不能为空
        let mut fixture = settings_fixture();
        fixture.main_shortcut = "".into();
        let err = save_settings_core(&pool, fixture, &dir).await.unwrap_err();
        assert!(err.contains("主界面快捷键不能为空"));
        let mut fixture = settings_fixture();
        fixture.search_shortcut = " ".into();
        let err = save_settings_core(&pool, fixture, &dir).await.unwrap_err();
        assert!(err.contains("搜索快捷键不能为空"));

        // mutedUntil 写入与清空
        let mut fixture = settings_fixture();
        fixture.muted_until = Some("2026-11-05T00:00:00".into());
        let saved = save_settings_core(&pool, fixture, &dir).await.unwrap();
        assert_eq!(saved.muted_until.as_deref(), Some("2026-11-05T00:00:00"));

        let mut fixture = settings_fixture();
        fixture.muted_until = None;
        let saved = save_settings_core(&pool, fixture, &dir).await.unwrap();
        assert_eq!(saved.muted_until, None);

        // 非法 mutedUntil 被拒绝
        let mut fixture = settings_fixture();
        fixture.muted_until = Some("明天".into());
        assert!(save_settings_core(&pool, fixture, &dir).await.is_err());

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn mute_reminders_today_sets_tomorrow_midnight() {
        let (pool, dir) = test_pool().await;
        let settings = mute_reminders_today_core(&pool, &dir).await.unwrap();
        let expected = tomorrow_midnight();
        assert_eq!(settings.muted_until.as_deref(), Some(expected.as_str()));
        assert!(expected.ends_with("T00:00:00"));
        // 落库后重新读取仍生效
        let reloaded = load_settings(&pool, &dir).await.unwrap();
        assert_eq!(reloaded.muted_until, settings.muted_until);
        cleanup_test_pool(pool, dir).await;
    }
}
