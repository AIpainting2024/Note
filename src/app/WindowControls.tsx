import { Copy, Minus, Square, X } from "lucide-react";
import { useEffect } from "react";
import { cn } from "../lib/cn";
import { useWindowChrome } from "../lib/windowChrome";

/**
 * 自绘窗口按钮（v0.13）。
 *
 * 主窗口 `decorations: false`，系统标题栏被真正移除（不是 CSS 隐藏），
 * 最小化 / 最大化·还原 / 关闭由这里调用 Tauri 窗口 API 完成。
 * 按钮本身是 <button>，Tauri 的 drag-region 逻辑会跳过可点击元素，不会误触发拖动。
 */
export function WindowControls() {
  const maximized = useWindowChrome((s) => s.maximized);
  const sync = useWindowChrome((s) => s.sync);
  const toggleMaximize = useWindowChrome((s) => s.toggleMaximize);
  const minimize = useWindowChrome((s) => s.minimize);
  const close = useWindowChrome((s) => s.close);

  useEffect(() => {
    void sync();
  }, [sync]);

  const base =
    "flex h-8 w-[46px] items-center justify-center text-ink-faint transition-colors";

  return (
    <div
      data-no-drag
      className="fixed right-0 top-0 z-[80] flex select-none items-center"
    >
      <button
        type="button"
        title="最小化"
        aria-label="最小化"
        onClick={() => void minimize()}
        className={cn(base, "hover:bg-black/5 hover:text-ink")}
      >
        <Minus size={14} />
      </button>
      <button
        type="button"
        title={maximized ? "向下还原" : "最大化"}
        aria-label={maximized ? "向下还原" : "最大化"}
        onClick={() => void toggleMaximize()}
        className={cn(base, "hover:bg-black/5 hover:text-ink")}
      >
        {maximized ? <Copy size={12} /> : <Square size={12} />}
      </button>
      <button
        type="button"
        title="关闭"
        aria-label="关闭"
        onClick={() => void close()}
        className={cn(base, "hover:bg-[#e5484d] hover:text-white")}
      >
        <X size={15} />
      </button>
    </div>
  );
}
