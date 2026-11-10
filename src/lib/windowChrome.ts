import { currentMonitor, getCurrentWindow } from "@tauri-apps/api/window";
import { PhysicalPosition, PhysicalSize } from "@tauri-apps/api/dpi";
import { create } from "zustand";

type Bounds = { x: number; y: number; w: number; h: number };

type State = {
  /** 是否处于自绘最大化状态 */
  maximized: boolean;
  /** 最大化前的窗口位置与尺寸（还原用） */
  saved: Bounds | null;
  /** 启动时同步一次（窗口可能被系统还原/缩放） */
  sync: () => Promise<void>;
  toggleMaximize: () => Promise<void>;
  minimize: () => Promise<void>;
  close: () => Promise<void>;
  /** 拖动窗口（按住自绘标题区时调用） */
  startDrag: () => Promise<void>;
};

const isTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function currentBounds(): Promise<Bounds> {
  const win = getCurrentWindow();
  const pos = await win.outerPosition();
  const size = await win.outerSize();
  return { x: pos.x, y: pos.y, w: size.width, h: size.height };
}

/** 无边框窗口仍有隐形边框：setSize 设的是内部尺寸，需要按内外差补偿才能得到目标外框尺寸 */
async function setOuterSize(win: ReturnType<typeof getCurrentWindow>, w: number, h: number) {
  const inner = await win.innerSize();
  const outer = await win.outerSize();
  const dx = outer.width - inner.width;
  const dy = outer.height - inner.height;
  await win.setSize(new PhysicalSize(Math.max(1, w - dx), Math.max(1, h - dy)));
}

async function workArea(): Promise<{ x: number; y: number; w: number; h: number } | null> {
  const monitor = await currentMonitor();
  if (!monitor) return null;
  const area = monitor.workArea;
  return { x: area.position.x, y: area.position.y, w: area.size.width, h: area.size.height };
}

export const useWindowChrome = create<State>((set, get) => ({
  maximized: false,
  saved: null,

  sync: async () => {
    if (!isTauri()) return;
    try {
      const win = getCurrentWindow();
      if (await win.isMaximized()) {
        set({ maximized: true });
        return;
      }
      // 自绘最大化后系统 IsZoomed 仍为 false，用「尺寸是否等于工作区」判断
      const area = await workArea();
      const bounds = await currentBounds();
      if (area) {
        const same =
          Math.abs(bounds.x - area.x) <= 2 &&
          Math.abs(bounds.y - area.y) <= 2 &&
          Math.abs(bounds.w - area.w) <= 2 &&
          Math.abs(bounds.h - area.h) <= 2;
        set({ maximized: same });
      }
    } catch {
      /* 忽略 */
    }
  },

  toggleMaximize: async () => {
    if (!isTauri()) return;
    const win = getCurrentWindow();
    const { maximized, saved } = get();
    try {
      if (!maximized) {
        const area = await workArea();
        if (!area) {
          // 拿不到工作区就退回系统最大化
          await win.maximize();
          set({ maximized: true });
          return;
        }
        set({ saved: await currentBounds() });
        await setOuterSize(win, area.w, area.h);
        await win.setPosition(new PhysicalPosition(area.x, area.y));
        set({ maximized: true });
      } else {
        const target = saved;
        set({ maximized: false, saved: null });
        if (target) {
          await setOuterSize(win, target.w, target.h);
          await win.setPosition(new PhysicalPosition(target.x, target.y));
        }
      }
    } catch {
      /* 忽略 */
    }
  },

  minimize: async () => {
    if (!isTauri()) return;
    await getCurrentWindow().minimize();
  },

  close: async () => {
    if (!isTauri()) return;
    await getCurrentWindow().close();
  },

  startDrag: async () => {
    if (!isTauri()) return;
    await getCurrentWindow().startDragging();
  },
}));

const INTERACTIVE =
  "button, a, input, textarea, select, label, summary, [role='button'], [role='link'], [data-no-drag]";

/**
 * 自绘标题区事件：按住可拖动窗口，双击切换最大化。
 * 可交互控件（按钮/输入框等）不触发拖动，避免影响搜索框、按钮。
 */
export function dragRegionProps() {
  return {
    onMouseDown: (event: React.MouseEvent<HTMLElement>) => {
      if (event.button !== 0) return;
      const target = event.target as HTMLElement;
      if (target.closest(INTERACTIVE)) return;
      event.preventDefault();
      // 第一下拖动，第二下（detail=2）切最大化 —— 与系统标题栏行为一致
      if (event.detail === 2) void useWindowChrome.getState().toggleMaximize();
      else void useWindowChrome.getState().startDrag();
    },
  };
}
