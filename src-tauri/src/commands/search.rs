//! 全文搜索：契约明确不使用 FTS5，改用 `LIKE '%q%'` + 转义。
//!
//! 命中片段用 `\u{1}`（左）和 `\u{2}`（右）标记，前端据此高亮。

use sqlx::SqlitePool;
use tauri::State;

use crate::commands::entries::{ENTRY_COLUMNS, ENTRY_SOURCE};
use crate::db::models::SearchHit;
use crate::AppState;

/// 默认返回条数（契约第 4 节）。
pub const DEFAULT_SEARCH_LIMIT: u32 = 50;
/// 上限，防止前端传入巨大值把整库读出来。
pub const MAX_SEARCH_LIMIT: u32 = 1000;
/// snippet 命中点左右保留的字符数。
const SNIPPET_CONTEXT_CHARS: usize = 30;

/// 搜索结果行：`Entry` + 父任务标题（v0.4）。
///
/// `#[sqlx(flatten)]` 让 `Entry` 继续按列名从同一行读取；
/// `parent_content` 由子查询单独带出，顶层记录为 NULL。
#[derive(sqlx::FromRow)]
struct SearchRow {
    #[sqlx(flatten)]
    entry: crate::db::models::Entry,
    parent_content: Option<String>,
}

/// 转义 LIKE 通配符：`\`、`%`、`_`（SQL 里配合 `ESCAPE '\'` 使用）。
pub fn escape_like(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '%' => escaped.push_str("\\%"),
            '_' => escaped.push_str("\\_"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn lower_char(ch: char) -> char {
    ch.to_lowercase().next().unwrap_or(ch)
}

/// 生成带 `\u{1}` / `\u{2}` 标记的片段；找不到时退回正文开头预览。
pub fn make_snippet(content: &str, query: &str) -> String {
    let chars: Vec<char> = content.chars().collect();
    let query_chars: Vec<char> = query.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    if query_chars.is_empty() {
        let end = chars.len().min(SNIPPET_CONTEXT_CHARS * 2);
        let mut out: String = chars[..end].iter().collect();
        if end < chars.len() {
            out.push('…');
        }
        return out;
    }

    let query_lower: Vec<char> = query_chars.iter().copied().map(lower_char).collect();
    let mut hit: Option<usize> = None;
    if query_chars.len() <= chars.len() {
        for start in 0..=(chars.len() - query_chars.len()) {
            let window = chars[start..start + query_chars.len()]
                .iter()
                .copied()
                .map(lower_char);
            if window.eq(query_lower.iter().copied()) {
                hit = Some(start);
                break;
            }
        }
    }

    match hit {
        Some(start) => {
            let end = start + query_chars.len();
            let before_start = start.saturating_sub(SNIPPET_CONTEXT_CHARS);
            let after_end = (end + SNIPPET_CONTEXT_CHARS).min(chars.len());

            let mut out = String::new();
            if before_start > 0 {
                out.push('…');
            }
            out.extend(chars[before_start..start].iter());
            out.push('\u{1}');
            out.extend(chars[start..end].iter());
            out.push('\u{2}');
            out.extend(chars[end..after_end].iter());
            if after_end < chars.len() {
                out.push('…');
            }
            out
        }
        None => {
            let end = chars.len().min(SNIPPET_CONTEXT_CHARS * 2);
            let mut out: String = chars[..end].iter().collect();
            if end < chars.len() {
                out.push('…');
            }
            out
        }
    }
}

/// 搜索核心逻辑：空串直接返回空数组（不是错误）。
pub async fn search_entries_core(
    pool: &SqlitePool,
    query: &str,
    limit: Option<u32>,
) -> Result<Vec<SearchHit>, String> {
    let keyword = query.trim();
    if keyword.is_empty() {
        return Ok(Vec::new());
    }
    let limit = limit
        .unwrap_or(DEFAULT_SEARCH_LIMIT)
        .clamp(1, MAX_SEARCH_LIMIT) as i64;
    let pattern = format!("%{}%", escape_like(keyword));

    let sql = format!(
        "SELECT {ENTRY_COLUMNS}, \
                (SELECT p.content FROM entries p WHERE p.id = e0.parent_id) AS parent_content \
         FROM ({ENTRY_SOURCE}) e0 \
         WHERE e0.is_deleted = 0 AND e0.content LIKE ?1 ESCAPE '\\' \
         ORDER BY e0.day DESC, e0.created_at DESC, e0.id DESC LIMIT ?2"
    );
    let rows = sqlx::query_as::<_, SearchRow>(&sql)
        .bind(pattern)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("搜索失败：{e}"))?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let snippet = make_snippet(&row.entry.content, keyword);
            SearchHit {
                entry: row.entry,
                parent_content: row.parent_content,
                snippet,
            }
        })
        .collect())
}

#[tauri::command]
pub async fn search_entries(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<SearchHit>, String> {
    let pool = state.pool.read().await;
    search_entries_core(&pool, &query, limit).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::entries::{create_entry_core, create_entry_full_core};
    use crate::db::{cleanup_test_pool, test_pool};

    #[test]
    fn escape_like_handles_wildcards_and_backslash() {
        assert_eq!(escape_like("100%"), "100\\%");
        assert_eq!(escape_like("a_b"), "a\\_b");
        assert_eq!(escape_like("c\\d"), "c\\\\d");
        assert_eq!(escape_like("中文"), "中文");
    }

    #[test]
    fn snippet_marks_match_with_markers() {
        let s = make_snippet("今天买了苹果和香蕉", "苹果");
        assert_eq!(s, "今天买了\u{1}苹果\u{2}和香蕉");
        let s = make_snippet("Hello World", "world");
        assert_eq!(s, "Hello \u{1}World\u{2}");
        let s = make_snippet("没有命中", "xyz");
        assert!(!s.contains('\u{1}'));
    }

    #[test]
    fn snippet_truncates_long_content() {
        let content = format!("{}目标{}", "前".repeat(100), "后".repeat(100));
        let s = make_snippet(&content, "目标");
        assert!(s.starts_with('…'));
        assert!(s.ends_with('…'));
        assert!(s.contains('\u{1}'));
        // 字符级截断，不会切坏 UTF-8
        assert_eq!(s.chars().filter(|c| *c == '…').count(), 2);
    }

    #[tokio::test]
    async fn search_escapes_percent_and_underscore() {
        let (pool, dir) = test_pool().await;
        create_entry_core(&pool, "进度 100% 完成", false, None)
            .await
            .unwrap();
        create_entry_core(&pool, "进度 1000 完成", false, None)
            .await
            .unwrap();
        create_entry_core(&pool, "变量 a_b 说明", false, None)
            .await
            .unwrap();
        create_entry_core(&pool, "变量 axb 说明", false, None)
            .await
            .unwrap();

        // % 不应被当成通配符
        let hits = search_entries_core(&pool, "100%", None).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].snippet.contains('\u{1}'));

        // _ 不应被当成单字符通配符
        let hits = search_entries_core(&pool, "a_b", None).await.unwrap();
        assert_eq!(hits.len(), 1);

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn search_empty_query_and_deleted_rows() {
        let (pool, dir) = test_pool().await;
        create_entry_core(&pool, "关键字记录", false, None)
            .await
            .unwrap();

        assert!(search_entries_core(&pool, "", None).await.unwrap().is_empty());
        assert!(search_entries_core(&pool, "   ", None).await.unwrap().is_empty());

        let entry = create_entry_core(&pool, "待删除关键字", false, None)
            .await
            .unwrap();
        crate::commands::entries::delete_entry_core(&pool, entry.id)
            .await
            .unwrap();
        let hits = search_entries_core(&pool, "关键字", None).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entry.content, "关键字记录");

        // limit 生效
        for i in 0..5 {
            create_entry_core(&pool, &format!("关键字 {i}"), false, None)
                .await
                .unwrap();
        }
        let hits = search_entries_core(&pool, "关键字", Some(2)).await.unwrap();
        assert_eq!(hits.len(), 2);

        cleanup_test_pool(pool, dir).await;
    }

    // -----------------------------------------------------------------------
    // v0.4：搜索包含子任务，命中时带出 parentId
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn search_includes_children_and_exposes_parent_id() {
        let (pool, dir) = test_pool().await;
        let project = crate::commands::projects::create_project_core(&pool, "项目甲", None, None)
            .await
            .unwrap();
        let parent =
            create_entry_full_core(&pool, "父任务正文", true, None, None, Some(project.id))
                .await
                .unwrap();
        let child =
            crate::commands::children::create_child_core(&pool, parent.id, "关键字子任务")
                .await
                .unwrap();

        let hits = search_entries_core(&pool, "关键字", None).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entry.id, child.id);
        assert_eq!(
            hits[0].entry.parent_id,
            Some(parent.id),
            "命中子任务时能拿到 parentId"
        );
        assert_eq!(hits[0].entry.project_id, Some(project.id));
        assert!(hits[0].snippet.contains('\u{1}'));

        // 父任务本身也能被搜到，parentId 为空
        let hits = search_entries_core(&pool, "父任务", None).await.unwrap();
        assert!(
            hits.iter()
                .any(|hit| hit.entry.id == parent.id && hit.entry.parent_id.is_none())
        );

        cleanup_test_pool(pool, dir).await;
    }

    #[tokio::test]
    async fn search_hit_carries_parent_content_only_for_children() {
        let (pool, dir) = test_pool().await;
        let parent = create_entry_full_core(&pool, "父任务标题", true, None, None, None)
            .await
            .unwrap();
        let child = crate::commands::children::create_child_core(&pool, parent.id, "关键字子任务")
            .await
            .unwrap();
        // 父任务改名后，子任务命中时带出的应是新的父任务标题
        let mut patch = crate::db::models::EntryPatch::default();
        patch.content = Some("另一个父任务词".into());
        crate::commands::entries::update_entry_core(&pool, parent.id, patch)
            .await
            .unwrap();

        let hits = search_entries_core(&pool, "关键字", None).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entry.id, child.id);
        assert_eq!(
            hits[0].parent_content.as_deref(),
            Some("另一个父任务词"),
            "命中子任务时 parentContent 是父任务标题"
        );

        let hits = search_entries_core(&pool, "另一个父任务词", None)
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entry.id, parent.id);
        assert_eq!(hits[0].parent_content, None, "顶层记录的 parentContent 为 null");

        cleanup_test_pool(pool, dir).await;
    }
}
