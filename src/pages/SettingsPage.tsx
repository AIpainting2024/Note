import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  BellRing,
  Database,
  Download,
  FolderOpen,
  HardDriveDownload,
  Keyboard,
  Monitor,
  Moon,
  RotateCcw,
  Sun,
  VolumeX,
} from "lucide-react";
import { useEffect, useState } from "react";
import { PageHeader } from "../components/PageHeader";
import { Button } from "../components/ui/Button";
import { Input } from "../components/ui/Input";
import { api, errorText } from "../lib/api";
import { cn } from "../lib/cn";
import { todayStr } from "../lib/dates";
import { displayShortcut, validateShortcut } from "../lib/shortcuts";
import type { BackupInfo, Theme } from "../lib/types";
import { useNotifications } from "../stores/notifications";
import { useSettingsStore } from "../stores/settings";
import { useUI } from "../stores/ui";

type Notice = { tone: "ok" | "error"; text: string } | null;

const THEMES: { key: Theme; label: string; icon: typeof Sun }[] = [
  { key: "light", label: "浅色", icon: Sun },
  { key: "dark", label: "深色", icon: Moon },
  { key: "system", label: "跟随系统", icon: Monitor },
];

const REMIND_OPTIONS = [
  { value: 0, label: "关闭（不提醒）" },
  { value: 15, label: "提前 15 分钟" },
  { value: 30, label: "提前 30 分钟" },
  { value: 60, label: "提前 1 小时" },
  { value: 1440, label: "提前 1 天" },
];

/** 三组可改快捷键：两种是全局（后端注册），一种是应用内 */
const SHORTCUT_ROWS: {
  key: "capture" | "main" | "search";
  label: string;
  hint: string;
}[] = [
  { key: "capture", label: "全局快速记录", hint: "任意程序里唤起记录小窗" },
  { key: "main", label: "显示主界面", hint: "托盘隐藏时把主窗口叫回来" },
  { key: "search", label: "打开搜索", hint: "应用内的搜索弹层" },
];

export function SettingsPage() {
  const settings = useSettingsStore((s) => s.settings);
  const save = useSettingsStore((s) => s.save);
  const load = useSettingsStore((s) => s.load);
  const theme = useUI((s) => s.theme);
  const setTheme = useUI((s) => s.setTheme);

  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [rowErrors, setRowErrors] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<Notice>(null);

  useEffect(() => {
    void load();
    api
      .listBackups()
      .then(setBackups)
      .catch(() => undefined);
  }, [load]);

  useEffect(() => {
    if (!settings) return;
    setDrafts({
      capture: displayShortcut(settings.globalShortcut),
      main: displayShortcut(settings.mainShortcut),
      search: displayShortcut(settings.searchShortcut),
    });
  }, [settings]);

  const flash = (tone: "ok" | "error", text: string) => {
    setNotice({ tone, text });
    window.setTimeout(() => setNotice(null), 4000);
  };

  const backupNow = async () => {
    setBusy(true);
    try {
      const info = await api.backupNow();
      setBackups(await api.listBackups());
      await load();
      flash("ok", `已备份：${basename(info.path)}（${formatSize(info.sizeBytes)}）`);
    } catch (err) {
      flash("error", errorText(err));
    } finally {
      setBusy(false);
    }
  };

  const restore = async () => {
    try {
      const picked = await openDialog({
        multiple: false,
        title: "选择要恢复的备份文件",
        filters: [{ name: "SQLite 备份", extensions: ["db"] }],
      });
      if (!picked || Array.isArray(picked)) return;
      if (!window.confirm("恢复会用备份文件覆盖当前数据（覆盖前会先自动备份当前库）。继续吗？")) return;

      setBusy(true);
      await api.restoreBackup(picked);
      useUI.getState().bumpRevision();
      await load();
      flash("ok", "恢复完成，数据已回到备份时间点");
    } catch (err) {
      flash("error", errorText(err));
    } finally {
      setBusy(false);
    }
  };

  const exportAs = async (kind: "json" | "csv") => {
    try {
      const path = await saveDialog({
        title: kind === "json" ? "导出全部数据" : "导出任务 CSV",
        defaultPath: `worknote-${todayStr()}.${kind}`,
        filters: [
          kind === "json"
            ? { name: "JSON", extensions: ["json"] }
            : { name: "CSV", extensions: ["csv"] },
        ],
      });
      if (!path) return;
      setBusy(true);
      if (kind === "json") await api.exportJson(path);
      else await api.exportCsv(path);
      flash("ok", `已导出到 ${path}`);
    } catch (err) {
      flash("error", errorText(err));
    } finally {
      setBusy(false);
    }
  };

  const applyShortcut = async (kind: "capture" | "main" | "search") => {
    const draft = (drafts[kind] ?? "").trim();
    const invalid = validateShortcut(draft);
    if (invalid) {
      setRowErrors((e) => ({ ...e, [kind]: invalid }));
      return;
    }
    setRowErrors((e) => ({ ...e, [kind]: "" }));
    const accelerator = draft.replace(/\bCtrl\b/gi, "CommandOrControl").replace(/\s+/g, "");
    try {
      setBusy(true);
      if (kind === "search") {
        await save({ searchShortcut: accelerator });
        flash("ok", `搜索快捷键已更新为 ${displayShortcut(accelerator)}`);
      } else {
        const next = await api.setShortcut(kind, accelerator);
        useSettingsStore.setState({ settings: next });
        flash("ok", `快捷键已更新为 ${displayShortcut(accelerator)}`);
      }
    } catch (err) {
      setRowErrors((e) => ({ ...e, [kind]: errorText(err) }));
    } finally {
      setBusy(false);
    }
  };

  if (!settings) {
    return (
      <div className="flex min-h-0 flex-1 flex-col">
        <PageHeader title="设置" showPanelToggle={false} />
        <p className="px-6 text-[13px] text-ink-faint">读取设置中…</p>
      </div>
    );
  }

  const muted = Boolean(settings.mutedUntil && settings.mutedUntil > `${todayStr()}T00:00:00`);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <PageHeader
        title="设置"
        subtitle="提醒、快捷键、数据与外观"
        showPanelToggle={false}
        actions={
          notice ? (
            <span
              className={cn(
                "max-w-[360px] truncate rounded-md px-2 py-1 text-[11.5px]",
                notice.tone === "ok" ? "bg-accent-soft text-accent" : "bg-danger/10 text-danger",
              )}
            >
              {notice.text}
            </span>
          ) : null
        }
      />

      <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-10">
        <div className="max-w-[760px] space-y-3">
          <Section title="提醒" icon={<BellRing size={14} />}>
            <p className="mb-3 text-[12px] text-ink-muted">
              提醒只在软件内提示（右下角提醒条 + 侧边栏「通知」），不弹 Windows 系统通知。
            </p>
            <div className="flex flex-wrap items-center gap-3">
              <label className="flex items-center gap-2 text-[12.5px] text-ink-muted">
                截止前
                <select
                  value={settings.remindBeforeMinutes}
                  onChange={(e) => void save({ remindBeforeMinutes: Number(e.target.value) })}
                  className="h-8 rounded-md border border-line bg-panel px-2 text-[12.5px] text-ink focus:border-accent focus:outline-none"
                >
                  {REMIND_OPTIONS.map((o) => (
                    <option key={o.value} value={o.value}>
                      {o.label}
                    </option>
                  ))}
                </select>
              </label>
              <span className="text-[11.5px] text-ink-faint">只对设了具体时间的待办生效</span>
            </div>

            <div className="mt-3 flex flex-wrap items-center gap-3 text-[12.5px] text-ink-muted">
              <label className="flex items-center gap-1.5">
                <input
                  type="checkbox"
                  checked={settings.dailyDigestEnabled}
                  onChange={(e) => void save({ dailyDigestEnabled: e.target.checked })}
                />
                每日汇总
              </label>
              <Input
                type="time"
                value={settings.dailyDigestTime}
                disabled={!settings.dailyDigestEnabled}
                onChange={(e) => {
                  const v = e.target.value;
                  if (/^\d{2}:\d{2}$/.test(v)) void save({ dailyDigestTime: v });
                }}
                className="h-7 w-[92px]"
              />
              <span className="text-[11.5px] text-ink-faint">汇总昨日逾期 + 今日到期</span>
            </div>

            <div className="mt-3 flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                disabled={busy}
                onClick={async () => {
                  try {
                    await useNotifications.getState().muteToday();
                    await load();
                    flash("ok", "已静音到明天");
                  } catch (err) {
                    flash("error", errorText(err));
                  }
                }}
              >
                <VolumeX size={13} /> 今天别再提醒
              </Button>
              <span className="text-[11.5px] text-ink-faint">
                {muted ? `已静音至 ${settings.mutedUntil?.replace("T", " ")}` : "当前未静音"}
              </span>
            </div>
          </Section>

          <Section title="快捷键" icon={<Keyboard size={14} />}>
            <div className="space-y-2">
              {SHORTCUT_ROWS.map((row) => (
                <div key={row.key}>
                  <div className="flex flex-wrap items-center gap-2">
                    <div className="w-[168px] text-[12.5px] text-ink">{row.label}</div>
                    <Input
                      value={drafts[row.key] ?? ""}
                      onChange={(e) => setDrafts((d) => ({ ...d, [row.key]: e.target.value }))}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") void applyShortcut(row.key);
                      }}
                      className="h-7 w-[200px] font-mono"
                      spellCheck={false}
                    />
                    <Button
                      variant="outline"
                      size="sm"
                      disabled={busy}
                      onClick={() => void applyShortcut(row.key)}
                    >
                      应用
                    </Button>
                    <span className="text-[11px] text-ink-faint">{row.hint}</span>
                  </div>
                  {rowErrors[row.key] && (
                    <p className="mt-1 pl-[176px] text-[11.5px] text-danger">{rowErrors[row.key]}</p>
                  )}
                </div>
              ))}
            </div>
            <p className="mt-2 text-[11.5px] text-ink-faint">
              全局快捷键被其他程序占用时会当场报错并保留原组合；搜索快捷键为应用内生效。
            </p>
          </Section>

          <Section title="外观" icon={<Sun size={14} />}>
            <div className="flex gap-1.5">
              {THEMES.map(({ key, label, icon: Icon }) => (
                <button
                  key={key}
                  type="button"
                  onClick={() => {
                    setTheme(key);
                    void save({ theme: key });
                  }}
                  className={cn(
                    "flex h-8 items-center gap-1.5 rounded-md border px-2.5 text-[12.5px]",
                    theme === key
                      ? "border-accent bg-accent-soft font-medium text-accent"
                      : "border-line text-ink-muted hover:text-ink",
                  )}
                >
                  <Icon size={13} />
                  {label}
                </button>
              ))}
            </div>
          </Section>

          <Section title="数据位置" icon={<Database size={14} />}>
            <p className="mb-2 break-all font-mono text-[12px] text-ink-muted">{settings.dataDir}</p>
            <Button variant="outline" size="sm" onClick={() => void api.openDataDir()}>
              <FolderOpen size={13} /> 打开数据文件夹
            </Button>
          </Section>

          <Section title="备份与恢复" icon={<HardDriveDownload size={14} />}>
            <div className="flex flex-wrap items-center gap-2">
              <Button variant="primary" size="sm" disabled={busy} onClick={() => void backupNow()}>
                <HardDriveDownload size={13} /> 立即备份
              </Button>
              <Button variant="outline" size="sm" disabled={busy} onClick={() => void restore()}>
                <RotateCcw size={13} /> 从备份恢复
              </Button>
              <span className="text-[11.5px] text-ink-faint">
                最近备份：{settings.lastBackupAt ?? "尚未备份"}
              </span>
            </div>

            <div className="mt-3 flex flex-wrap items-center gap-3 text-[12.5px] text-ink-muted">
              <label className="flex items-center gap-1.5">
                <input
                  type="checkbox"
                  checked={settings.autoBackup}
                  onChange={(e) => void save({ autoBackup: e.target.checked })}
                />
                每日自动备份
              </label>
              <label className="flex items-center gap-1.5">
                保留
                <Input
                  type="number"
                  min={1}
                  max={60}
                  value={settings.autoBackupKeep}
                  onChange={(e) => void save({ autoBackupKeep: Number(e.target.value) || 14 })}
                  className="h-7 w-[64px] text-center"
                />
                份
              </label>
            </div>

            <div className="mt-3 divide-y divide-line rounded-lg border border-line">
              {backups.length === 0 ? (
                <p className="px-3 py-3 text-[12px] text-ink-faint">还没有备份文件</p>
              ) : (
                backups.slice(0, 8).map((b) => (
                  <div key={b.path} className="flex items-center gap-2 px-3 py-2">
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-[12.5px] text-ink">{basename(b.path)}</p>
                      <p className="text-[11px] text-ink-faint">
                        {b.createdAt} · {formatSize(b.sizeBytes)}
                      </p>
                    </div>
                  </div>
                ))
              )}
            </div>
          </Section>

          <Section title="导出" icon={<Download size={14} />}>
            <div className="flex gap-2">
              <Button variant="outline" size="sm" disabled={busy} onClick={() => void exportAs("json")}>
                导出全部 JSON
              </Button>
              <Button variant="outline" size="sm" disabled={busy} onClick={() => void exportAs("csv")}>
                导出 CSV
              </Button>
            </div>
          </Section>

          <Section title="关于" icon={<Monitor size={14} />}>
            <p className="text-[12.5px] text-ink-muted">WorkNote 0.7.1 · 本地 SQLite 存储，不联网。</p>
          </Section>
        </div>
      </div>
    </div>
  );
}

function Section({
  title,
  icon,
  children,
}: {
  title: string;
  icon: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <section className="rounded-xl border border-line bg-panel p-4">
      <h2 className="mb-2.5 flex items-center gap-1.5 text-[13px] font-semibold text-ink">
        <span className="text-ink-faint">{icon}</span>
        {title}
      </h2>
      {children}
    </section>
  );
}

function basename(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

function formatSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
