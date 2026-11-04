//! 系统托盘：WorkNote 的第二阶段常驻入口。
//!
//! 行为契约：
//! - **图标来源**：`app.default_window_icon()`。tauri-codegen 在构建时已把 `bundle.icon` 里的窗口图标
//!   以内嵌 RGBA（`tauri::image::Image`）注入运行时，因此这里不需要读磁盘文件，也不依赖 exe 同目录资源。
//! - **降级**：万一 `default_window_icon()` 为 `None`，只打日志并跳过托盘创建，**绝不 panic**
//!   （否则应用直接起不来）。此时 `tray::is_ready()` 保持 false，主窗口关闭退回 v0.1 的退出行为，
//!   避免窗口被藏起来却没有任何入口能找回。
//! - **左键单击**：切换主窗口显隐（可见则隐藏；隐藏/最小化则还原 + 显示 + 聚焦）。
//! - **右键菜单**：打开主界面 / 快速记录 / --- / 退出，`show_menu_on_left_click(false)` 保证左键不弹菜单。
//! - **退出**：`app.exit(0)`，不弹确认框——前端自动保存窗口 600ms 内已把内容写进 SQLite，无需等待。

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
#[cfg(not(target_os = "macos"))]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::{show_main_window, show_quick_window, MAIN_WINDOW_LABEL};

/// 托盘 icon 的 id，后续要按 id 查找/更新托盘时用。
pub const TRAY_ID: &str = "worknote-tray";

/// 托盘菜单项 id（与 `on_menu_event` 里的 match 分支一一对应）。
const MENU_OPEN: &str = "open";
const MENU_QUICK: &str = "quick";
const MENU_QUIT: &str = "quit";

/// 托盘是否创建成功。主窗口 `CloseRequested` 时据此决定「隐藏到托盘」还是「直接退出」。
static TRAY_READY: AtomicBool = AtomicBool::new(false);

/// 托盘是否可用（图标缺失、创建失败时为 false）。
pub fn is_ready() -> bool {
    TRAY_READY.load(Ordering::Relaxed)
}

/// 创建系统托盘。返回 `Err` 只代表本次创建失败，调用方打日志后继续启动，不影响应用可用。
pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    // 托盘图标直接复用窗口图标：tauri-codegen 已把 bundle.icon 以内嵌 RGBA 注入。
    // 取不到图标时不能 panic（会让整个应用起不来），只降级为「没有托盘」。
    let Some(icon) = app.default_window_icon().cloned() else {
        eprintln!(
            "[WorkNote] 未取到默认窗口图标（default_window_icon 为 None），跳过系统托盘创建；\
             应用仍可正常启动，但关闭主窗口将直接退出（v0.1 行为）。"
        );
        return Ok(());
    };

    // 菜单：打开主界面 / 快速记录 / --- / 退出。
    let open = MenuItem::with_id(app, MENU_OPEN, "打开主界面", true, None::<&str>)?;
    let quick = MenuItem::with_id(app, MENU_QUICK, "快速记录", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出", true, None::<&str>)?;

    let items: [&dyn IsMenuItem<Wry>; 4] = [&open, &quick, &separator, &quit];
    let menu = Menu::with_items(app, &items)?;

    // Windows / Linux：左键切换主窗口，菜单走右键。
    // macOS：托盘图标在菜单栏里，左键就该弹菜单（系统习惯），切换窗口交给菜单项。
    let show_menu_on_left_click = cfg!(target_os = "macos");

    let builder = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("WorkNote · 日常记录")
        .menu(&menu)
        .show_menu_on_left_click(show_menu_on_left_click)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_OPEN => show_main_window(app),
            MENU_QUICK => show_quick_window(app),
            MENU_QUIT => {
                // 直接退出，不弹确认：前端 600ms 自动保存已入库。
                app.exit(0);
            }
            other => eprintln!("[WorkNote] 收到未知的托盘菜单项：{other}"),
        });

    #[cfg(not(target_os = "macos"))]
    let builder = builder.on_tray_icon_event(|tray, event| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            toggle_main_window(tray.app_handle());
        }
    });

    builder.build(app)?;

    TRAY_READY.store(true, Ordering::Relaxed);
    Ok(())
}

/// 托盘左键单击：主窗口可见且没最小化则隐藏，否则还原/显示并聚焦。
fn toggle_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        eprintln!("[WorkNote] 找不到主窗口（label={MAIN_WINDOW_LABEL}），托盘单击无窗口可切换");
        return;
    };

    let visible = window.is_visible().unwrap_or(false);
    let minimized = window.is_minimized().unwrap_or(false);

    if visible && !minimized {
        if let Err(err) = window.hide() {
            eprintln!("[WorkNote] 隐藏主窗口失败：{err}");
        }
    } else {
        // 隐藏、或「可见但最小化」都走显示路径（内部会先 unminimize 再 show + focus）。
        show_main_window(app);
    }
}
