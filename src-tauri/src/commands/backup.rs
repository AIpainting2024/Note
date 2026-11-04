//! 备份 / 恢复 / 导出 / 自动备份。
//!
//! 关键约束（docs/接口契约-v1.md 第 5 节）：
//! - 备份一律用 `VACUUM INTO`（WAL 下的一致性快照，禁止直接复制 db 文件）。
//! - 恢复：先 VACUUM INTO 备份当前库 → 关闭连接池 → 覆盖 db（清理 -wal/-shm）→ 重开连接池。
//!   任一步失败都要保留原库并返回明确错误；恢复后 `last_backup_at` 不变。
//! - 自动备份：启动时及每 60 分钟检查一次，距上次超过 24 小时才备份，
//!   并按 `auto_backup_keep` 从旧到新清理。

use std::path::{Path, PathBuf};

use chrono::Local;
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

use crate::commands::entries::select_sql;
use crate::commands::settings::load_settings;
use crate::db::models::{BackupInfo, Entry, Project};
use crate::db::{
    self, display_datetime, now_timestamp, parse_backup_file_name, parse_timestamp, today_string,
    Paths,
};
use crate::AppState;

// ---------------------------------------------------------------------------
// 备份
// ---------------------------------------------------------------------------

/// `VACUUM INTO '<绝对路径>'`。目标文件必须不存在（文件名由 `unique_backup_path` 保证唯一）。
async fn vacuum_into(pool: &SqlitePool, dest: &Path) -> Result<(), String> {
    let dest = std::path::absolute(dest)
        .map_err(|e| format!("无法解析备份路径 {}：{e}", dest.display()))?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("无法创建备份目录 {}：{e}", parent.display()))?;
    }
    if dest.exists() {
        return Err(format!("备份文件已存在：{}", dest.display()));
    }

    let dest_text = dest.to_string_lossy().to_string();
    sqlx::query("VACUUM INTO ?1")
        .bind(dest_text)
        .execute(pool)
        .await
        .map_err(|e| format!("备份失败（VACUUM INTO）：{e}"))?;

    if !dest.exists() {
        return Err(format!("备份失败：未生成备份文件 {}", dest.display()));
    }
    Ok(())
}

/// 生成一个不冲突的备份路径：主文件名 `worknote_YYYY-MM-DD_HHMMSS.db`，
/// 同一秒内重复备份时追加 `-2`、`-3` 后缀，绝不覆盖已有备份。
fn unique_backup_path(paths: &Paths) -> PathBuf {
    let base = db::backup_file_name();
    let first = paths.backups_dir.join(&base);
    if !first.exists() {
        return first;
    }
    let stem = base.strip_suffix(".db").unwrap_or(&base);
    for index in 2..=999u32 {
        let candidate = paths.backups_dir.join(format!("{stem}-{index}.db"));
        if !candidate.exists() {
            return candidate;
        }
    }
    // 极端情况（同一秒 999 份备份）：退化成纳秒级文件名。
    let nanos = Local::now().timestamp_nanos_opt().unwrap_or_default();
    paths.backups_dir.join(format!("{stem}-{nanos}.db"))
}

/// 生成一份备份文件，但**不**更新 `last_backup_at`（恢复流程的“恢复前备份”用）。
pub async fn create_backup_file(pool: &SqlitePool, paths: &Paths) -> Result<BackupInfo, String> {
    paths.ensure_dirs()?;
    let dest = unique_backup_path(paths);
    vacuum_into(pool, &dest).await?;
    backup_info_from_path(&dest)
        .ok_or_else(|| format!("备份已生成但无法读取其信息：{}", dest.display()))
}

fn backup_info_from_path(path: &Path) -> Option<BackupInfo> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let file_name = path.file_name()?.to_string_lossy().to_string();
    let created_at = parse_backup_file_name(&file_name)
        .map(display_datetime)
        .or_else(|| {
            metadata.modified().ok().map(|modified| {
                let local: chrono::DateTime<Local> = modified.into();
                local.format("%Y-%m-%d %H:%M:%S").to_string()
            })
        })
        .unwrap_or_else(now_timestamp);
    Some(BackupInfo {
        path: path.display().to_string(),
        size_bytes: metadata.len() as i64,
        created_at,
    })
}

pub fn list_backups_core(paths: &Paths) -> Result<Vec<BackupInfo>, String> {
    let read_dir = match std::fs::read_dir(&paths.backups_dir) {
        Ok(read_dir) => read_dir,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(format!(
                "读取备份目录 {} 失败：{e}",
                paths.backups_dir.display()
            ))
        }
    };

    let mut infos = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with("worknote_") || !name.ends_with(".db") {
            continue;
        }
        if let Some(info) = backup_info_from_path(&path) {
            infos.push(info);
        }
    }
    // 按创建时间倒序；同一秒内按文件名兜底，保证顺序稳定。
    infos.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| b.path.cmp(&a.path)));
    Ok(infos)
}

/// 立即备份：生成文件并更新 `last_backup_at`。
pub async fn backup_now_core(pool: &SqlitePool, paths: &Paths) -> Result<BackupInfo, String> {
    let info = create_backup_file(pool, paths).await?;
    db::set_setting(pool, "last_backup_at", &now_timestamp()).await?;
    Ok(info)
}

/// 按保留份数从旧到新清理。
pub fn prune_backups(paths: &Paths, keep: i64) -> Result<(), String> {
    let keep = keep.max(1) as usize;
    let mut backups = list_backups_core(paths)?;
    while backups.len() > keep {
        let oldest = backups
            .pop()
            .ok_or_else(|| "清理旧备份失败：备份列表为空".to_string())?;
        std::fs::remove_file(&oldest.path)
            .map_err(|e| format!("清理旧备份失败 {}：{e}", oldest.path))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 恢复
// ---------------------------------------------------------------------------

/// 恢复前校验备份文件：能打开、完整性检查通过、且确实是 WorkNote 的库。
async fn validate_backup_file(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("备份文件不存在：{}", path.display()));
    }
    if !path.is_file() {
        return Err(format!("所选路径不是文件：{}", path.display()));
    }

    let pool = db::open_read_only(path).await?;
    let integrity_result = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await;
    let tables_result = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('entries', 'app_settings')",
    )
    .fetch_one(&pool)
    .await;
    let _ = pool.close().await;

    let integrity = integrity_result.map_err(|e| {
        format!(
            "所选文件不是有效的 SQLite 数据库（{}）：{e}",
            path.display()
        )
    })?;
    if integrity != "ok" {
        return Err(format!("备份文件完整性检查未通过：{integrity}"));
    }
    let table_count = tables_result.map_err(|e| format!("无法读取备份文件结构：{e}"))?;
    if table_count != 2 {
        return Err("所选文件不是 WorkNote 备份（缺少 entries / app_settings 表）".to_string());
    }
    Ok(())
}

fn paths_point_to_same_file(left: &Path, right: &Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(a), Ok(b)) => a == b,
        _ => left == right,
    }
}

/// 打开恢复后的库并补齐迁移/默认设置。
async fn reopen_and_prepare(db_path: &Path) -> Result<SqlitePool, String> {
    let pool = db::build_pool(db_path).await?;
    if let Err(err) = db::run_migrations(&pool).await {
        pool.close().await;
        return Err(err);
    }
    if let Err(err) = db::ensure_default_settings(&pool).await {
        pool.close().await;
        return Err(err);
    }
    Ok(pool)
}

/// 把 `source` 复制到 `db_path`（先写临时文件再替换），并清理 -wal/-shm。
///
/// Windows 上文件句柄释放/杀软扫描是异步的：连接池刚关闭时立刻 `rename` 覆盖 `db_path`
/// 可能短暂返回「拒绝访问」，所以做有限次重试（最多约 1 秒），而不是直接判定恢复失败。
fn replace_database_file(source: &Path, db_path: &Path) -> Result<(), String> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("无法创建数据库目录 {}：{e}", parent.display()))?;
    }
    let tmp_path = db_path.with_extension("db.restore-tmp");
    let _ = std::fs::remove_file(&tmp_path);
    std::fs::copy(source, &tmp_path).map_err(|e| format!("恢复失败：无法复制备份文件：{e}"))?;

    const REPLACE_ATTEMPTS: u32 = 20;
    let mut last_error = None;
    for attempt in 0..REPLACE_ATTEMPTS {
        // 每次替换前都清理附属文件：旧 WAL/SHM 残留会让 SQLite 打开后状态不一致。
        for suffix in ["-wal", "-shm"] {
            let _ = std::fs::remove_file(db::sidecar_path(db_path, suffix));
        }
        match std::fs::rename(&tmp_path, db_path) {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_error = Some(err);
                if attempt + 1 < REPLACE_ATTEMPTS {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
    }

    let _ = std::fs::remove_file(&tmp_path);
    let reason = last_error
        .map(|e| e.to_string())
        .unwrap_or_else(|| "未知错误".to_string());
    Err(format!("恢复失败：无法替换数据库文件：{reason}"))
}

/// 恢复后写回 `last_backup_at`：契约要求 `restore_backup` 之后该值不变，
/// 而备份文件里的值可能比当前库旧，所以恢复成功后按恢复前读到的值覆盖回去。
async fn restore_last_backup_at(pool: &SqlitePool, value: Option<&str>) {
    let result = match value {
        Some(value) => db::set_setting(pool, "last_backup_at", value).await,
        None => sqlx::query("DELETE FROM app_settings WHERE key = 'last_backup_at'")
            .execute(pool)
            .await
            .map(|_| ())
            .map_err(|e| format!("清除 last_backup_at 失败：{e}")),
    };
    if let Err(err) = result {
        eprintln!("[WorkNote] 恢复后写回 last_backup_at 失败：{err}");
    }
}

/// 恢复备份。失败时尽最大努力保留/回退原库。
pub async fn restore_backup_core(state: &AppState, backup_path: &str) -> Result<(), String> {
    let raw = backup_path.trim();
    if raw.is_empty() {
        return Err("请选择要恢复的备份文件".to_string());
    }
    let source = PathBuf::from(raw);

    // 1) 恢复前先校验目标备份，避免动到当前库。
    validate_backup_file(&source).await?;
    if paths_point_to_same_file(&source, &state.paths.db_path) {
        return Err("不能使用当前正在使用的数据库文件作为恢复来源".to_string());
    }

    // 记录恢复前的 last_backup_at：契约要求恢复后该值不变。
    let last_backup_before = {
        let pool = state.pool.read().await;
        db::get_setting(&pool, "last_backup_at")
            .await
            .unwrap_or(None)
    };

    // 2) 先用 VACUUM INTO 备份当前库（不更新 last_backup_at）。
    let safety_backup = {
        let pool = state.pool.read().await;
        create_backup_file(&pool, &state.paths)
            .await
            .map_err(|e| format!("恢复前备份当前数据库失败，已取消恢复：{e}"))?
    };

    // 3) 关池 → 覆盖 db → 重开池；全程持有写锁，避免恢复期间命令读到半成品。
    let mut guard = state.pool.write().await;
    guard.close().await;

    let db_path = state.paths.db_path.clone();
    if let Err(err) = replace_database_file(&source, &db_path) {
        // 替换未生效（临时文件已清理），原库不受影响：把原库重新打开。
        if let Ok(pool) = reopen_and_prepare(&db_path).await {
            *guard = pool;
        }
        return Err(err);
    }

    match reopen_and_prepare(&db_path).await {
        Ok(pool) => {
            restore_last_backup_at(&pool, last_backup_before.as_deref()).await;
            *guard = pool;
            Ok(())
        }
        Err(err) => {
            // 新库打不开：用恢复前的备份回滚。
            if let Err(rollback_err) = replace_database_file(Path::new(&safety_backup.path), &db_path)
            {
                return Err(format!(
                    "恢复失败：{err}；回退到恢复前数据库也失败（{rollback_err}）；\
                     恢复前备份位于 {}，请手动恢复",
                    safety_backup.path
                ));
            }
            match reopen_and_prepare(&db_path).await {
                Ok(pool) => {
                    *guard = pool;
                    Err(format!(
                        "恢复失败：{err}。已回退到恢复前的数据库（恢复前备份：{}）",
                        safety_backup.path
                    ))
                }
                Err(reopen_err) => Err(format!(
                    "恢复失败：{err}；回退后的数据库仍无法打开（{reopen_err}）；\
                     恢复前备份位于 {}，请手动恢复",
                    safety_backup.path
                )),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 导出
// ---------------------------------------------------------------------------

fn write_file(path: &Path, contents: &[u8]) -> Result<(), String> {
    let path = std::path::absolute(path)
        .map_err(|e| format!("无法解析导出路径 {}：{e}", path.display()))?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("无法创建导出目录 {}：{e}", parent.display()))?;
        }
    }
    std::fs::write(&path, contents).map_err(|e| format!("写入文件 {} 失败：{e}", path.display()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonExport {
    exported_at: String,
    /// v0.4：项目全量导出（含归档，不含已删除）。
    projects: Vec<Project>,
    entries: Vec<Entry>,
}

pub async fn export_json_core(pool: &SqlitePool, path: &Path) -> Result<(), String> {
    // v0.4：走统一数据源，entries 才能带上 projectId / parentId / projectName 等字段。
    // 导出是完整备份语义，子任务也一并导出（不按顶层过滤）。
    let sql = select_sql("WHERE is_deleted = 0 ORDER BY day ASC, created_at ASC, id ASC");
    let entries = sqlx::query_as::<_, Entry>(&sql)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("读取导出数据失败：{e}"))?;
    let projects = crate::commands::projects::list_projects_core(pool, true, &today_string()).await?;
    let payload = JsonExport {
        exported_at: now_timestamp(),
        projects,
        entries,
    };
    let json =
        serde_json::to_string_pretty(&payload).map_err(|e| format!("生成 JSON 失败：{e}"))?;
    write_file(path, json.as_bytes())
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

pub async fn export_csv_core(pool: &SqlitePool, path: &Path) -> Result<(), String> {
    // v0.4：走统一数据源以带出项目名；子任务一并导出（parent_id 列不写进 CSV，保持既有列）。
    let sql = select_sql("WHERE is_deleted = 0 ORDER BY day ASC, created_at ASC, id ASC");
    let entries = sqlx::query_as::<_, Entry>(&sql)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("读取导出数据失败：{e}"))?;

    // 带 BOM，Excel 打开中文不乱码；列定义见契约第 4 节。
    // v0.4 在末尾追加「项目」列（无项目留空），保持既有列顺序不变。
    let mut out = String::from("\u{feff}日期,时间,内容,是否待办,截止日期,完成时间,项目\r\n");
    for entry in entries {
        let time = entry.created_at.get(11..19).unwrap_or("");
        let cells = [
            entry.day,
            time.to_string(),
            entry.content,
            if entry.is_todo { "是" } else { "否" }.to_string(),
            entry.due_date.unwrap_or_default(),
            entry.completed_at.unwrap_or_default(),
            entry.project_name.unwrap_or_default(),
        ];
        let line = cells
            .iter()
            .map(|cell| csv_escape(cell))
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&line);
        out.push_str("\r\n");
    }
    write_file(path, out.as_bytes())
}

// ---------------------------------------------------------------------------
// 自动备份
// ---------------------------------------------------------------------------

/// 自动备份检查：开关打开且距上次备份超过 24 小时才执行，并按保留份数清理。
pub async fn auto_backup_tick(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;

    let state = app.state::<AppState>();

    let settings = {
        let pool = state.pool.read().await;
        load_settings(&pool, &state.paths.data_dir).await?
    };
    if !settings.auto_backup {
        return Ok(());
    }

    let due = match settings.last_backup_at.as_deref() {
        Some(raw) => match parse_timestamp(raw) {
            Some(last) => {
                Local::now().naive_local().signed_duration_since(last).num_hours() >= 24
            }
            None => true,
        },
        None => true,
    };
    if !due {
        return Ok(());
    }

    let info = {
        let pool = state.pool.read().await;
        backup_now_core(&pool, &state.paths).await?
    };
    eprintln!("[WorkNote] 自动备份完成：{}", info.path);

    prune_backups(&state.paths, settings.auto_backup_keep)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

fn export_path(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("导出路径不能为空".to_string());
    }
    Ok(PathBuf::from(trimmed))
}

#[tauri::command]
pub async fn backup_now(state: State<'_, AppState>) -> Result<BackupInfo, String> {
    let pool = state.pool.read().await;
    backup_now_core(&pool, &state.paths).await
}

#[tauri::command]
pub async fn list_backups(state: State<'_, AppState>) -> Result<Vec<BackupInfo>, String> {
    list_backups_core(&state.paths)
}

#[tauri::command]
pub async fn restore_backup(state: State<'_, AppState>, path: String) -> Result<(), String> {
    restore_backup_core(state.inner(), &path).await
}

#[tauri::command]
pub async fn export_json(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let path = export_path(&path)?;
    let pool = state.pool.read().await;
    export_json_core(&pool, &path).await
}

#[tauri::command]
pub async fn export_csv(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let path = export_path(&path)?;
    let pool = state.pool.read().await;
    export_csv_core(&pool, &path).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::entries::{create_entry_core, list_entries_core};
    use crate::db::{connect, today_string};

    /// 建一个用临时目录 + 真实连接池的 AppState（db 文件就在 paths.db_path）。
    async fn test_state() -> (AppState, PathBuf) {
        // 并发测试时 pid + 纳秒仍可能撞名，再叠加进程内自增序号保证唯一。
        static TEST_DIR_SEQ: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        let seq = TEST_DIR_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "worknote-backup-test-{}-{}-{}",
            std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap_or_default(),
            seq
        ));
        let data_dir = root.join("data");
        let paths = Paths {
            db_path: data_dir.join("database").join("worknote.db"),
            backups_dir: data_dir.join("backups"),
            exports_dir: data_dir.join("exports"),
            logs_dir: data_dir.join("logs"),
            attachments_dir: data_dir.join("attachments"),
            data_dir,
        };
        paths.ensure_dirs().unwrap();
        let pool = connect(&paths).await.unwrap();
        let state = AppState {
            pool: tokio::sync::RwLock::new(pool),
            paths,
            shortcuts: std::sync::Mutex::new(crate::Shortcuts::default()),
        };
        (state, root)
    }

    async fn state_pool(state: &AppState) -> SqlitePool {
        state.pool.read().await.clone()
    }

    #[tokio::test]
    async fn backup_uses_vacuum_and_updates_last_backup_at() {
        let (state, root) = test_state().await;
        let pool = state_pool(&state).await;
        create_entry_core(&pool, "备份内容", false, None)
            .await
            .unwrap();

        let info = backup_now_core(&pool, &state.paths).await.unwrap();
        assert!(Path::new(&info.path).is_file());
        assert!(info.size_bytes > 0);
        assert_eq!(info.created_at.len(), 19);

        let raw = db::get_setting(&pool, "last_backup_at").await.unwrap();
        assert!(raw.is_some());
        assert!(parse_timestamp(&raw.unwrap()).is_some());

        // 备份文件是可直接打开的独立 SQLite 库，且包含刚才的记录
        let backup_pool = db::build_pool(Path::new(&info.path)).await.unwrap();
        let content: String = sqlx::query_scalar("SELECT content FROM entries LIMIT 1")
            .fetch_one(&backup_pool)
            .await
            .unwrap();
        assert_eq!(content, "备份内容");
        backup_pool.close().await;

        pool.close().await;
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn list_backups_sorted_desc_and_prune() {
        let (state, root) = test_state().await;
        let pool = state_pool(&state).await;

        let fresh = create_backup_file(&pool, &state.paths).await.unwrap();
        let old_path = state.paths.backups_dir.join("worknote_2020-01-01_000000.db");
        std::fs::copy(&fresh.path, &old_path).unwrap();
        let middle_path = state.paths.backups_dir.join("worknote_2021-01-01_000000.db");
        std::fs::copy(&fresh.path, &middle_path).unwrap();

        let list = list_backups_core(&state.paths).unwrap();
        assert_eq!(list.len(), 3);
        assert!(list[0].created_at > list[1].created_at);
        assert!(list[1].created_at > list[2].created_at);

        prune_backups(&state.paths, 2).unwrap();
        let list = list_backups_core(&state.paths).unwrap();
        assert_eq!(list.len(), 2);
        assert!(!old_path.exists());

        pool.close().await;
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn restore_roundtrip_keeps_data_and_last_backup_at() {
        let (state, root) = test_state().await;
        let pool = state_pool(&state).await;
        create_entry_core(&pool, "恢复前记录", false, None)
            .await
            .unwrap();
        let backup = backup_now_core(&pool, &state.paths).await.unwrap();
        let last_backup_before = db::get_setting(&pool, "last_backup_at").await.unwrap();

        // 备份之后再写一条，恢复后应消失
        create_entry_core(&pool, "备份之后才写的", false, None)
            .await
            .unwrap();
        assert_eq!(list_entries_core(&pool, &today_string()).await.unwrap().len(), 2);

        restore_backup_core(&state, &backup.path).await.unwrap();

        let restored_pool = state_pool(&state).await;
        let entries = list_entries_core(&restored_pool, &today_string()).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "恢复前记录");

        // 恢复后 last_backup_at 不变（契约第 5 节）
        let last_backup_after = db::get_setting(&restored_pool, "last_backup_at")
            .await
            .unwrap();
        assert_eq!(last_backup_before, last_backup_after);

        // 恢复前的当前库已被自动备份，备份目录里至少 2 份
        assert!(list_backups_core(&state.paths).unwrap().len() >= 2);

        restored_pool.close().await;
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn restore_rejects_invalid_file_without_touching_db() {
        let (state, root) = test_state().await;
        let pool = state_pool(&state).await;
        create_entry_core(&pool, "别弄丢我", false, None)
            .await
            .unwrap();

        let bad = state.paths.backups_dir.join("bad.db");
        std::fs::write(&bad, b"not a sqlite file").unwrap();

        let err = restore_backup_core(&state, &bad.display().to_string())
            .await
            .unwrap_err();
        assert!(!err.is_empty());

        // 原库仍然可读
        let entries = list_entries_core(&pool, &today_string()).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "别弄丢我");

        pool.close().await;
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn same_second_backups_do_not_overwrite_each_other() {
        let (state, root) = test_state().await;
        let pool = state_pool(&state).await;
        create_entry_core(&pool, "第一份", false, None)
            .await
            .unwrap();
        let first = create_backup_file(&pool, &state.paths).await.unwrap();

        create_entry_core(&pool, "第二份", false, None)
            .await
            .unwrap();
        let second = create_backup_file(&pool, &state.paths).await.unwrap();
        assert_ne!(first.path, second.path);
        assert!(Path::new(&first.path).is_file());
        assert!(Path::new(&second.path).is_file());

        // 先备份的文件内容不能被后来的备份覆盖（只应有 1 条记录）
        let first_pool = db::build_pool(Path::new(&first.path)).await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entries")
            .fetch_one(&first_pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
        first_pool.close().await;

        pool.close().await;
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn exports_write_json_and_csv() {
        let (state, root) = test_state().await;
        let pool = state_pool(&state).await;
        create_entry_core(&pool, "逗号,引号\"测试", true, Some("2026-12-01".into()))
            .await
            .unwrap();
        let json_path = state.paths.exports_dir.join("out.json");
        let csv_path = state.paths.exports_dir.join("out.csv");

        export_json_core(&pool, &json_path).await.unwrap();
        let json = std::fs::read_to_string(&json_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("exportedAt").is_some());
        let entries = parsed.get("entries").unwrap().as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].get("isTodo").unwrap().as_bool(), Some(true));
        assert_eq!(
            entries[0].get("dueDate").unwrap().as_str(),
            Some("2026-12-01")
        );

        export_csv_core(&pool, &csv_path).await.unwrap();
        let csv = std::fs::read_to_string(&csv_path).unwrap();
        assert!(csv.starts_with('\u{feff}'));
        assert!(csv.contains("日期,时间,内容,是否待办,截止日期,完成时间"));
        assert!(csv.contains("\"逗号,引号\"\"测试\""));

        pool.close().await;
        let _ = std::fs::remove_dir_all(root);
    }

    // -----------------------------------------------------------------------
    // v0.4：导出增加 projects 数组与「项目」列
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn exports_include_projects_and_project_column() {
        let (state, root) = test_state().await;
        let pool = state_pool(&state).await;
        let project = crate::commands::projects::create_project_core(
            &pool,
            "导出项目",
            Some("teal".into()),
            None,
        )
        .await
        .unwrap();
        crate::commands::entries::create_entry_full_core(
            &pool,
            "带项目记录",
            true,
            None,
            None,
            Some(project.id),
        )
        .await
        .unwrap();
        let json_path = state.paths.exports_dir.join("v04.json");
        let csv_path = state.paths.exports_dir.join("v04.csv");

        export_json_core(&pool, &json_path).await.unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap()).unwrap();
        let projects = parsed.get("projects").unwrap().as_array().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(
            projects[0].get("name").unwrap().as_str(),
            Some("导出项目")
        );
        assert_eq!(projects[0].get("taskTotal").unwrap().as_i64(), Some(1));
        let entries = parsed.get("entries").unwrap().as_array().unwrap();
        assert_eq!(entries[0].get("projectId").unwrap().as_i64(), Some(project.id));
        assert!(entries[0].get("parentId").unwrap().is_null());

        export_csv_core(&pool, &csv_path).await.unwrap();
        let csv = std::fs::read_to_string(&csv_path).unwrap();
        assert!(csv.contains("日期,时间,内容,是否待办,截止日期,完成时间,项目"));
        assert!(csv.contains("导出项目"));

        pool.close().await;
        let _ = std::fs::remove_dir_all(root);
    }
}
