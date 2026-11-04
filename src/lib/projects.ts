/** 项目色板：Tailwind 不能动态拼类名，统一用 hex 内联 */
export const PROJECT_COLORS: { key: string; label: string; hex: string }[] = [
  { key: "indigo", label: "靛蓝", hex: "#5b6cff" },
  { key: "teal", label: "青绿", hex: "#22b8a6" },
  { key: "amber", label: "琥珀", hex: "#e0a13a" },
  { key: "rose", label: "玫红", hex: "#f2637c" },
  { key: "violet", label: "紫罗兰", hex: "#a78bfa" },
  { key: "slate", label: "石板", hex: "#8b93a7" },
];

const HEX: Record<string, string> = Object.fromEntries(
  PROJECT_COLORS.map((c) => [c.key, c.hex]),
);

export function projectHex(color?: string | null): string {
  return (color && HEX[color]) || HEX.slate;
}

export const PROJECT_STATUS_LABEL: Record<string, string> = {
  active: "进行中",
  paused: "暂停",
  done: "已完成",
  archived: "已归档",
};

export const PROJECT_STATUS_OPTIONS = [
  { key: "active", label: "进行中" },
  { key: "paused", label: "暂停" },
  { key: "done", label: "已完成" },
  { key: "archived", label: "已归档" },
] as const;

/** 解析快速输入末尾的 `#项目名`，返回正文与匹配到的项目名（不做数据库匹配） */
export function splitProjectToken(text: string): { content: string; token: string | null } {
  const match = /(?:^|\s)#([^\s#]+)\s*$/.exec(text);
  if (!match) return { content: text.trim(), token: null };
  const content = text.slice(0, match.index).trim();
  return { content: content || text.trim(), token: match[1] };
}
