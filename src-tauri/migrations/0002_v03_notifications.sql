-- WorkNote v0.3.0 增量迁移：待办截止时间 + 软件内通知
-- 与 docs/接口契约-v0.3.md 第 1 节保持一致。
-- 注意：不要修改已交付的 0001_init.sql —— 已有用户的库会按文件名顺序增量执行本文件。

-- 1.1 entries 增加可选截止时间：HH:MM，可空。为空 = 只有日期没有具体时间。
ALTER TABLE entries ADD COLUMN due_time TEXT;

-- 1.2 notifications：软件内通知（不接 Windows 系统通知）。
CREATE TABLE IF NOT EXISTS notifications (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  kind          TEXT    NOT NULL,        -- due_soon | daily_digest
  entry_id      INTEGER,                 -- 关联待办；汇总类为 NULL
  title         TEXT    NOT NULL,
  body          TEXT    NOT NULL,
  dedupe_key    TEXT    NOT NULL,        -- 防重复，见契约 1.2
  scheduled_at  TEXT    NOT NULL,        -- 计划发出时间 YYYY-MM-DDTHH:MM:SS（snooze 时后移）
  emitted_at    TEXT,                    -- 实际已推送给前端的时刻；NULL=尚未发出
  read_at       TEXT,
  dismissed_at  TEXT,
  created_at    TEXT    NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_notifications_dedupe ON notifications(dedupe_key);
CREATE INDEX IF NOT EXISTS idx_notifications_pending ON notifications(emitted_at, scheduled_at);
