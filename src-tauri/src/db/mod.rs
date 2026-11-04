//! 数据目录解析、SQLite 连接池与迁移、通用设置读写与时间工具。
//!
//! 数据目录约定（见 docs/接口契约-v1.md 第 1 节）：
//!
//! ```text
//! Windows: %APPDATA%\WorkNote\
//! macOS:   ~/Library/Application Support/WorkNote/
//! Linux:   $XDG_DATA_HOME/WorkNote/ 或 ~/.local/share/WorkNote/
//!     database/worknote.db
//!     backups/
//!     exports/
//!     logs/
//! ```
//!
//! 可用环境变量 `WORKNOTE_DATA_DIR` 覆盖根目录（测试用）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{Datelike, Days, Local, NaiveDate, NaiveDateTime, NaiveTime};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;

pub mod models;

/// 应用目录名（%APPDATA% 下）。
pub const APP_DIR_NAME: &str = "WorkNote";
/// 本地时间戳格式（SQLite 里存的字符串）。
pub const TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H:%M:%S";
/// 本地日期格式。
pub const DATE_FORMAT: &str = "%Y-%m-%d";
/// 时刻格式（`entries.due_time` / 每日汇总时间）。
pub const TIME_FORMAT: &str = "%H:%M";

/// 数据目录下的全部路径。
#[derive(Debug, Clone)]
pub struct Paths {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub backups_dir: PathBuf,
    pub exports_dir: PathBuf,
    pub logs_dir: PathBuf,
}

/// 跨平台的用户数据根目录。
///
/// - Windows：`%APPDATA%`
/// - macOS：`~/Library/Application Support`
/// - 其他类 Unix：`$XDG_DATA_HOME`，回退 `~/.local/share`
fn user_data_base() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").map_err(|_| {
            "无法定位数据目录：未找到 APPDATA 环境变量，可用 WORKNOTE_DATA_DIR 指定".to_string()
        })?;
        if appdata.trim().is_empty() {
            return Err("无法定位数据目录：APPDATA 环境变量为空".to_string());
        }
        Ok(PathBuf::from(appdata.trim()))
    }

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").map_err(|_| {
            "无法定位数据目录：未找到 HOME 环境变量，可用 WORKNOTE_DATA_DIR 指定".to_string()
        })?;
        if home.trim().is_empty() {
            return Err("无法定位数据目录：HOME 环境变量为空".to_string());
        }
        Ok(PathBuf::from(home.trim()).join("Library").join("Application Support"))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            if !xdg.trim().is_empty() {
                return Ok(PathBuf::from(xdg.trim()));
            }
        }
        let home = std::env::var("HOME").map_err(|_| {
            "无法定位数据目录：未找到 HOME 环境变量，可用 WORKNOTE_DATA_DIR 指定".to_string()
        })?;
        if home.trim().is_empty() {
            return Err("无法定位数据目录：HOME 环境变量为空".to_string());
        }
        Ok(PathBuf::from(home.trim()).join(".local").join("share"))
    }
}

impl Paths {
    /// 解析数据目录。优先 `WORKNOTE_DATA_DIR`，否则用平台默认的用户数据目录 + `WorkNote`。
    pub fn resolve() -> Result<Self, String> {
        let raw = match std::env::var("WORKNOTE_DATA_DIR") {
            Ok(value) if !value.trim().is_empty() => PathBuf::from(value.trim()),
            _ => user_data_base()?.join(APP_DIR_NAME),
        };

        // SQLite 的 VACUUM INTO 需要绝对路径，这里统一转绝对路径（不解析符号链接，
        // 避免 Windows 上出现 SQLite 不适用的 \\?\ 前缀）。
        let data_dir = if raw.is_absolute() {
            raw
        } else {
            std::env::current_dir()
                .map_err(|e| format!("无法解析当前目录：{e}"))?
                .join(raw)
        };

        let db_dir = data_dir.join("database");
        Ok(Self {
            db_path: db_dir.join("worknote.db"),
            backups_dir: data_dir.join("backups"),
            exports_dir: data_dir.join("exports"),
            logs_dir: data_dir.join("logs"),
            data_dir,
        })
    }

    /// 建好全部目录（数据库文件所在目录必须存在，SQLite 不会自动建目录）。
    pub fn ensure_dirs(&self) -> Result<(), String> {
        let dirs = [
            &self.data_dir,
            self.db_path.parent().unwrap_or(&self.data_dir),
            &self.backups_dir,
            &self.exports_dir,
            &self.logs_dir,
        ];
        for dir in dirs {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("无法创建数据目录 {}：{e}", dir.display()))?;
        }
        Ok(())
    }

    /// `data_dir` 的展示字符串。
    pub fn data_dir_string(&self) -> String {
        self.data_dir.display().to_string()
    }
}

/// 打开（必要时创建）数据库、跑迁移、补齐默认设置。应用启动时调用一次。
pub async fn connect(paths: &Paths) -> Result<SqlitePool, String> {
    paths.ensure_dirs()?;
    let pool = build_pool(&paths.db_path).await?;
    run_migrations(&pool).await?;
    ensure_default_settings(&pool).await?;
    Ok(pool)
}

/// 按契约创建连接池：WAL、外键、5s busy_timeout。
pub async fn build_pool(db_path: &Path) -> Result<SqlitePool, String> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("无法创建数据库目录 {}：{e}", parent.display()))?;
    }
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_millis(5000));

    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .map_err(|e| format!("无法打开数据库 {}：{e}", db_path.display()))
}

/// 以只读方式打开一个数据库文件（用于恢复前校验备份文件）。
pub async fn open_read_only(db_path: &Path) -> Result<SqlitePool, String> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .read_only(true)
        .busy_timeout(Duration::from_millis(5000));
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|e| format!("无法打开文件 {}：{e}", db_path.display()))
}

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), String> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| format!("数据库迁移失败：{e}"))
}

/// 首次启动时补上默认设置；已存在的值不会被覆盖。
pub async fn ensure_default_settings(pool: &SqlitePool) -> Result<(), String> {
    let defaults: [(&str, String); 9] = [
        ("theme", models::DEFAULT_THEME.to_string()),
        (
            "auto_backup",
            if models::DEFAULT_AUTO_BACKUP { "true" } else { "false" }.to_string(),
        ),
        (
            "auto_backup_keep",
            models::DEFAULT_AUTO_BACKUP_KEEP.to_string(),
        ),
        (
            "global_shortcut",
            models::DEFAULT_GLOBAL_SHORTCUT.to_string(),
        ),
        ("main_shortcut", models::DEFAULT_MAIN_SHORTCUT.to_string()),
        (
            "search_shortcut",
            models::DEFAULT_SEARCH_SHORTCUT.to_string(),
        ),
        (
            "remind_before_minutes",
            models::DEFAULT_REMIND_BEFORE_MINUTES.to_string(),
        ),
        (
            "daily_digest_enabled",
            if models::DEFAULT_DAILY_DIGEST_ENABLED {
                "true"
            } else {
                "false"
            }
            .to_string(),
        ),
        (
            "daily_digest_time",
            models::DEFAULT_DAILY_DIGEST_TIME.to_string(),
        ),
    ];

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("数据库事务开启失败：{e}"))?;
    for (key, value) in defaults.iter() {
        sqlx::query("INSERT OR IGNORE INTO app_settings (key, value) VALUES (?1, ?2)")
            .bind(*key)
            .bind(value)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("初始化默认设置失败：{e}"))?;
    }
    tx.commit()
        .await
        .map_err(|e| format!("初始化默认设置失败：{e}"))?;
    Ok(())
}

/// 读取单个设置项。
pub async fn get_setting(pool: &SqlitePool, key: &str) -> Result<Option<String>, String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM app_settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("读取设置失败：{e}"))
}

/// 写入单个设置项（不存在则插入）。
pub async fn set_setting(pool: &SqlitePool, key: &str, value: &str) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await
    .map_err(|e| format!("保存设置失败：{e}"))?;
    Ok(())
}

/// 读取整张 settings 表为 map。
pub async fn load_settings_map(pool: &SqlitePool) -> Result<HashMap<String, String>, String> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM app_settings")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("读取设置失败：{e}"))?;
    Ok(rows.into_iter().collect())
}

/// SQLite 的附属文件路径（`-wal` / `-shm`）。
pub fn sidecar_path(db_path: &Path, suffix: &str) -> PathBuf {
    let mut path = db_path.as_os_str().to_os_string();
    path.push(suffix);
    PathBuf::from(path)
}

/// 当前本地时间戳：`YYYY-MM-DDTHH:MM:SS`。
pub fn now_timestamp() -> String {
    Local::now().format(TIMESTAMP_FORMAT).to_string()
}

/// 当前本地日期：`YYYY-MM-DD`。
pub fn today_string() -> String {
    Local::now().format(DATE_FORMAT).to_string()
}

/// 解析数据库里的时间戳字符串。
pub fn parse_timestamp(value: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(value, TIMESTAMP_FORMAT).ok()
}

/// 备份文件名：`worknote_YYYY-MM-DD_HHMMSS.db`（契约第 5 节）。
pub fn backup_file_name() -> String {
    Local::now().format("worknote_%Y-%m-%d_%H%M%S.db").to_string()
}

/// 从备份文件名解析出创建时间，失败返回 None。
///
/// 兼容同名冲突时追加的 `-2` / `-3` 后缀（如 `worknote_2026-11-03_201500-2.db`）。
pub fn parse_backup_file_name(name: &str) -> Option<NaiveDateTime> {
    let stem = name.strip_prefix("worknote_")?.strip_suffix(".db")?;
    // 基础部分固定为 "YYYY-MM-DD_HHMMSS"（17 个字符），后面只允许 `-N` 冲突后缀。
    let base = stem.get(..17)?;
    let rest = &stem[17..];
    if !rest.is_empty() && !rest.starts_with('-') {
        return None;
    }
    NaiveDateTime::parse_from_str(base, "%Y-%m-%d_%H%M%S").ok()
}

/// 把 `NaiveDateTime` 格式化成用户可读的本地时间。
pub fn display_datetime(value: NaiveDateTime) -> String {
    value.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// 校验并规范化截止日期：空串/纯空白视作“无截止日期”。
pub fn normalize_due_date(value: Option<String>) -> Result<Option<String>, String> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    chrono::NaiveDate::parse_from_str(trimmed, DATE_FORMAT)
        .map(|_| Some(trimmed.to_string()))
        .map_err(|_| format!("截止日期格式无效（应为 YYYY-MM-DD）：{trimmed}"))
}

/// 校验并规范化截止时间：空串/纯空白视作“没有具体时间”。
pub fn normalize_due_time(value: Option<String>) -> Result<Option<String>, String> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    NaiveTime::parse_from_str(trimmed, TIME_FORMAT)
        .map(|_| Some(trimmed.to_string()))
        .map_err(|_| format!("截止时间格式无效（应为 HH:MM）：{trimmed}"))
}

/// 解析 `HH:MM` 时刻（也用于 `dailyDigestTime`）。
pub fn parse_time(value: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(value.trim(), TIME_FORMAT).ok()
}

/// 解析 `YYYY-MM-DD` 日期。
pub fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.trim(), DATE_FORMAT).ok()
}

/// 把 `due_date + due_time` 组合成截止时刻；任一缺失/非法返回 `None`。
pub fn due_datetime(due_date: &str, due_time: &str) -> Option<NaiveDateTime> {
    let date = parse_date(due_date)?;
    let time = parse_time(due_time)?;
    Some(date.and_time(time))
}

/// 当前本地时间（不含时区）。
pub fn now_datetime() -> NaiveDateTime {
    Local::now().naive_local()
}

/// 把 `NaiveDateTime` 格式化成 SQLite 里统一的时间戳字符串。
pub fn format_timestamp(value: NaiveDateTime) -> String {
    value.format(TIMESTAMP_FORMAT).to_string()
}

/// 把日期格式化成 `YYYY-MM-DD`。
pub fn format_date(value: NaiveDate) -> String {
    value.format(DATE_FORMAT).to_string()
}

/// 明天 00:00（`mute_reminders_today` 的静音截止时刻）。
pub fn tomorrow_midnight() -> String {
    let tomorrow = Local::now().date_naive() + Days::new(1);
    format_timestamp(tomorrow.and_time(NaiveTime::MIN))
}

/// 把日期格式化成中文展示用的「11月4日」（不补前导零）。
pub fn display_month_day(value: NaiveDate) -> String {
    format!("{}月{}日", value.month(), value.day())
}

/// 测试用：在临时目录建一个跑过迁移的库。
#[cfg(test)]
pub async fn test_pool() -> (SqlitePool, PathBuf) {
    use std::sync::atomic::{AtomicU64, Ordering};

    // 并发跑测试时只用 pid + 纳秒仍可能撞名（Windows 时间精度有限），
    // 再叠加一个进程内自增序号，保证每个测试目录绝对唯一。
    static TEST_DIR_SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = TEST_DIR_SEQ.fetch_add(1, Ordering::Relaxed);
    let unique = format!(
        "worknote-test-{}-{}-{}",
        std::process::id(),
        Local::now().timestamp_nanos_opt().unwrap_or_default(),
        seq
    );
    let dir = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&dir).expect("创建测试目录失败");
    let db_path = dir.join("test.db");
    let pool = build_pool(&db_path).await.expect("打开测试数据库失败");
    run_migrations(&pool).await.expect("测试库迁移失败");
    ensure_default_settings(&pool).await.expect("测试库默认设置失败");
    (pool, dir)
}

/// 测试用：关闭连接池并删除临时目录。
#[cfg(test)]
pub async fn cleanup_test_pool(pool: SqlitePool, dir: PathBuf) {
    let _ = pool.close().await;
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_file_name_is_parseable_including_collision_suffix() {
        assert!(parse_backup_file_name("worknote_2026-11-03_201500.db").is_some());
        assert!(parse_backup_file_name("worknote_2026-11-03_201500-2.db").is_some());
        assert!(parse_backup_file_name("worknote_2026-11-03_201500-999.db").is_some());
        assert!(parse_backup_file_name("worknote_backup.db").is_none());
        assert!(parse_backup_file_name("random.db").is_none());
        assert!(parse_backup_file_name("worknote_2026-11-03_201500.txt").is_none());
    }

    #[test]
    fn due_date_validation() {
        assert_eq!(normalize_due_date(None).unwrap(), None);
        assert_eq!(normalize_due_date(Some("  ".into())).unwrap(), None);
        assert_eq!(
            normalize_due_date(Some("2026-12-01".into())).unwrap(),
            Some("2026-12-01".to_string())
        );
        assert!(normalize_due_date(Some("2026-13-01".into())).is_err());
    }

    #[test]
    fn due_time_validation() {
        assert_eq!(normalize_due_time(None).unwrap(), None);
        assert_eq!(normalize_due_time(Some("".into())).unwrap(), None);
        assert_eq!(
            normalize_due_time(Some("09:30".into())).unwrap(),
            Some("09:30".to_string())
        );
        assert!(normalize_due_time(Some("9:30:00".into())).is_err());
        assert!(normalize_due_time(Some("25:00".into())).is_err());
        assert!(parse_time("09:30").is_some());
        assert!(parse_time("9点").is_none());
    }

    #[tokio::test]
    async fn migration_0002_upgrades_existing_v02_database() {
        // 模拟已交付用户的 v0.2 库：只应用 0001，再插入旧数据。
        let root = std::env::temp_dir().join(format!(
            "worknote-migration-test-{}-{}",
            std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let db_path = root.join("worknote.db");
        let pool = build_pool(&db_path).await.unwrap();

        let all = sqlx::migrate!("./migrations");
        let mut first_only = all.migrations.to_vec();
        first_only.truncate(1); // 只保留 0001_init.sql
        sqlx::migrate::Migrator {
            migrations: std::borrow::Cow::Owned(first_only),
            ignore_missing: false,
            locking: true,
            no_tx: false,
        }
        .run(&pool)
        .await
        .unwrap();

        // v0.2 的表结构里没有 due_time / notifications。
        let old_columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info('entries')")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(!old_columns.contains(&"due_time".to_string()));
        sqlx::query(
            "INSERT INTO entries (day, content, is_todo, created_at, updated_at) \
             VALUES ('2026-11-01', '旧数据', 1, '2026-11-01T09:00:00', '2026-11-01T09:00:00')",
        )
        .execute(&pool)
        .await
        .unwrap();

        // 与启动路径一致：跑完整迁移（0001 已记录，只增量执行 0002）。
        run_migrations(&pool).await.unwrap();
        ensure_default_settings(&pool).await.unwrap();

        let columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info('entries')")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(columns.contains(&"due_time".to_string()));
        let notifications: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'notifications'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(notifications, 1);

        // 旧数据保留，且 due_time 为 NULL 可正常解码。
        let entry: crate::db::models::Entry =
            sqlx::query_as("SELECT id, day, content, is_todo, due_date, due_time, completed_at, \
                            is_pinned, is_deleted, created_at, updated_at FROM entries LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(entry.content, "旧数据");
        assert_eq!(entry.due_time, None);

        // 新字段的默认设置也补齐了。
        assert_eq!(
            get_setting(&pool, "main_shortcut").await.unwrap(),
            Some(models::DEFAULT_MAIN_SHORTCUT.to_string())
        );

        pool.close().await;
        let _ = std::fs::remove_dir_all(root);
    }
}
