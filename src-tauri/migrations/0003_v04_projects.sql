-- WorkNote v0.4.0 增量迁移：项目（projects）+ 任务层级（parent_id）
-- 与 docs/接口契约-v0.4.md 第 1 节保持一致。
-- 注意：不要修改已交付的 0001 / 0002 —— 已有用户的库会按文件名顺序增量执行本文件。
-- 本迁移只加列 / 建表，不重写任何旧数据：旧记录的 project_id / parent_id / sort_order 都是默认值，
-- 行为与 v0.3 完全一致。

-- 1. 项目表：项目是最高层，不支持项目嵌套。
CREATE TABLE IF NOT EXISTS projects (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  name        TEXT    NOT NULL,
  description TEXT,
  color       TEXT,                                  -- indigo|teal|amber|rose|violet|slate，可空
  status      TEXT    NOT NULL DEFAULT 'active',     -- active|paused|done|archived
  sort_order  INTEGER NOT NULL DEFAULT 0,
  is_deleted  INTEGER NOT NULL DEFAULT 0,
  created_at  TEXT    NOT NULL,
  updated_at  TEXT    NOT NULL
);

-- 2. entries 增加归属项目与父任务。
ALTER TABLE entries ADD COLUMN project_id INTEGER REFERENCES projects(id);
ALTER TABLE entries ADD COLUMN parent_id  INTEGER REFERENCES entries(id);

-- 3. entries 增加排序权重：项目页任务列表 / 子任务拖拽排序都要落库。
--    契约第 1 节的 DDL 漏了这一列，但第 3 节 reorder_children（按数组顺序写 sort_order）、
--    list_children（按 sort_order 排序）与第 5 节顶层任务排序都依赖它，故在此补上；默认 0 不影响旧数据。
ALTER TABLE entries ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_projects_status ON projects(status, is_deleted);
CREATE INDEX IF NOT EXISTS idx_entries_project ON entries(project_id);
CREATE INDEX IF NOT EXISTS idx_entries_parent  ON entries(parent_id);
