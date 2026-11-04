import { format, parseISO } from "date-fns";
import { zhCN } from "date-fns/locale";

export const WEEKDAYS = ["日", "一", "二", "三", "四", "五", "六"];

/** 本地日期 YYYY-MM-DD */
export function todayStr(d: Date = new Date()): string {
  return format(d, "yyyy-MM-dd");
}

export function parseDay(day: string): Date {
  return parseISO(`${day}T00:00:00`);
}

/** 2026-11-03 → 11月3日 星期一 */
export function humanDay(day: string): string {
  const d = parseDay(day);
  return `${d.getMonth() + 1}月${d.getDate()}日 星期${WEEKDAYS[d.getDay()]}`;
}

/** 2026-11-03 → 11月3日 */
export function shortDay(day: string): string {
  const d = parseDay(day);
  return `${d.getMonth() + 1}月${d.getDate()}日`;
}

export function addDays(day: string, n: number): string {
  const d = parseDay(day);
  d.setDate(d.getDate() + n);
  return todayStr(d);
}

/** 所在周的周一 */
export function weekStart(day: string): string {
  const d = parseDay(day);
  const offset = (d.getDay() + 6) % 7;
  d.setDate(d.getDate() - offset);
  return todayStr(d);
}

export function weekDays(day: string): string[] {
  const start = weekStart(day);
  return Array.from({ length: 7 }, (_, i) => addDays(start, i));
}

export function relativeDayLabel(day: string): string {
  const today = todayStr();
  if (day === today) return "今天";
  if (day === addDays(today, -1)) return "昨天";
  if (day === addDays(today, 1)) return "明天";
  return humanDay(day);
}

/** 2026-11-03T19:56:47 → 19:56 */
export function timeOf(iso: string | null): string {
  if (!iso) return "";
  const t = iso.includes("T") ? iso.split("T")[1] : "";
  return t.slice(0, 5);
}

export function nowTimeStr(d: Date = new Date()): string {
  return format(d, "HH:mm");
}

export type DueTone = "overdue" | "today" | "soon" | "later";

/**
 * 截止时间文案。有 dueTime 时精确到分钟，并把「今天但已过点」视为逾期。
 */
export function dueLabel(
  dueDate: string,
  dueTime?: string | null,
): { text: string; tone: DueTone } {
  const today = todayStr();
  const now = nowTimeStr();
  const hasTime = Boolean(dueTime);
  const timeSuffix = hasTime ? ` ${dueTime}` : "";

  if (dueDate < today) {
    const diff = Math.round((parseDay(today).getTime() - parseDay(dueDate).getTime()) / 86400000);
    return { text: diff === 1 ? "昨天到期" : `逾期 ${diff} 天`, tone: "overdue" };
  }

  if (dueDate === today) {
    if (hasTime && (dueTime as string) < now) {
      return { text: `今天${timeSuffix} 已过`, tone: "overdue" };
    }
    return { text: `今天${timeSuffix}`, tone: "today" };
  }

  const diff = Math.round((parseDay(dueDate).getTime() - parseDay(today).getTime()) / 86400000);
  if (diff === 1) return { text: `明天${timeSuffix}`, tone: "soon" };
  if (diff <= 3) return { text: `${diff} 天后${timeSuffix}`, tone: "soon" };
  return { text: `${shortDay(dueDate)}${timeSuffix}`, tone: "later" };
}

export function monthLabel(d: Date): string {
  return format(d, "yyyy 年 M 月", { locale: zhCN });
}

export function fileStamp(d: Date = new Date()): string {
  return format(d, "yyyy-MM-dd_HHmmss");
}

/** 时间轴分段：把一天切成四段，用于日历日/周视图 */
export const TIME_SLOTS: { key: string; label: string; from: number; to: number }[] = [
  { key: "morning", label: "上午", from: 6, to: 12 },
  { key: "noon", label: "中午", from: 12, to: 14 },
  { key: "afternoon", label: "下午", from: 14, to: 18 },
  { key: "evening", label: "晚上", from: 18, to: 24 },
];

/** HH:MM → 落在哪个时间段的 key；无时间返回 "allDay" */
export function slotOf(dueTime: string | null): string {
  if (!dueTime) return "allDay";
  const hour = Number(dueTime.slice(0, 2));
  if (Number.isNaN(hour)) return "allDay";
  if (hour < 6) return "evening"; // 凌晨归到晚上显示，避免空段
  for (const slot of TIME_SLOTS) {
    if (hour >= slot.from && hour < slot.to) return slot.key;
  }
  return "evening";
}
