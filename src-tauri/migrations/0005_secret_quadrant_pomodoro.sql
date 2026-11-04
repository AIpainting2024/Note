-- v0.6：保密项目 / 四象限 / 番茄钟

-- 保密项目：设了 is_secret 的项目归到侧边栏「保密项目」区，需密码解锁后才可见
ALTER TABLE projects ADD COLUMN is_secret INTEGER NOT NULL DEFAULT 0;

-- 四象限：1=重要且紧急 2=重要不紧急 3=紧急不重要 4=不重要不紧急；NULL=未划分
ALTER TABLE entries ADD COLUMN quadrant INTEGER;

-- 番茄钟记录（一次专注或休息一条）
CREATE TABLE IF NOT EXISTS pomodoro_sessions (
  id               INTEGER PRIMARY KEY AUTOINCREMENT,
  entry_id         INTEGER REFERENCES entries(id) ON DELETE SET NULL,
  kind             TEXT NOT NULL DEFAULT 'focus',  -- focus | break
  duration_seconds INTEGER NOT NULL,
  day              TEXT NOT NULL,                  -- YYYY-MM-DD（本地日期，便于统计）
  started_at       TEXT NOT NULL,
  ended_at         TEXT NOT NULL,
  completed        INTEGER NOT NULL DEFAULT 1,
  created_at       TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pomodoro_day ON pomodoro_sessions(day);
CREATE INDEX IF NOT EXISTS idx_entries_quadrant ON entries(quadrant);

-- 保密密码相关设置（口径固定，避免打错 key）
--   secret_pw_hash  sha256(salt + password) 迭代若干轮的十六进制
--   secret_pw_salt  随机盐
--   secret_pw_hint  密码提示（连续输错 3 次才显示）
