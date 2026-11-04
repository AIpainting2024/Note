/**
 * 快捷键字符串解析：后端用 Tauri 的 accelerator 语法（CommandOrControl+Shift+N），
 * 前端在应用内匹配 keydown 时用同一套字符串，避免两处各写一份规则。
 */

export type ParsedShortcut = {
  ctrl: boolean;
  meta: boolean;
  shift: boolean;
  alt: boolean;
  key: string; // 小写单字符或 "f1"… "space"
};

/** macOS 上 CommandOrControl 指 Command（⌘），Windows/Linux 上指 Ctrl */
const IS_MAC =
  typeof navigator !== "undefined" && /mac/i.test((navigator as Navigator).platform ?? "");

const ALIASES: Record<string, string> = {
  commandorcontrol: "mod",
  cmdorctrl: "mod",
  command: "meta",
  cmd: "meta",
  control: "ctrl",
  ctrl: "ctrl",
  shift: "shift",
  alt: "alt",
  option: "alt",
};

/** 解析失败返回 null */
export function parseShortcut(accel: string): ParsedShortcut | null {
  const parts = accel
    .split("+")
    .map((p) => p.trim())
    .filter(Boolean);
  if (parts.length < 2) return null;

  const parsed: ParsedShortcut = { ctrl: false, meta: false, shift: false, alt: false, key: "" };
  let key = "";

  for (const raw of parts) {
    const lower = raw.toLowerCase();
    const alias = ALIASES[lower];
    if (alias === "mod") {
      // macOS = Command，Windows/Linux = Ctrl
      if (IS_MAC) parsed.meta = true;
      else parsed.ctrl = true;
    } else if (alias === "ctrl") {
      parsed.ctrl = true;
    } else if (alias === "meta") {
      parsed.meta = true;
    } else if (alias === "shift") {
      parsed.shift = true;
    } else if (alias === "alt") {
      parsed.alt = true;
    } else {
      key = lower;
    }
  }

  if (!key) return null;
  parsed.key = key;
  return parsed;
}

/** 判断键盘事件是否命中该快捷键 */
export function matchShortcut(accel: string, e: KeyboardEvent): boolean {
  const parsed = parseShortcut(accel);
  if (!parsed) return false;
  const key = e.key.toLowerCase();
  const keyMatches = key === parsed.key || (parsed.key === "space" && key === " ");
  return (
    keyMatches &&
    e.ctrlKey === parsed.ctrl &&
    e.metaKey === parsed.meta &&
    e.shiftKey === parsed.shift &&
    e.altKey === parsed.alt
  );
}

/** 展示用：macOS 显示 ⌘⇧N 这类符号，Windows/Linux 显示 Ctrl + Shift + N */
export function displayShortcut(accel: string): string {
  if (IS_MAC) {
    return accel
      .replace(/CommandOrControl/gi, "⌘")
      .replace(/Command|Cmd/gi, "⌘")
      .replace(/Control|Ctrl/gi, "⌃")
      .replace(/Option|Alt/gi, "⌥")
      .replace(/Shift/gi, "⇧")
      .replace(/\+/g, "");
  }
  return accel.replace(/CommandOrControl/gi, "Ctrl").replace(/\+/g, " + ");
}

export { IS_MAC };

/** 校验用户输入的组合是否有效（至少一个修饰键 + 一个主键） */
export function validateShortcut(accel: string): string | null {
  const trimmed = accel.trim();
  if (!trimmed) return "请填写快捷键";
  const parsed = parseShortcut(trimmed);
  if (!parsed) return "格式不对，示例：Ctrl+Shift+N";
  if (!parsed.ctrl && !parsed.meta && !parsed.alt) return "至少需要 Ctrl / Alt 其中一个修饰键";
  if (/^[a-z0-9]$/.test(parsed.key) === false && !/^f\d{1,2}$/.test(parsed.key) && parsed.key !== "space") {
    return "主键只支持字母、数字、F1–F12 或 Space";
  }
  return null;
}
