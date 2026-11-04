# WorkNote

个人工作记录工具：随手记一笔，按天回看，全文搜回来，数据只存在本地。支持 Windows 与 macOS（macOS 需在 macOS 上构建）。

## 已实现（截至 v0.5.0）

- **今日**：常驻输入框，`Ctrl+Enter` 保存；记录按时间倒序，双击就地编辑，停止输入 600ms 自动保存；悬停可置顶 / 转待办 / 删除。
- **项目与层级**：左侧项目列表（色点 + 完成进度），**点箭头可展开/收起该项目下的任务**（就地勾选完成、点开详情、显示截止与子任务进度），表头支持全部展开/收起；项目页含任务/记录双页签；任务可拆子任务（勾选、改名、拖动排序），卡片显示 `3/6` 进度；快速输入末尾加 `#项目名` 直接归属。
- **日历**：月 / 周 / 日三视图；周/日视图按「全天 + 上午/中午/下午/晚上」铺开待办，拖到别的日期即改期；回收站可恢复软删除的记录。
- **待办**：未完成 / 今日到期 / 已逾期 / 已完成；可设截止日期与具体时间；记录与待办是同一份数据。
- **提醒（软件内）**：截止前 N 分钟提醒（仅对设了具体时间的任务）+ 每日汇总（昨日逾期 + 今日到期）；通知中心支持完成 / 稍后 30 分钟 / 忽略 / 今天别再提醒。不弹 Windows 系统通知。
- **搜索**：`Ctrl+K` 弹层（可在设置改），中文全文搜索，结果显示所属项目，子任务会标注「父任务」。
- **快速记录小窗**：任意程序里按 `Ctrl+Shift+N` 弹窗记一笔，`Ctrl+Enter` 保存并收起。
- **系统托盘**：关闭主窗口 = 收进托盘，进程常驻；左键单击图标切换主窗口；右键菜单：打开主界面 / 快速记录 / 退出。
- **快捷键自定义**：全局快速记录、显示主界面、打开搜索三组均可改，冲突时当场报错并保留原组合。
- **单实例**：重复启动只会唤起已有窗口，不会出现两个托盘图标或两个进程写同一个库。
- **数据安全**：本地 SQLite（WAL）、`VACUUM INTO` 一致性备份、每日自动备份、恢复、JSON/CSV 导出。

明确不做（后续版本）：标签、看板视图、复盘报表、批量操作、周期任务、AI、云同步。

## 平台

| 平台 | 状态 | 说明 |
|---|---|---|
| Windows 10 / 11 | ✅ 已验证 | 安装包 `WorkNote_0.5.0_x64-setup.exe` |
| macOS 10.15+ | ⚠️ 代码已适配，需在 macOS 上构建 | 见下方「构建 macOS 安装包」 |

Tauri 的 macOS 包（`.app` / `.dmg`）只能在 macOS 上构建（依赖 Apple SDK、`codesign`、`hdiutil`），Windows 不能交叉编译。

### 构建 macOS 安装包

**方式 A：GitHub Actions**（工作流已内置 `.github/workflows/build-macos.yml`）
推送代码后，在 Actions 页面手动运行 `build-macos`，或打 `v*` 标签触发；跑完下载 artifact 里的 dmg（arm64 / x64 各一个）。

**方式 B：本机 Mac**

```bash
pnpm install
pnpm tauri build            # 产出 .app + .dmg
# 仅 dmg：pnpm tauri build --target aarch64-apple-darwin --bundles dmg
```

产物在 `src-tauri/target/release/bundle/dmg/`。未签名产物首次打开需「右键 → 打开」，或
`xattr -dr com.apple.quarantine /Applications/WorkNote.app`。

macOS 数据目录：`~/Library/Application Support/WorkNote/`（Windows 为 `%APPDATA%\WorkNote`）。

## 环境要求

| 项 | 版本 |
|---|---|
| Windows | 10 / 11（本机 Windows 10 19044 验证） |
| Node.js | ≥ 20（本机 22.23.2） |
| pnpm | ≥ 9（本机 11.25.0） |
| Rust | stable（本机 1.99.1），需 MSVC C++ 工具链 |
| WebView2 | 系统自带运行时 |

Rust 若未安装：从 <https://rustup.rs> 下载 `rustup-init.exe`，运行 `rustup-init.exe -y --default-toolchain stable --profile minimal`。

## 开发

```powershell
pnpm install          # 安装前端依赖
pnpm tauri dev        # 启动桌面应用（含热更新）
```

只跑前端（没有 Tauri 后端，数据命令会失败，仅用于看样式）：

```powershell
pnpm dev              # http://localhost:1420
```

## 构建安装包

```powershell
pnpm tauri build
```

产物：

- 安装包 `src-tauri/target/release/bundle/nsis/WorkNote_0.1.0_x64-setup.exe`
- 免安装程序 `src-tauri/target/release/worknote.exe`

## 数据位置

```text
%APPDATA%\WorkNote\
    database\worknote.db      # SQLite 主库（WAL）
    backups\                  # 备份文件 worknote_YYYY-MM-DD_HHMMSS.db
    exports\                  # 手动导出的 JSON / CSV
    logs\
```

数据不在安装目录，升级、重装、卸载默认不碰用户数据。设置页可「打开数据文件夹」。

## 快捷键

| 键 | 行为 |
|---|---|
| `Ctrl+Shift+N` | 全局：任意程序内唤起快速记录小窗（设置里可改） |
| `Ctrl+Shift+M` | 全局：显示主窗口（托盘隐藏时也叫得回来，设置里可改） |
| `Ctrl+K` | 打开搜索（设置里可改） |
| `Ctrl+N` | 聚焦快速输入框 |
| `Ctrl+Enter` | 保存当前编辑 / 小窗保存并收起 |
| `Esc` | 关闭弹层 / 退出编辑 / 收起小窗 |

输入法组字过程中不会触发保存或快捷键。三组可改快捷键在「设置 → 快捷键」，被其他程序占用时会当场报错并保留原组合。

**退出应用**：关闭主窗口只会收进托盘；要完全退出，请右键托盘图标 → 退出。

## 常用命令

```powershell
pnpm build                 # 前端类型检查 + 构建
cd src-tauri; cargo check  # Rust 编译检查
```

## 验证与测试

```powershell
cd src-tauri; cargo test   # 后端 72 项测试
```
