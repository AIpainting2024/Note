//! WorkNote Tauri 后端入口。
//!
//! 职责：
//! 1. 启动时建数据目录、打开 SQLite（WAL/外键/busy_timeout）、跑迁移、补默认设置；
//! 2. 注册全部 Tauri 命令（清单见 docs/接口契约-v1.md 第 4 节、v0.3.md 第 3 节）；
//! 3. 创建系统托盘（见 `tray` 模块）：关闭主窗口 = 隐藏到托盘，进程常驻；
//! 4. 按设置注册两个全局快捷键（capture / main），见 `docs/接口契约-v0.3.md` 第 5 节；
//! 5. 启动时及每 60 分钟检查一次自动备份；启动时及每 60 秒跑一次提醒调度。
//!
//! 改命令名/参数名/返回结构前必须同步 docs 下的接口契约，前端按契约写死了调用。

mod commands;
mod db;
mod tray;

use std::sync::{Mutex as StdMutex, MutexGuard};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use db::models::{DEFAULT_GLOBAL_SHORTCUT, DEFAULT_MAIN_SHORTCUT};
use db::Paths;

/// 主窗口的窗口 label（与 tauri.conf.json 中一致）。
pub const MAIN_WINDOW_LABEL: &str = "main";
/// 快速记录小窗的窗口 label（与 tauri.conf.json 中一致）。
pub const QUICK_WINDOW_LABEL: &str = "quick";
/// 小窗保存后广播给主窗口的事件名。
pub const ENTRIES_CHANGED_EVENT: &str = "entries-changed";

/// v0.3 起有两个全局快捷键，触发行为不同（契约第 5 节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutKind {
    /// 唤起快速记录小窗（默认 `CommandOrControl+Shift+N`）。
    Capture,
    /// 显示并聚焦主窗口（默认 `CommandOrControl+Shift+M`）。
    Main,
}

impl ShortcutKind {
    /// 解析前端传入的 `kind`：只接受 `capture` / `main`。
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim() {
            "capture" => Ok(Self::Capture),
            "main" => Ok(Self::Main),
            other => Err(format!(
                "未知的快捷键类型：{other}（可选 capture / main）"
            )),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Main => "main",
        }
    }

    /// 错误提示里用的中文名。
    pub const fn label(self) -> &'static str {
        match self {
            Self::Capture => "快速记录快捷键",
            Self::Main => "主界面快捷键",
        }
    }

    /// 持久化到 `app_settings` 时用的键名。
    pub const fn setting_key(self) -> &'static str {
        match self {
            Self::Capture => "global_shortcut",
            Self::Main => "main_shortcut",
        }
    }
}

/// 当前两个全局快捷键的取值（内存镜像，用于重新注册时注销旧值）。
#[derive(Debug, Clone)]
pub struct Shortcuts {
    pub capture: String,
    pub main: String,
}

impl Shortcuts {
    pub fn with_defaults() -> Self {
        Self {
            capture: DEFAULT_GLOBAL_SHORTCUT.to_string(),
            main: DEFAULT_MAIN_SHORTCUT.to_string(),
        }
    }

    pub fn get(&self, kind: ShortcutKind) -> &str {
        match kind {
            ShortcutKind::Capture => &self.capture,
            ShortcutKind::Main => &self.main,
        }
    }

    pub fn set(&mut self, kind: ShortcutKind, accelerator: String) {
        match kind {
            ShortcutKind::Capture => self.capture = accelerator,
            ShortcutKind::Main => self.main = accelerator,
        }
    }
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// 全局应用状态：连接池在恢复备份时会被整体替换，所以用读写锁包住。
pub struct AppState {
    pub pool: tokio::sync::RwLock<sqlx::SqlitePool>,
    pub paths: Paths,
    /// 当前已注册的两个快捷键字符串，用于重新注册时注销旧值。
    pub shortcuts: StdMutex<Shortcuts>,
}

impl AppState {
    /// 初始化：解析数据目录 → 建连接池（WAL/外键/busy_timeout）→ 迁移 → 默认设置。
    pub async fn init() -> Result<Self, String> {
        let paths = Paths::resolve()?;
        let pool = db::connect(&paths).await?;
        let capture = db::get_setting(&pool, "global_shortcut")
            .await?
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_GLOBAL_SHORTCUT.to_string());
        let main = db::get_setting(&pool, "main_shortcut")
            .await?
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MAIN_SHORTCUT.to_string());
        Ok(Self {
            pool: tokio::sync::RwLock::new(pool),
            paths,
            shortcuts: StdMutex::new(Shortcuts { capture, main }),
        })
    }

    fn shortcut_guard(&self) -> MutexGuard<'_, Shortcuts> {
        self.shortcuts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 读取某个全局快捷键的当前值。
    pub fn shortcut(&self, kind: ShortcutKind) -> String {
        self.shortcut_guard().get(kind).to_string()
    }

    /// 更新内存中的快捷键值（注册成功后调用）。
    pub fn set_shortcut_value(&self, kind: ShortcutKind, accelerator: String) {
        self.shortcut_guard().set(kind, accelerator);
    }

    /// v1 兼容：`global_shortcut` 即 capture 快捷键。
    pub fn current_shortcut(&self) -> String {
        self.shortcut(ShortcutKind::Capture)
    }

    /// v1 兼容：设置 capture 快捷键。
    pub fn set_shortcut(&self, accelerator: String) {
        self.set_shortcut_value(ShortcutKind::Capture, accelerator);
    }
}

/// 把 accelerator 注册为全局快捷键，按下时按 `kind` 执行对应行为。
pub(crate) fn register_shortcut(
    app: &AppHandle,
    kind: ShortcutKind,
    accelerator: &str,
) -> Result<(), String> {
    let shortcut: Shortcut = accelerator.parse().map_err(|_| {
        format!("无法识别的快捷键：{accelerator}（示例：CommandOrControl+Shift+N）")
    })?;
    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                match kind {
                    ShortcutKind::Capture => toggle_quick_window(app),
                    ShortcutKind::Main => show_main_window(app),
                }
            }
        })
        .map_err(|err| format!("快捷键 {accelerator} 注册失败（可能已被其他程序占用）：{err}"))
}

/// 注销快捷键；未注册时忽略错误。
pub(crate) fn unregister_shortcut(app: &AppHandle, accelerator: &str) {
    if let Ok(shortcut) = accelerator.parse::<Shortcut>() {
        let _ = app.global_shortcut().unregister(shortcut);
    }
}

/// 显隐快速记录小窗：可见则隐藏，不可见则显示并聚焦（不存在则按契约创建）。
pub(crate) fn toggle_quick_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(QUICK_WINDOW_LABEL) {
        if window.is_visible().unwrap_or(false) {
            if let Err(err) = window.hide() {
                eprintln!("[WorkNote] 隐藏快速记录小窗失败：{err}");
            }
            return;
        }
    }
    show_quick_window(app);
}

/// 显示并聚焦快速记录小窗；窗口不存在（配置被裁剪、或被前端 close）时按契约重建。
/// 托盘菜单「快速记录」和全局快捷键共用这一条路径。
pub(crate) fn show_quick_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(QUICK_WINDOW_LABEL) {
        if let Err(err) = window.show() {
            eprintln!("[WorkNote] 显示快速记录小窗失败：{err}");
        }
        let _ = window.set_focus();
        return;
    }

    // tauri.conf.json 已声明 quick 窗口；这里只是兜底（例如配置被裁剪时）。
    match WebviewWindowBuilder::new(
        app,
        QUICK_WINDOW_LABEL,
        WebviewUrl::App("index.html?quick=1".into()),
    )
    .title("快速记录")
    .inner_size(560.0, 220.0)
    .resizable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .center()
    .visible(true)
    .build()
    {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(err) => eprintln!("[WorkNote] 创建快速记录小窗失败：{err}"),
    }
}

/// 显示并聚焦主窗口：最小化时先 `unminimize`，再 `show` + `set_focus`。
/// 关闭到托盘后，托盘菜单「打开主界面」、托盘左键单击、main 全局快捷键都从这里唤回。
pub(crate) fn show_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        eprintln!("[WorkNote] 找不到主窗口（label={MAIN_WINDOW_LABEL}），无法显示");
        return;
    };

    if window.is_minimized().unwrap_or(false) {
        if let Err(err) = window.unminimize() {
            eprintln!("[WorkNote] 还原主窗口失败：{err}");
        }
    }
    if let Err(err) = window.show() {
        eprintln!("[WorkNote] 显示主窗口失败：{err}");
    }
    if let Err(err) = window.set_focus() {
        eprintln!("[WorkNote] 聚焦主窗口失败：{err}");
    }
}

/// 通知所有窗口记录已变化（前端监听 `entries-changed` 后刷新当天列表）。
pub(crate) fn emit_entries_changed(app: &AppHandle) {
    if let Err(err) = app.emit(ENTRIES_CHANGED_EVENT, ()) {
        eprintln!("[WorkNote] 广播 {ENTRIES_CHANGED_EVENT} 事件失败：{err}");
    }
}

/// 启动时检查一次自动备份，之后每 60 分钟检查一次（失败只打日志，不阻断应用）。
fn start_auto_backup(app: AppHandle) {
    let _ = tauri::async_runtime::spawn(async move {
        if let Err(err) = commands::backup::auto_backup_tick(&app).await {
            eprintln!("[WorkNote] 启动时自动备份检查失败：{err}");
        }
        loop {
            tokio::time::sleep(Duration::from_secs(60 * 60)).await;
            if let Err(err) = commands::backup::auto_backup_tick(&app).await {
                eprintln!("[WorkNote] 自动备份检查失败：{err}");
            }
        }
    });
}

/// macOS 应用菜单：没有它就没有 ⌘Q / ⌘C / ⌘V / ⌘Z。
/// Windows 上整段被 cfg 掉，不影响现有行为。
#[cfg(target_os = "macos")]
fn setup_macos_menu(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::{AboutMetadata, Menu, PredefinedMenuItem, Submenu};

    let app_menu = Submenu::with_items(
        app,
        "WorkNote",
        true,
        &[
            &PredefinedMenuItem::about(app, Some("关于 WorkNote"), None::<AboutMetadata>)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, Some("隐藏 WorkNote"))?,
            &PredefinedMenuItem::hide_others(app, Some("隐藏其他"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, Some("退出 WorkNote"))?,
        ],
    )?;

    let edit_menu = Submenu::with_items(
        app,
        "编辑",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;

    let window_menu = Submenu::with_items(
        app,
        "窗口",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;

    let menu = Menu::with_items(app, &[&app_menu, &edit_menu, &window_menu])?;
    app.set_menu(menu)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 单实例守卫必须第一个注册：第二次启动改为唤起已有窗口，而不是再开一个进程。
        // 托盘常驻后尤其重要——否则会出现两个托盘图标、两个进程写同一个库。
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // 关闭主窗口 = 隐藏到托盘，进程继续常驻（托盘 / 全局快捷键都能唤回）。
        // 只有托盘没建起来时（图标缺失等降级路径）才退回 v0.1 的「关闭即退出」，
        // 否则窗口被藏起来、又没有托盘入口，用户会找不到应用。
        .on_window_event(|window, event| {
            if window.label() != MAIN_WINDOW_LABEL {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if tray::is_ready() {
                    api.prevent_close();
                    if let Err(err) = window.hide() {
                        eprintln!("[WorkNote] 隐藏主窗口到托盘失败：{err}");
                    }
                } else {
                    eprintln!("[WorkNote] 系统托盘不可用，关闭主窗口将直接退出应用。");
                    window.app_handle().exit(0);
                }
            }
        })
        .setup(|app| {
            // 建目录 / 连库 / 迁移；失败直接中止启动（stderr 有明确原因）。
            let state = tauri::async_runtime::block_on(AppState::init())?;
            let capture = state.shortcut(ShortcutKind::Capture);
            let main = state.shortcut(ShortcutKind::Main);
            app.manage(state);

            // v0.3：两个全局快捷键分别注册，互不影响；启动时冲突只打日志，前端可在设置页改。
            for (kind, accelerator) in [
                (ShortcutKind::Capture, capture),
                (ShortcutKind::Main, main),
            ] {
                if let Err(err) = register_shortcut(app.handle(), kind, &accelerator) {
                    eprintln!("[WorkNote] 启动时注册{}失败：{err}", kind.label());
                }
            }

            // 系统托盘：常驻入口。失败只打日志——托盘没起来不能连带应用起不来。
            if let Err(err) = tray::setup(app.handle()) {
                eprintln!("[WorkNote] 创建系统托盘失败（应用继续运行，关闭主窗口将直接退出）：{err}");
            }

            // macOS 必须有应用菜单，否则 ⌘Q / ⌘C / ⌘V / ⌘Z 这些标准快捷键不可用。
            #[cfg(target_os = "macos")]
            if let Err(err) = setup_macos_menu(app.handle()) {
                eprintln!("[WorkNote] 创建 macOS 应用菜单失败：{err}");
            }

            start_auto_backup(app.handle().clone());
            // 提醒调度：启动立即跑一次，之后每 60 秒一次（软件内通知，不接系统通知）。
            commands::reminders::start_reminder_scheduler(app.handle().clone());

            // 允许 asset 协议读取附件目录（正文里的图片靠它显示）。
            // 注意：不能用配置里的 $APPDATA —— 它指向应用专属目录（%APPDATA%\<identifier>），
            // 而我们的数据目录是 %APPDATA%\WorkNote，所以在运行时按真实路径授权。
            {
                let attachments_dir = app.state::<AppState>().paths.attachments_dir.clone();
                if let Err(err) = app.asset_protocol_scope().allow_directory(&attachments_dir, true) {
                    eprintln!("[WorkNote] 允许图片目录访问失败（图片可能显示不出来）：{err}");
                }
            }

            // 清理「粘贴了图片但记录没保存」留下的孤儿文件（超过 1 天的）。
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                let pool = state.pool.read().await.clone();
                match commands::attachments::sweep_orphans(&pool, &state.paths).await {
                    Ok(0) => {}
                    Ok(n) => println!("[WorkNote] 清理孤儿图片 {n} 张"),
                    Err(err) => eprintln!("[WorkNote] 清理孤儿图片失败：{err}"),
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::entries::list_entries,
            commands::entries::create_entry,
            commands::entries::update_entry,
            commands::entries::set_entry_completed,
            commands::entries::delete_entry,
            commands::entries::restore_entry,
            commands::entries::purge_entry,
            commands::entries::list_trash,
            commands::entries::list_marked_days,
            commands::entries::list_todos_range,
            commands::todos::list_todos,
            commands::todos::get_counts,
            commands::projects::list_projects,
            commands::projects::create_project,
            commands::projects::update_project,
            commands::projects::delete_project,
            commands::projects::restore_project,
            commands::projects::list_project_entries,
            commands::children::list_children,
            commands::children::create_child,
            commands::children::reorder_children,
            commands::search::search_entries,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::mute_reminders_today,
            commands::notifications::notifications_list,
            commands::notifications::notifications_unread,
            commands::notifications::notification_mark_read,
            commands::notifications::notification_dismiss,
            commands::notifications::notification_snooze,
            commands::backup::backup_now,
            commands::backup::list_backups,
            commands::backup::restore_backup,
            commands::backup::export_json,
            commands::backup::export_csv,
            commands::system::open_data_dir,
            commands::system::set_global_shortcut,
            commands::system::set_shortcut,
            commands::attachments::save_attachment,
            commands::attachments::attach_file,
            commands::attachments::list_attachments,
            commands::attachments::claim_attachments,
            commands::attachments::delete_attachment,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
