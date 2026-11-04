import { Check, Clock, X } from "lucide-react";
import { useEffect } from "react";
import type { Notification } from "../lib/types";
import { useNotifications } from "../stores/notifications";
import { Button } from "./ui/Button";

const AUTO_CLOSE_MS = 15000;

/** 软件内提醒条：新通知到达时在右下角弹出，不依赖 Windows 系统通知 */
export function NotificationToasts() {
  const toasts = useNotifications((s) => s.toasts);
  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-30 flex w-[340px] flex-col gap-2">
      {toasts.map((n) => (
        <Toast key={n.id} n={n} />
      ))}
    </div>
  );
}

function Toast({ n }: { n: Notification }) {
  const dropToast = useNotifications((s) => s.dropToast);
  const complete = useNotifications((s) => s.complete);
  const snooze = useNotifications((s) => s.snooze);
  const dismiss = useNotifications((s) => s.dismiss);

  useEffect(() => {
    const id = window.setTimeout(() => dropToast(n.id), AUTO_CLOSE_MS);
    return () => window.clearTimeout(id);
  }, [n.id, dropToast]);

  return (
    <div className="pointer-events-auto rounded-xl border border-line bg-panel p-3 shadow-2xl">
      <div className="flex items-start gap-2">
        <div className="min-w-0 flex-1">
          <p className="flex items-center gap-1.5 text-[11px] text-ink-faint">
            <Clock size={10} /> WorkNote 提醒
          </p>
          <p className="mt-1 text-[13px] font-medium leading-snug text-ink">{n.title}</p>
          <p className="mt-0.5 text-[11.5px] leading-relaxed text-ink-muted">{n.body}</p>
        </div>
        <button
          type="button"
          aria-label="关闭提醒"
          onClick={() => dropToast(n.id)}
          className="rounded p-1 text-ink-faint hover:bg-panel-2 hover:text-ink"
        >
          <X size={13} />
        </button>
      </div>

      <div className="mt-2 flex items-center justify-end gap-1.5">
        {n.entryId != null && (
          <>
            <Button variant="primary" size="sm" onClick={() => void complete(n)}>
              <Check size={12} /> 已完成
            </Button>
            <Button variant="outline" size="sm" onClick={() => void snooze(n.id, 30)}>
              稍后 30 分钟
            </Button>
          </>
        )}
        <Button variant="ghost" size="sm" onClick={() => void dismiss(n.id)}>
          忽略
        </Button>
      </div>
    </div>
  );
}
