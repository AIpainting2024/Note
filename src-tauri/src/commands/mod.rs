//! Tauri 命令集合，按领域拆分模块。
//!
//! v0.4 新增 `projects`（项目）与 `children`（任务层级）两个模块。

pub mod attachments;
pub mod backup;
pub mod children;
pub mod entries;
pub mod notifications;
pub mod projects;
pub mod reminders;
pub mod search;
pub mod settings;
pub mod system;
pub mod todos;
