import { useCallback, useEffect, useRef, useState } from "react";

/** 长按多久进入拖动（毫秒） */
const LONG_PRESS_MS = 400;
/** 长按等待期间移动超过这个距离就当成普通点击/滚动，取消长按 */
const MOVE_CANCEL_PX = 6;

type SortState = {
  /** 正在拖动的项目 id */
  draggingId: number | null;
  /** 插入位置（0..n，n 表示放到最后） */
  overIndex: number | null;
};

/**
 * 侧边栏项目「长按拖动排序」（v0.14）。
 *
 * - 按住 400ms 进入拖动，期间移动超过 6px 视为普通点击，不进入拖动；
 * - 拖动时按鼠标位置算出插入位置，松手后回调新的 id 顺序；
 * - 拖动结束后抑制随后的 click，避免顺带打开项目。
 */
export function useLongPressSort(
  ids: number[],
  onReorder: (ids: number[]) => void,
  enabled = true,
) {
  const [state, setState] = useState<SortState>({ draggingId: null, overIndex: null });
  const containerRef = useRef<HTMLDivElement | null>(null);
  const idsRef = useRef(ids);
  idsRef.current = ids;

  const timerRef = useRef<number | null>(null);
  const startRef = useRef<{ x: number; y: number; id: number } | null>(null);
  const draggingRef = useRef<number | null>(null);
  const overRef = useRef<number | null>(null);
  const suppressClickRef = useRef(false);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const rowEls = useCallback((): HTMLElement[] => {
    const root = containerRef.current;
    if (!root) return [];
    return Array.from(root.querySelectorAll<HTMLElement>("[data-sort-id]"));
  }, []);

  const computeOver = useCallback(
    (clientY: number): number => {
      const els = rowEls();
      for (let i = 0; i < els.length; i += 1) {
        const rect = els[i].getBoundingClientRect();
        if (clientY < rect.top + rect.height / 2) return i;
      }
      return els.length;
    },
    [rowEls],
  );

  // 拖动中：跟随鼠标更新插入位置，松手提交
  useEffect(() => {
    if (state.draggingId === null) return;

    const onMove = (event: MouseEvent) => {
      const index = computeOver(event.clientY);
      overRef.current = index;
      setState((prev) => (prev.overIndex === index ? prev : { ...prev, overIndex: index }));
    };

    const onUp = () => {
      const from = draggingRef.current;
      const to = overRef.current;
      draggingRef.current = null;
      overRef.current = null;
      setState({ draggingId: null, overIndex: null });
      if (from !== null && to !== null) {
        const list = [...idsRef.current];
        const fromIndex = list.indexOf(from);
        if (fromIndex >= 0) {
          list.splice(fromIndex, 1);
          const insertAt = to > fromIndex ? to - 1 : to;
          list.splice(insertAt, 0, from);
          if (list.join(",") !== idsRef.current.join(",")) onReorder(list);
        }
        // 拖动结束时浏览器可能补发一次 click，这里抑制掉
        suppressClickRef.current = true;
        window.setTimeout(() => {
          suppressClickRef.current = false;
        }, 0);
      }
    };

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [state.draggingId, computeOver, onReorder]);

  // 长按等待期间：移动太多就取消
  useEffect(() => {
    if (state.draggingId !== null) return;
    const onMove = (event: MouseEvent) => {
      const start = startRef.current;
      if (!start || timerRef.current === null) return;
      if (
        Math.abs(event.clientX - start.x) + Math.abs(event.clientY - start.y) >
        MOVE_CANCEL_PX
      ) {
        clearTimer();
        startRef.current = null;
      }
    };
    window.addEventListener("mousemove", onMove);
    return () => window.removeEventListener("mousemove", onMove);
  }, [state.draggingId, clearTimer]);

  useEffect(() => clearTimer, [clearTimer]);

  const rowProps = useCallback(
    (id: number) => ({
      "data-sort-id": id,
      onMouseDown: (event: React.MouseEvent<HTMLElement>) => {
        if (!enabled || event.button !== 0) return;
        const target = event.target as HTMLElement;
        if (target.closest("[data-no-sort]")) return;
        clearTimer();
        startRef.current = { x: event.clientX, y: event.clientY, id };
        timerRef.current = window.setTimeout(() => {
          timerRef.current = null;
          draggingRef.current = id;
          const index = computeOver(event.clientY);
          overRef.current = index;
          setState({ draggingId: id, overIndex: index });
        }, LONG_PRESS_MS);
      },
      onMouseUp: () => {
        clearTimer();
        startRef.current = null;
      },
      onClickCapture: (event: React.MouseEvent<HTMLElement>) => {
        if (suppressClickRef.current) {
          event.preventDefault();
          event.stopPropagation();
        }
      },
    }),
    [enabled, clearTimer, computeOver],
  );

  /** 是否正在拖动 */
  const dragging = state.draggingId !== null;

  /** 在第 index 行之前画一条插入线 */
  const showIndicatorBefore = (index: number) =>
    dragging && state.overIndex === index && state.draggingId !== null;

  return {
    containerRef,
    rowProps,
    dragging,
    draggingId: state.draggingId,
    overIndex: state.overIndex,
    showIndicatorBefore,
  };
}
