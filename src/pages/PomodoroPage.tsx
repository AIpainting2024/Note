import { Coffee, Pause, Play, RotateCcw, Timer, Trash2 } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "../components/ui/Button";
import { Modal } from "../components/ui/Modal";
import { PomodoroDoneOverlay } from "../components/PomodoroDoneOverlay";
import { api, errorText } from "../lib/api";
import { cn } from "../lib/cn";
import type { PomodoroSession, PomodoroSummary } from "../lib/types";
import { useSettingsStore } from "../stores/settings";
import { useUI } from "../stores/ui";

const ALERT_LABELS: Record<string, string> = {
  dialog: "弹窗",
  animation: "动画",
  badge: "红点",
};

const MODES = [
  { kind: "focus", label: "专注", minutes: 25 },
  { kind: "break", label: "短休息", minutes: 5 },
  { kind: "break", label: "长休息", minutes: 15 },
] as const;

function fmt(seconds: number): string {
  const m = Math.floor(Math.max(0, seconds) / 60);
  const s = Math.max(0, seconds) % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

/** 番茄钟：计时在前端，完成后写入记录；右侧展示总览与最近记录 */
export function PomodoroPage() {
  const revision = useUI((s) => s.revision);
  const [modeIndex, setModeIndex] = useState(0);
  const [remaining, setRemaining] = useState(MODES[0].minutes * 60);
  const [running, setRunning] = useState(false);
  const [summary, setSummary] = useState<PomodoroSummary | null>(null);
  const [records, setRecords] = useState<PomodoroSession[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [lastMsg, setLastMsg] = useState<string | null>(null);
  /** 结束动画 token：每次 +1 重放（v0.12） */
  const [celebrate, setCelebrate] = useState(0);
  /** 弹窗提示（v0.12） */
  const [alertOpen, setAlertOpen] = useState(false);
  const [alertText, setAlertText] = useState("");
  const alertMode = useSettingsStore((s) => s.settings?.pomodoroAlert ?? "dialog");

  const startedAt = useRef<string | null>(null);
  const mode = MODES[modeIndex];
  const total = mode.minutes * 60;

  const loadStats = useCallback(async () => {
    try {
      const [s, list] = await Promise.all([api.pomodoroSummary(), api.pomodoroList(30)]);
      setSummary(s);
      setRecords(list);
    } catch (err) {
      setError(errorText(err));
    }
  }, []);

  useEffect(() => {
    void loadStats();
  }, [loadStats, revision]);

  // v0.12：进页面即清掉侧边栏红点
  useEffect(() => {
    useUI.getState().setPomodoroDone(false);
  }, []);

  // 计时
  useEffect(() => {
    if (!running) return;
    const timer = window.setInterval(() => {
      setRemaining((prev) => {
        if (prev <= 1) {
          window.clearInterval(timer);
          setRunning(false);
          void finish(mode.kind, total, true);
          return 0;
        }
        return prev - 1;
      });
    }, 1000);
    return () => window.clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [running, mode.kind, total]);

  const finish = async (kind: string, seconds: number, completed: boolean) => {
    const end = new Date();
    const begin = startedAt.current ?? new Date(end.getTime() - seconds * 1000).toISOString();
    startedAt.current = null;
    try {
      await api.pomodoroSave({
        kind,
        durationSeconds: seconds,
        startedAt: begin,
        endedAt: end.toISOString(),
        completed,
        entryId: null,
      });
      setLastMsg(
        completed
          ? `${kind === "focus" ? "完成一个番茄" : "休息结束"}：${Math.round(seconds / 60)} 分钟`
          : `已结束并记录：${Math.round(seconds / 60)} 分钟`,
      );
      // v0.12：结束提示三选一 —— 弹窗 / 动画 / 红点
      if (completed) {
        const mode = useSettingsStore.getState().settings?.pomodoroAlert ?? "dialog";
        const tip = kind === "focus" ? "专注完成，休息一下" : "休息结束，继续专注吧";
        if (mode === "dialog") {
          setAlertText(tip);
          setAlertOpen(true);
        } else if (mode === "animation") {
          setCelebrate((n) => n + 1);
        } else {
          useUI.getState().setPomodoroDone(true);
        }
      }
      useUI.getState().bumpRevision();
      await loadStats();
    } catch (err) {
      setError(errorText(err));
    }
  };

  const start = () => {
    if (!startedAt.current) {
      startedAt.current = new Date(
        Date.now() - (total - remaining) * 1000,
      ).toISOString();
    }
    setRunning(true);
    setLastMsg(null);
  };

  /** 提前结束：把已经过的时长记下来（统计真实投入） */
  const stop = async () => {
    setRunning(false);
    const spent = total - remaining;
    if (spent >= 60) {
      await finish(mode.kind, spent, false);
    } else {
      startedAt.current = null;
      setLastMsg("不足 1 分钟，未记录");
    }
    setRemaining(total);
  };

  const reset = () => {
    setRunning(false);
    startedAt.current = null;
    setRemaining(total);
    setLastMsg(null);
  };

  const pickMode = (index: number) => {
    setModeIndex(index);
    setRunning(false);
    startedAt.current = null;
    setRemaining(MODES[index].minutes * 60);
    setLastMsg(null);
  };

  const clearAll = async () => {
    if (!window.confirm("清空所有番茄记录？统计会归零，此操作不可撤销。")) return;
    try {
      await api.pomodoroClear();
      useUI.getState().bumpRevision();
      await loadStats();
      setLastMsg("已清空番茄记录");
    } catch (err) {
      setError(errorText(err));
    }
  };

  const progress = total > 0 ? 1 - remaining / total : 0;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {/* 结束提示：整屏番茄切开动画（设置里选「动画」时） */}
      <PomodoroDoneOverlay token={celebrate} text={alertText || "番茄完成，休息一下"} />

      {/* 结束提示：弹窗（设置里选「弹窗」时） */}
      <Modal
        open={alertOpen}
        onOpenChange={setAlertOpen}
        title="番茄钟"
        className="w-[340px]"
      >
        <div className="p-5 text-center">
          <p className="text-[15px] font-semibold text-ink">{alertText}</p>
          <p className="mt-1 text-[12px] text-ink-faint">
            {mode.kind === "focus" ? `已记录 ${Math.round(total / 60)} 分钟专注` : "休息结束了"}
          </p>
          <div className="mt-4 flex justify-center gap-2">
            <Button variant="ghost" size="sm" onClick={() => setAlertOpen(false)}>
              知道了
            </Button>
            <Button
              variant="primary"
              size="sm"
              onClick={() => {
                setAlertOpen(false);
                pickMode(mode.kind === "focus" ? 1 : 0);
                start();
              }}
            >
              {mode.kind === "focus" ? "开始休息" : "开始专注"}
            </Button>
          </div>
        </div>
      </Modal>
      <header className="flex items-center gap-2 border-b border-line px-6 py-3">
        <Timer size={16} className="text-accent" />
        <h1 className="text-[15px] font-semibold text-ink">番茄钟</h1>
        <span className="text-[11.5px] text-ink-faint">专注 {MODES[0].minutes} 分钟，休息 {MODES[1].minutes} 分钟</span>
        <span className="ml-auto rounded-full bg-panel-2 px-2 py-0.5 text-[11px] text-ink-faint">
          结束提示：{ALERT_LABELS[alertMode] ?? "弹窗"}
        </span>
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-[1fr_320px] gap-4 overflow-y-auto p-5">
        {/* 计时区 */}
        <section className="flex flex-col items-center justify-center rounded-xl border border-line bg-panel p-6">
          <div className="mb-4 flex gap-1.5">
            {MODES.map((m, i) => (
              <button
                key={`${m.kind}-${m.minutes}`}
                type="button"
                onClick={() => pickMode(i)}
                className={cn(
                  "h-7 rounded-md border px-2.5 text-[12px]",
                  i === modeIndex
                    ? "border-accent bg-accent-soft font-medium text-accent"
                    : "border-line text-ink-muted hover:text-ink",
                )}
              >
                {m.label} {m.minutes}′
              </button>
            ))}
          </div>

          <div className="relative mb-5 flex h-[210px] w-[210px] items-center justify-center">
            <svg viewBox="0 0 100 100" className="absolute h-full w-full -rotate-90">
              <circle cx="50" cy="50" r="45" fill="none" stroke="currentColor" strokeWidth="6" className="text-line" />
              <circle
                cx="50"
                cy="50"
                r="45"
                fill="none"
                stroke="currentColor"
                strokeWidth="6"
                strokeLinecap="round"
                strokeDasharray={`${progress * 283} 283`}
                className={mode.kind === "focus" ? "text-accent" : "text-warn"}
              />
            </svg>
            <div className="text-center">
              <p className="font-mono text-[38px] leading-none text-ink">{fmt(remaining)}</p>
              <p className="mt-1 text-[11.5px] text-ink-faint">
                {mode.kind === "focus" ? "专注中" : "休息中"}
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2">
            {running ? (
              <Button variant="primary" size="md" onClick={() => void stop()}>
                <Pause size={14} /> 提前结束
              </Button>
            ) : (
              <Button variant="primary" size="md" onClick={start}>
                <Play size={14} /> 开始
              </Button>
            )}
            <Button variant="ghost" size="md" onClick={reset}>
              <RotateCcw size={14} /> 重置
            </Button>
          </div>

          {lastMsg && <p className="mt-3 text-[12px] text-accent">{lastMsg}</p>}
          {error && <p className="mt-2 text-[12px] text-danger">{error}</p>}
        </section>

        {/* 统计 + 记录 */}
        <section className="flex min-h-0 flex-col gap-3">
          <div className="rounded-xl border border-line bg-panel p-3">
            <h2 className="mb-2 text-[12.5px] font-semibold text-ink">总览</h2>
            <div className="grid grid-cols-2 gap-2 text-center">
              <Stat label="今日" value={`${summary?.todayCount ?? 0} 个`} sub={`${summary?.todayMinutes ?? 0} 分钟`} />
              <Stat label="近 7 天" value={`${summary?.weekCount ?? 0} 个`} sub={`${summary?.weekMinutes ?? 0} 分钟`} />
              <Stat label="累计" value={`${summary?.totalCount ?? 0} 个`} sub={`${summary?.totalMinutes ?? 0} 分钟`} />
              <Stat label="连续" value={`${summary?.streakDays ?? 0} 天`} sub="有专注记录" />
            </div>
          </div>

          <div className="flex min-h-0 flex-1 flex-col rounded-xl border border-line bg-panel p-3">
            <div className="mb-2 flex items-center">
              <h2 className="flex-1 text-[12.5px] font-semibold text-ink">最近记录</h2>
              <button
                type="button"
                onClick={() => void clearAll()}
                className="flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] text-ink-faint hover:bg-danger/10 hover:text-danger"
              >
                <Trash2 size={11} /> 清空
              </button>
            </div>
            <div className="min-h-0 flex-1 space-y-1 overflow-y-auto">
              {records.length === 0 && (
                <p className="py-3 text-center text-[11.5px] text-ink-faint">还没有记录</p>
              )}
              {records.map((record) => (
                <div
                  key={record.id}
                  className="flex items-center gap-2 rounded-md px-1.5 py-1 text-[11.5px] hover:bg-panel-2"
                >
                  {record.kind === "focus" ? (
                    <Timer size={11} className="shrink-0 text-accent" />
                  ) : (
                    <Coffee size={11} className="shrink-0 text-warn" />
                  )}
                  <span className="text-ink-muted">{record.day}</span>
                  <span className="text-ink">{Math.round(record.durationSeconds / 60)} 分钟</span>
                  {!record.completed && <span className="text-ink-faint">(提前结束)</span>}
                  {record.entryContent && (
                    <span className="min-w-0 flex-1 truncate text-ink-faint">{record.entryContent}</span>
                  )}
                </div>
              ))}
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}

function Stat({ label, value, sub }: { label: string; value: string; sub: string }) {
  return (
    <div className="rounded-lg bg-panel-2 px-2 py-1.5">
      <p className="text-[10.5px] text-ink-faint">{label}</p>
      <p className="text-[14px] font-semibold text-ink">{value}</p>
      <p className="text-[10px] text-ink-faint">{sub}</p>
    </div>
  );
}
