//! 图片附件：粘贴 / 选择文件上传 → 落盘到数据目录 `attachments/` → 正文里用 `att:<id>` 引用。
//!
//! 只处理图片（png/jpeg/gif/webp/bmp/svg）。文件按 `att-<时间戳>-<序号>.<ext>` 命名，
//! 库里存相对路径，前端用 `convertFileSrc` 拼成可显示的 URL。

use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use sqlx::SqlitePool;
use tauri::State;

use crate::db::{self, models::Attachment};
use crate::AppState;

/// 单张图片上限：12 MB（够截图/照片，避免误传大文件把库撑爆）
const MAX_BYTES: usize = 12 * 1024 * 1024;

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// 由 MIME 推断扩展名；非图片返回 None。
fn ext_of(mime: &str) -> Option<&'static str> {
    match mime.to_ascii_lowercase().as_str() {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/bmp" => Some("bmp"),
        "image/svg+xml" => Some("svg"),
        _ => None,
    }
}

/// 由扩展名推断 MIME（上传本地文件时用）。
fn mime_of_ext(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

async fn insert_attachment(
    pool: &SqlitePool,
    paths: &db::Paths,
    entry_id: Option<i64>,
    mime: &str,
    bytes: &[u8],
) -> Result<Attachment, String> {
    if bytes.is_empty() {
        return Err("图片内容为空".to_string());
    }
    if bytes.len() > MAX_BYTES {
        return Err(format!(
            "图片太大（{} MB），上限 {} MB",
            bytes.len() / 1024 / 1024,
            MAX_BYTES / 1024 / 1024
        ));
    }
    let ext = ext_of(mime).ok_or_else(|| "目前只支持图片（png / jpg / gif / webp / bmp / svg）".to_string())?;

    if let Some(id) = entry_id {
        let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM entries WHERE id = ?1 AND is_deleted = 0")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
        if exists.is_none() {
            return Err("目标记录不存在，可能已被删除".to_string());
        }
    }

    std::fs::create_dir_all(&paths.attachments_dir)
        .map_err(|e| format!("无法创建附件目录：{e}"))?;

    let file_name = format!("att-{}-{}.{}", now_millis(), uuid_suffix(), ext);
    let abs_path = paths.attachments_dir.join(&file_name);
    std::fs::write(&abs_path, bytes).map_err(|e| format!("写入图片失败：{e}"))?;

    let rel_path = format!("attachments/{file_name}");
    let created_at = db::now_timestamp();
    let result = sqlx::query(
        "INSERT INTO attachments (entry_id, rel_path, mime, byte_size, is_deleted, created_at) \
         VALUES (?1, ?2, ?3, ?4, 0, ?5)",
    )
    .bind(entry_id)
    .bind(&rel_path)
    .bind(mime)
    .bind(bytes.len() as i64)
    .bind(&created_at)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(Attachment {
        id: result.last_insert_rowid(),
        entry_id,
        mime: mime.to_string(),
        byte_size: bytes.len() as i64,
        path: abs_path.to_string_lossy().to_string(),
        created_at,
    })
}

/// 文件名里的随机尾巴：同一毫秒内多次上传也不撞名。
fn uuid_suffix() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{n:x}{nanos:04x}")
}

fn to_abs(paths: &db::Paths, rel_path: &str) -> String {
    paths
        .data_dir
        .join(rel_path)
        .to_string_lossy()
        .to_string()
}

/// 粘贴图片：前端把 File 读成 base64 传过来。
#[tauri::command]
pub async fn save_attachment(
    state: State<'_, AppState>,
    entry_id: Option<i64>,
    mime: String,
    data_base64: String,
) -> Result<Attachment, String> {
    let raw = data_base64
        .split(',')
        .last()
        .unwrap_or(&data_base64)
        .trim();
    let bytes = BASE64
        .decode(raw)
        .map_err(|e| format!("图片数据解析失败：{e}"))?;

    let pool = state.pool.read().await.clone();
    insert_attachment(&pool, &state.paths, entry_id, &mime, &bytes).await
}

/// 选择本地文件上传：路径来自 dialog 插件。
#[tauri::command]
pub async fn attach_file(
    state: State<'_, AppState>,
    entry_id: Option<i64>,
    file_path: String,
) -> Result<Attachment, String> {
    let path = std::path::Path::new(&file_path);
    let meta = std::fs::metadata(path).map_err(|e| format!("读不到这个文件：{e}"))?;
    if meta.len() as usize > MAX_BYTES {
        return Err(format!(
            "图片太大（{} MB），上限 {} MB",
            meta.len() / 1024 / 1024,
            MAX_BYTES / 1024 / 1024
        ));
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| "这个文件没有扩展名，认不出图片类型".to_string())?;
    let mime = mime_of_ext(ext).ok_or_else(|| "目前只支持图片文件".to_string())?;
    let bytes = std::fs::read(path).map_err(|e| format!("读取文件失败：{e}"))?;

    let pool = state.pool.read().await.clone();
    insert_attachment(&pool, &state.paths, entry_id, mime, &bytes).await
}

/// 列出某条记录下的图片。
#[tauri::command]
pub async fn list_attachments(
    state: State<'_, AppState>,
    entry_id: i64,
) -> Result<Vec<Attachment>, String> {
    let pool = state.pool.read().await.clone();
    let rows: Vec<(i64, Option<i64>, String, String, i64, String)> = sqlx::query_as(
        "SELECT id, entry_id, rel_path, mime, byte_size, created_at \
         FROM attachments WHERE entry_id = ?1 AND is_deleted = 0 ORDER BY id",
    )
    .bind(entry_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|(id, entry_id, rel_path, mime, byte_size, created_at)| Attachment {
            id,
            entry_id,
            mime,
            byte_size,
            path: to_abs(&state.paths, &rel_path),
            created_at,
        })
        .collect())
}

/// 认领实现：把正文里 `att:<id>` 指向、且还没归属的图片挂到这条记录上。
pub(crate) async fn claim_from_content(
    pool: &SqlitePool,
    entry_id: i64,
    content: &str,
) -> Result<usize, String> {
    let ids = parse_markers(content);
    if ids.is_empty() {
        return Ok(0);
    }
    let mut claimed = 0usize;
    for id in ids {
        let result = sqlx::query(
            "UPDATE attachments SET entry_id = ?1 WHERE id = ?2 AND entry_id IS NULL AND is_deleted = 0",
        )
        .bind(entry_id)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
        claimed += result.rows_affected() as usize;
    }
    Ok(claimed)
}

/// 快速输入框里粘贴的图片先挂在空 entry_id 上，记录保存后用正文里的 `att:<id>` 认领。
#[tauri::command]
pub async fn claim_attachments(
    state: State<'_, AppState>,
    entry_id: i64,
    content: String,
) -> Result<usize, String> {
    let pool = state.pool.read().await.clone();
    claim_from_content(&pool, entry_id, &content).await
}

/// 删除一张图片：行标记删除 + 删文件。
#[tauri::command]
pub async fn delete_attachment(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let pool = state.pool.read().await.clone();
    let row: Option<(String,)> =
        sqlx::query_as("SELECT rel_path FROM attachments WHERE id = ?1 AND is_deleted = 0")
            .bind(id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;
    let Some((rel_path,)) = row else {
        return Ok(());
    };
    sqlx::query("UPDATE attachments SET is_deleted = 1 WHERE id = ?1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;
    let abs = state.paths.data_dir.join(&rel_path);
    let _ = std::fs::remove_file(abs);
    Ok(())
}

/// 清理超过 1 天还没被认领的孤儿图片（粘贴了但记录没保存）。
pub async fn sweep_orphans(pool: &SqlitePool, paths: &db::Paths) -> Result<usize, String> {
    let cutoff = db::now_timestamp();
    let cutoff_dt = db::parse_timestamp(&cutoff).ok_or_else(|| "时间解析失败".to_string())?;
    let cutoff = (cutoff_dt - chrono::Duration::days(1))
        .format(db::TIMESTAMP_FORMAT)
        .to_string();

    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, rel_path FROM attachments \
         WHERE entry_id IS NULL AND is_deleted = 0 AND created_at < ?1",
    )
    .bind(&cutoff)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    for (id, rel_path) in &rows {
        sqlx::query("UPDATE attachments SET is_deleted = 1 WHERE id = ?1")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(paths.data_dir.join(rel_path));
    }
    Ok(rows.len())
}

/// 从正文里解析出 `att:<id>`（也接受 `![](att:12)` 这种 Markdown 图片写法）。
fn parse_markers(content: &str) -> Vec<i64> {
    let mut ids = Vec::new();
    let mut rest = content;
    while let Some(pos) = rest.find("att:") {
        rest = &rest[pos + 4..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(id) = digits.parse::<i64>() {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        if digits.is_empty() {
            continue;
        }
        rest = &rest[digits.len()..];
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_markers() {
        assert_eq!(parse_markers("看这个 ![](att:12) 和 ![](att:13)"), vec![12, 13]);
        assert_eq!(parse_markers("att:7"), vec![7]);
        assert_eq!(parse_markers("没有图片"), Vec::<i64>::new());
        assert_eq!(parse_markers("att:12 att:12"), vec![12]);
    }

    #[test]
    fn mime_roundtrip() {
        assert_eq!(ext_of("image/png"), Some("png"));
        assert_eq!(mime_of_ext("png"), Some("image/png"));
        assert_eq!(ext_of("application/pdf"), None);
        assert_eq!(mime_of_ext("pdf"), None);
    }

    fn temp_paths(tag: &str) -> crate::db::Paths {
        let root = std::env::temp_dir().join(format!(
            "worknote-att-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let data_dir = root.join("data");
        let paths = crate::db::Paths {
            db_path: data_dir.join("database").join("worknote.db"),
            backups_dir: data_dir.join("backups"),
            exports_dir: data_dir.join("exports"),
            logs_dir: data_dir.join("logs"),
            attachments_dir: data_dir.join("attachments"),
            data_dir,
        };
        paths.ensure_dirs().unwrap();
        paths
    }

    async fn pool_with_entry(paths: &crate::db::Paths) -> (SqlitePool, i64) {
        let pool = crate::db::build_pool(&paths.db_path).await.unwrap();
        crate::db::run_migrations(&pool).await.unwrap();
        crate::db::ensure_default_settings(&pool).await.unwrap();
        let now = crate::db::now_timestamp();
        let today = crate::db::today_string();
        let res = sqlx::query(
            "INSERT INTO entries (day, content, is_todo, is_pinned, is_deleted, created_at, updated_at, sort_order) \
             VALUES (?1, '带图记录', 0, 0, 0, ?2, ?2, 0)",
        )
        .bind(&today)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();
        (pool, res.last_insert_rowid())
    }

    #[tokio::test]
    async fn saves_file_and_row() {
        let paths = temp_paths("save");
        let (pool, entry_id) = pool_with_entry(&paths).await;

        let att = insert_attachment(&pool, &paths, Some(entry_id), "image/png", b"\x89PNG\r\n fake")
            .await
            .unwrap();

        assert!(std::path::Path::new(&att.path).exists(), "图片文件应落盘");
        assert!(att.path.ends_with(".png"));
        assert_eq!(att.entry_id, Some(entry_id));
        assert!(att.byte_size > 0);

        let list: Vec<(i64,)> =
            sqlx::query_as("SELECT id FROM attachments WHERE entry_id = ?1")
                .bind(entry_id)
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(list.len(), 1);
    }

    #[tokio::test]
    async fn rejects_non_image_and_claims_by_content() {
        let paths = temp_paths("claim");
        let (pool, entry_id) = pool_with_entry(&paths).await;

        // 非图片被拒
        let err = insert_attachment(&pool, &paths, Some(entry_id), "application/pdf", b"%PDF")
            .await
            .unwrap_err();
        assert!(err.contains("只支持图片"), "错误文案应说明只支持图片：{err}");

        // 先挂空 entry_id（模拟快速输入框粘贴），再按正文认领
        let att = insert_attachment(&pool, &paths, None, "image/jpeg", b"\xff\xd8\xff fake")
            .await
            .unwrap();
        assert_eq!(att.entry_id, None);

        let claimed = claim_from_content(&pool, entry_id, &format!("看这个 ![](att:{})", att.id))
            .await
            .unwrap();
        assert_eq!(claimed, 1);

        let row: (Option<i64>,) = sqlx::query_as("SELECT entry_id FROM attachments WHERE id = ?1")
            .bind(att.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.0, Some(entry_id));

        // 重复认领不会再次生效
        let again = claim_from_content(&pool, entry_id, &format!("att:{}", att.id))
            .await
            .unwrap();
        assert_eq!(again, 0);
    }
}
