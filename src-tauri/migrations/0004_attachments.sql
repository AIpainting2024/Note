-- 图片附件：文件落在数据目录 attachments/ 下，库里只存元数据。
-- entry_id 允许为空：快速输入框里粘贴的图片先落库，等记录保存后再认领（claim）。
CREATE TABLE IF NOT EXISTS attachments (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  entry_id    INTEGER REFERENCES entries(id) ON DELETE SET NULL,
  rel_path    TEXT NOT NULL,
  mime        TEXT NOT NULL,
  byte_size   INTEGER NOT NULL,
  is_deleted  INTEGER NOT NULL DEFAULT 0,
  created_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_attachments_entry ON attachments(entry_id);
CREATE INDEX IF NOT EXISTS idx_attachments_orphan ON attachments(entry_id, created_at);
