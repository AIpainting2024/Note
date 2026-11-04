-- WorkNote 初始表结构
-- 与 docs/接口契约-v1.md 第 2 节保持一致；时间一律为本地时间字符串。

CREATE TABLE IF NOT EXISTS entries (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  day          TEXT    NOT NULL,             -- 本地日期 YYYY-MM-DD，按天查询走索引
  content      TEXT    NOT NULL,             -- 原始文本（轻 Markdown 存储，前端渲染）
  is_todo      INTEGER NOT NULL DEFAULT 0,
  due_date     TEXT,                         -- YYYY-MM-DD，可空
  completed_at TEXT,                         -- YYYY-MM-DDTHH:MM:SS 本地，可空
  is_pinned    INTEGER NOT NULL DEFAULT 0,
  is_deleted   INTEGER NOT NULL DEFAULT 0,   -- 软删除
  created_at   TEXT    NOT NULL,
  updated_at   TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_entries_day  ON entries(day);
CREATE INDEX IF NOT EXISTS idx_entries_todo ON entries(is_todo, is_deleted, completed_at);
CREATE INDEX IF NOT EXISTS idx_entries_due  ON entries(due_date);

CREATE TABLE IF NOT EXISTS app_settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
