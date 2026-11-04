//! 系统类命令：打开数据目录、重新注册全局快捷键。
//!
//! v0.3 起有两个全局快捷键（契约第 5 节）：
//! - `capture`：唤起快速记录小窗（`global_shortcut`，v1 的 `set_global_shortcut` 保留为它的别名）；
//! - `main`：显示并聚焦主窗口（`main_shortcut`）。
//!
//! `set_shortcut` 的顺序固定为「先注册新值 → 成功后才注销旧值与持久化」：
//! 注册失败时旧组合仍然有效，进程内状态与数据库都不会被改坏。

use tauri::{AppHandle, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::commands::settings::load_settings;
use crate::db::models::Settings;
use crate::{AppState, ShortcutKind};

/// 用系统文件管理器打开数据目录。
#[tauri::command]
pub async fn open_data_dir(state: State<'_, AppState>) -> Result<(), String> {
    let dir = state.paths.data_dir.clone();
    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("无法创建数据目录 {}：{e}", dir.display()))?;
    }
    tauri_plugin_opener::open_path(&dir, None::<&str>)
        .map_err(|e| format!("打开数据目录失败：{e}"))
}

/// 校验 accelerator 是否可解析（不改动任何注册状态）。
pub(crate) fn validate_accelerator(accelerator: &str) -> Result<Shortcut, String> {
    accelerator.parse::<Shortcut>().map_err(|_| {
        format!("无法识别的快捷键：{accelerator}（示例：CommandOrControl+Shift+N）")
    })
}

/// `set_shortcut` 的核心逻辑：先注册新值，成功才注销旧值并落库；任一步失败保留旧值。
pub async fn set_shortcut_core(
    app: &AppHandle,
    state: &AppState,
    kind: &str,
    accelerator: &str,
) -> Result<Settings, String> {
    let kind = ShortcutKind::parse(kind)?;
    let accelerator = accelerator.trim();
    if accelerator.is_empty() {
        return Err(format!("{}不能为空", kind.label()));
    }
    // 先做格式校验：非法组合不触发任何注册动作。
    validate_accelerator(accelerator)?;

    let old = state.shortcut(kind);
    if old == accelerator {
        // 已是当前值：启动时可能注册失败过，这里补注册一次。
        if !app.global_shortcut().is_registered(accelerator) {
            crate::register_shortcut(app, kind, accelerator)?;
        }
    } else {
        // 先注册新值：失败即返回，旧值完全不受影响（满足契约“失败回滚”）。
        crate::register_shortcut(app, kind, accelerator)?;
        // 新值注册成功后才注销旧值并更新内存状态。
        crate::unregister_shortcut(app, &old);
        state.set_shortcut_value(kind, accelerator.to_string());
    }

    // 持久化，下次启动按新值注册。
    let pool = state.pool.read().await;
    if let Err(err) = crate::db::set_setting(&pool, kind.setting_key(), accelerator).await {
        // 落库失败：把注册状态回滚到旧值，保证“设置失败时旧快捷键仍然有效”。
        crate::unregister_shortcut(app, accelerator);
        if old != accelerator {
            let _ = crate::register_shortcut(app, kind, &old);
            state.set_shortcut_value(kind, old.clone());
        }
        return Err(err);
    }
    load_settings(&pool, &state.paths.data_dir).await
}

/// v0.3：分别设置两个全局快捷键，返回落库后的最新 Settings。
#[tauri::command]
pub async fn set_shortcut(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: String,
    accelerator: String,
) -> Result<Settings, String> {
    set_shortcut_core(&app, state.inner(), &kind, &accelerator).await
}

/// v1 兼容命令：等价于 `set_shortcut(kind="capture")`。
#[tauri::command]
pub async fn set_global_shortcut(
    app: AppHandle,
    state: State<'_, AppState>,
    accelerator: String,
) -> Result<(), String> {
    set_shortcut_core(&app, state.inner(), ShortcutKind::Capture.as_str(), &accelerator).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_plugin_global_shortcut::Modifiers;

    #[test]
    fn default_accelerator_is_parseable() {
        let shortcut: Shortcut = "CommandOrControl+Shift+N".parse().expect("默认快捷键应可解析");
        // 至少包含 Control 与 Shift（Windows/Linux 下 CommandOrControl = Control）
        assert!(shortcut.mods.contains(Modifiers::SHIFT));
    }

    #[test]
    fn bad_accelerator_is_rejected() {
        assert!("不存在的键".parse::<Shortcut>().is_err());
        assert!("Ctrl+Shift".parse::<Shortcut>().is_err());
        let err = validate_accelerator("不存在的键").unwrap_err();
        assert!(err.contains("无法识别的快捷键"), "错误信息应为中文：{err}");
    }

    #[test]
    fn shortcut_kind_parses_capture_and_main() {
        assert_eq!(ShortcutKind::parse("capture").unwrap(), ShortcutKind::Capture);
        assert_eq!(ShortcutKind::parse(" main ").unwrap(), ShortcutKind::Main);
        let err = ShortcutKind::parse("search").unwrap_err();
        assert!(err.contains("capture / main"), "错误信息应为中文：{err}");
    }

    #[test]
    fn shortcut_kinds_map_to_distinct_setting_keys_and_actions() {
        assert_eq!(ShortcutKind::Capture.setting_key(), "global_shortcut");
        assert_eq!(ShortcutKind::Main.setting_key(), "main_shortcut");
        assert_ne!(
            ShortcutKind::Capture.setting_key(),
            ShortcutKind::Main.setting_key()
        );
        assert_eq!(ShortcutKind::Capture.label(), "快速记录快捷键");
        assert_eq!(ShortcutKind::Main.label(), "主界面快捷键");
    }

    #[test]
    fn main_shortcut_default_is_parseable() {
        let shortcut: Shortcut = crate::db::models::DEFAULT_MAIN_SHORTCUT
            .parse()
            .expect("默认 main 快捷键应可解析");
        assert!(shortcut.mods.contains(Modifiers::SHIFT));
    }
}
