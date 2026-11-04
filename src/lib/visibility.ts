import type { Entry } from "./types";
import { useSecret } from "../stores/secret";

/**
 * 保密项目锁定时的可见性判定。
 *
 * 口径（已与需求确认）：
 * - 未解锁：保密项目下的记录/任务在任何列表里都不出现（今日 / 待办 / 搜索 / 侧边栏）。
 * - 已解锁：正常显示。
 * - 日历与四象限：**永远**不显示保密任务（后端查询已排除，前端这里再兜一层）。
 */
export function useSecretFilter() {
  const unlocked = useSecret((s) => s.status.unlocked);
  return (entry: Pick<Entry, "projectIsSecret">) => !unlocked && entry.projectIsSecret === true;
}

/** 过滤掉锁定状态下的保密记录 */
export function useVisibleEntries() {
  const hidden = useSecretFilter();
  return (entries: Entry[]) => entries.filter((entry) => !hidden(entry));
}
