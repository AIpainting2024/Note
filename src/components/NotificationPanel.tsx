import { Bell, Check, Clock, X } from "lucide-react";
import { useEffect } from "react";
import { cn } from "../lib/cn";
import { timeOf } from "../lib/dates";
import type { Notification } from "../lib/types";
import { useNotifications } from "../stores/notifications";
import { Button } from "./ui/Button";
import { Modal } from "./ui/Modal";

export function NotificationPanel() {
  const open = useNotifications((s) => s.panelOpen);
  const items = useNotifications((s) => s.items);
  const error = useNotifications((s) => s.error);
  const setOpen = useNotifications((s) => s.setPanelOpen);
  const complete = useNotifications((s) => s.complete);
  const snooze = useNotifications((s) => s.snooze);
  const dismiss = useNotifications((s) => s.dismiss);
  const muteToday = useNotifications((s) => s.muteToday);
  const markRead = useNotifications((s) => s.markRead);

  // 打开通知中心即把未读标记为已读
  useEffect(() => {
    if (!open) return;
    items.filter((n) => !n.readAt).forEach((n) => void markRead(n.id));
  }, [open, items, markRead]);

  return (
    <Modal open={open} onOpenChange={setOpen} title="通知中心" className="w-[560px]">
      <div className="flex items-center justify-between border-b border-line px-4 py-2.5">
        <div className="flex items-center gap-2 text-[13px] font-semibold text-ink">
          <Bell size={14} className="text-ink-faint" />
          通知
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => void muteToday()}
            className="text-[11.5px] text-ink-faint hover:text-accent"
          >
            今天别再提醒
          </button>
          <button
            type="button"
            onClick={() => setOpen(false)}
            className="rounded p-1 text-ink-faint hover:bg-panel-2 hover:text-ink"
            aria-label="关闭"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      <div className="max-h-[52vh] overflow-y-auto p-2">
        {error && <p className="px-2 py-2 text-[12px] text-danger">{error}</p>}

        {items.length === 0 ? (
          <p className="px-2 py-10 text-center text-[12.5px] text-ink-faint">没有需要处理的提醒</p>
        ) : (
          items.map((n) => (
            <NotificationRow
              key={n.id}
              n={n}
              onComplete={() => void complete(n)}
              onSnooze={() => void snooze(n.id, 30)}
              onDismiss={() => void dismiss(n.id)}
            />
          ))
        )}
      </div>

      <div className="border-t border-line px-4 py-2 text-[11px] text-ink-faint">
        提醒只在软件内提示；关闭窗口到托盘后仍会在下次打开时列出。
      </div>
    </Modal>
  );
}

function NotificationRow({
  n,
  onComplete,
  onSnooze,
  onDismiss,
}: {
  n: Notification;
  onComplete: () => void;
  onSnooze: () => void;
  onDismiss: () => void;
}) {
  return (
    <div
      className={cn(
        "mb-1 rounded-lg border px-3 py-2.5",
        n.readAt ? "border-line bg-panel" : "border-accent/30 bg-accent-soft/40",
      )}
    >
      <div className="flex items-start gap-2">
        <div className="min-w-0 flex-1">
          <p className="text-[13px] font-medium text-ink">{n.title}</p>
          <p className="mt-0.5 text-[12px] leading-relaxed text-ink-muted">{n.body}</p>
          <p className="mt-1 flex items-center gap-1 text-[11px] text-ink-faint">
            <Clock size={10} />
            {timeOf(n.createdAt)} · {n.kind === "daily_digest" ? "每日汇总" : "到期提醒"}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          {n.entryId != null && (
            <>
              <Button variant="primary" size="sm" onClick={onComplete}>
                <Check size={12} /> 完成
              </Button>
              <Button variant="ghost" size="sm" onClick={onSnooze}>
                稍后
              </Button>
            </>
          )}
          <Button variant="ghost" size="sm" onClick={onDismiss}>
            忽略
          </Button>
        </div>
      </div>
    </div>
  );
}
