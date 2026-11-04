import { Bell } from "lucide-react";
import { cn } from "../lib/cn";
import { useNotifications } from "../stores/notifications";

/** 侧边栏顶部的通知入口：未读角标 + 打开通知中心 */
export function NotificationBell() {
  const unread = useNotifications((s) => s.unread);
  const open = useNotifications((s) => s.panelOpen);
  const setOpen = useNotifications((s) => s.setPanelOpen);

  return (
    <button
      type="button"
      onClick={() => setOpen(!open)}
      className={cn(
        "relative flex h-8 items-center gap-2 rounded-md px-2.5 text-[13px] transition-colors",
        open ? "bg-accent-soft font-medium text-accent" : "text-ink-muted hover:bg-panel hover:text-ink",
      )}
    >
      <Bell size={15} />
      通知
      {unread > 0 && (
        <span className="ml-auto flex h-[18px] min-w-[18px] items-center justify-center rounded-full bg-accent px-1 text-[10.5px] font-semibold text-white">
          {unread > 99 ? "99+" : unread}
        </span>
      )}
    </button>
  );
}
