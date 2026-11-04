//! 保密项目密码（v0.6）。
//!
//! 一个总密码，解锁后整个「保密项目」区可见；连续输错 3 次返回密码提示。
//! 存储：`app_settings` 里的 `secret_pw_hash` / `secret_pw_salt` / `secret_pw_hint`。
//! 解锁状态只存在内存（重启应用自动重新上锁）。

use sha2::{Digest, Sha256};
use tauri::State;

use crate::db;
use crate::db::models::{SecretStatus, UnlockResult};
use crate::AppState;

/// 连续输错几次后给提示
const MAX_ATTEMPTS: i64 = 3;
/// 哈希迭代轮数（本地应用，够用且启动不卡）
const ROUNDS: usize = 20_000;

const KEY_HASH: &str = "secret_pw_hash";
const KEY_SALT: &str = "secret_pw_salt";
const KEY_HINT: &str = "secret_pw_hint";

fn hash_password(salt: &str, password: &str) -> String {
    let mut digest = Sha256::digest(format!("{salt}:{password}").as_bytes()).to_vec();
    for _ in 0..ROUNDS {
        let mut hasher = Sha256::new();
        hasher.update(&digest);
        hasher.update(salt.as_bytes());
        hasher.update(password.as_bytes());
        digest = hasher.finalize().to_vec();
    }
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn make_salt() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}{:x}", std::process::id())
}

/// 读取已配置的密码哈希与盐
async fn stored_password(
    pool: &sqlx::SqlitePool,
) -> Result<Option<(String, String)>, String> {
    let hash = db::get_setting(pool, KEY_HASH).await?;
    let salt = db::get_setting(pool, KEY_SALT).await?;
    Ok(match (hash, salt) {
        (Some(h), Some(s)) if !h.trim().is_empty() && !s.trim().is_empty() => Some((h, s)),
        _ => None,
    })
}

#[tauri::command]
pub async fn secret_status(state: State<'_, AppState>) -> Result<SecretStatus, String> {
    let pool = state.pool.read().await.clone();
    let configured = stored_password(&pool).await?.is_some();
    let unlocked = state.secret.lock().map(|s| s.unlocked).unwrap_or(false);
    Ok(SecretStatus {
        configured,
        unlocked: configured && unlocked,
        hint: None,
    })
}

/// 首次设置密码 + 密码提示（提示用于连续输错 3 次后展示，必须填）。
#[tauri::command]
pub async fn set_secret_password(
    state: State<'_, AppState>,
    password: String,
    hint: String,
) -> Result<SecretStatus, String> {
    let pool = state.pool.read().await.clone();
    if stored_password(&pool).await?.is_some() {
        return Err("已经设置过保密密码，如需修改请用「修改密码」".to_string());
    }
    let password = password.trim();
    if password.chars().count() < 4 {
        return Err("密码至少 4 位".to_string());
    }
    let hint = hint.trim();
    if hint.is_empty() {
        return Err("密码提示必须填写（忘记密码时靠它提醒自己）".to_string());
    }

    let salt = make_salt();
    let hash = hash_password(&salt, password);
    db::set_setting(&pool, KEY_SALT, &salt).await?;
    db::set_setting(&pool, KEY_HASH, &hash).await?;
    db::set_setting(&pool, KEY_HINT, hint).await?;

    if let Ok(mut secret) = state.secret.lock() {
        secret.unlocked = true;
        secret.attempts = 0;
    }
    Ok(SecretStatus {
        configured: true,
        unlocked: true,
        hint: None,
    })
}

/// 修改密码（需要旧密码 + 新提示）。
#[tauri::command]
pub async fn change_secret_password(
    state: State<'_, AppState>,
    old_password: String,
    new_password: String,
    new_hint: String,
) -> Result<(), String> {
    let pool = state.pool.read().await.clone();
    let Some((hash, salt)) = stored_password(&pool).await? else {
        return Err("还没设置过保密密码".to_string());
    };
    if hash_password(&salt, old_password.trim()) != hash {
        return Err("旧密码不对".to_string());
    }
    let new_password = new_password.trim();
    if new_password.chars().count() < 4 {
        return Err("密码至少 4 位".to_string());
    }
    let new_hint = new_hint.trim();
    if new_hint.is_empty() {
        return Err("密码提示必须填写".to_string());
    }
    let new_salt = make_salt();
    let new_hash = hash_password(&new_salt, new_password);
    db::set_setting(&pool, KEY_SALT, &new_salt).await?;
    db::set_setting(&pool, KEY_HASH, &new_hash).await?;
    db::set_setting(&pool, KEY_HINT, new_hint).await?;
    Ok(())
}

/// 解锁；连续输错 3 次返回密码提示（随后计数清零，再错 3 次又给一次）。
#[tauri::command]
pub async fn unlock_secret(
    state: State<'_, AppState>,
    password: String,
) -> Result<UnlockResult, String> {
    let pool = state.pool.read().await.clone();
    let Some((hash, salt)) = stored_password(&pool).await? else {
        return Ok(UnlockResult {
            ok: false,
            attempts_left: 0,
            hint: None,
        });
    };

    if hash_password(&salt, password.trim()) == hash {
        if let Ok(mut secret) = state.secret.lock() {
            secret.unlocked = true;
            secret.attempts = 0;
        }
        return Ok(UnlockResult {
            ok: true,
            attempts_left: MAX_ATTEMPTS,
            hint: None,
        });
    }

    let mut attempts = 0;
    if let Ok(mut secret) = state.secret.lock() {
        secret.attempts += 1;
        attempts = secret.attempts;
    }
    let left = (MAX_ATTEMPTS - attempts).max(0);
    let hint = if attempts >= MAX_ATTEMPTS {
        // 到阈值：给出提示，并把计数清零，避免“永久锁死”在同一个提示循环里
        if let Ok(mut secret) = state.secret.lock() {
            secret.attempts = 0;
        }
        db::get_setting(&pool, KEY_HINT).await?.filter(|h| !h.trim().is_empty())
    } else {
        None
    };

    Ok(UnlockResult {
        ok: false,
        attempts_left: if attempts >= MAX_ATTEMPTS { 0 } else { left },
        hint,
    })
}

/// 手动上锁（解锁后想立刻遮起来）。
#[tauri::command]
pub async fn lock_secret(state: State<'_, AppState>) -> Result<(), String> {
    if let Ok(mut secret) = state.secret.lock() {
        secret.unlocked = false;
        secret.attempts = 0;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_and_salted() {
        let a = hash_password("salt1", "1234");
        let b = hash_password("salt1", "1234");
        let c = hash_password("salt2", "1234");
        assert_eq!(a, b, "同盐同密码必须得到同一哈希");
        assert_ne!(a, c, "不同盐必须得到不同哈希");
        assert_ne!(a, hash_password("salt1", "12345"));
        assert_eq!(a.len(), 64, "sha256 十六进制应为 64 字符");
    }

    #[test]
    fn salt_is_random() {
        assert_ne!(make_salt(), make_salt());
    }
}
