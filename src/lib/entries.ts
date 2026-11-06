import type { Entry } from "./types";

/**
 * 「有效完成态」（v0.8，用户确认口径）：
 * 父任务只要还有未完成的子任务，就视为**未完成**；
 * 子任务全部完成时，后端会自动给父任务打勾，所以正常情况下 `completedAt` 已经一致，
 * 这里再兜一层，避免历史数据（或旧版本写入）出现「父任务勾了但子任务没做完」。
 */
export function isEffectivelyDone(
  entry: Pick<Entry, "completedAt" | "childTotal" | "childDone">,
): boolean {
  const total = entry.childTotal ?? 0;
  const done = entry.childDone ?? 0;
  if (total > 0 && done < total) return false;
  return Boolean(entry.completedAt);
}

/** 逾期但未完成 → 才该显示红色（已完成的一律不红） */
export function isOverdueRed(
  entry: Pick<Entry, "completedAt" | "childTotal" | "childDone">,
  tone: string,
): boolean {
  return tone === "overdue" && !isEffectivelyDone(entry);
}
