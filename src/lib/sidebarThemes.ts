/**
 * 侧边栏配色（v0.11）。
 *
 * 用户口径：左侧边栏颜色相对较深，右侧内容区用同色系更浅的颜色。
 * 每个方案给出一组 CSS 变量，浅色/深色模式各一套，运行时写到 <html> 上。
 */

export type SidebarThemeKey =
  | "default"
  | "bamboo"
  | "paper"
  | "moon"
  | "indigo"
  | "lotus"
  | "tea"
  | "sky"
  | "ash";

type Palette = {
  /** 侧边栏底色 */
  sidebar: string;
  /** 侧边栏边框/分隔线 */
  sidebarLine: string;
  /** 侧边栏主文字 */
  sidebarInk: string;
  /** 侧边栏次级文字 */
  sidebarInkMuted: string;
  /** 侧边栏更淡的文字 */
  sidebarInkFaint: string;
  /** 侧边栏悬停/选中底色 */
  sidebarHover: string;
  /** 主背景（比侧边栏浅） */
  canvas: string;
  /** 卡片/面板底色 */
  panel: string;
  /** 次级面板底色 */
  panel2: string;
};

export type SidebarTheme = {
  key: SidebarThemeKey;
  label: string;
  /** 色板预览用主色 */
  swatch: string;
  light: Palette;
  dark: Palette;
};

const DEFAULT_LIGHT: Palette = {
  sidebar: "#fbfbfc",
  sidebarLine: "#e6e6ea",
  sidebarInk: "#1b1b1f",
  sidebarInkMuted: "#6b6b75",
  sidebarInkFaint: "#9a9aa4",
  sidebarHover: "#ffffff",
  canvas: "#f6f6f7",
  panel: "#ffffff",
  panel2: "#fbfbfc",
};

const DEFAULT_DARK: Palette = {
  sidebar: "#222228",
  sidebarLine: "#2c2c34",
  sidebarInk: "#e9e9ee",
  sidebarInkMuted: "#9a9aa6",
  sidebarInkFaint: "#6f6f7a",
  sidebarHover: "#2a2a31",
  canvas: "#151519",
  panel: "#1c1c21",
  panel2: "#222228",
};

export const SIDEBAR_THEMES: SidebarTheme[] = [
  {
    key: "default",
    label: "默认",
    swatch: "#e6e6ea",
    light: DEFAULT_LIGHT,
    dark: DEFAULT_DARK,
  },
  {
    key: "bamboo",
    label: "竹叶青",
    swatch: "#3f6146",
    light: {
      sidebar: "#3f6146",
      sidebarLine: "#4d7355",
      sidebarInk: "#eef4ed",
      sidebarInkMuted: "#c6d6c4",
      sidebarInkFaint: "#9fb3a0",
      sidebarHover: "#4a7053",
      canvas: "#f2f7f1",
      panel: "#ffffff",
      panel2: "#f6faf5",
    },
    dark: {
      sidebar: "#26362b",
      sidebarLine: "#33473a",
      sidebarInk: "#e3ede3",
      sidebarInkMuted: "#a9bdaa",
      sidebarInkFaint: "#7d927f",
      sidebarHover: "#2f4234",
      canvas: "#151a16",
      panel: "#1b211c",
      panel2: "#1f2620",
    },
  },
  {
    key: "paper",
    label: "宣纸黄",
    swatch: "#eadfc6",
    light: {
      sidebar: "#f0e7d2",
      sidebarLine: "#ded2b8",
      sidebarInk: "#3b3327",
      sidebarInkMuted: "#6b5f49",
      sidebarInkFaint: "#8f8168",
      sidebarHover: "#f8f1e1",
      canvas: "#fcf9f1",
      panel: "#fffefb",
      panel2: "#faf6ea",
    },
    dark: {
      sidebar: "#3a332a",
      sidebarLine: "#4b4236",
      sidebarInk: "#efe7d8",
      sidebarInkMuted: "#c2b49b",
      sidebarInkFaint: "#93866f",
      sidebarHover: "#463d31",
      canvas: "#191612",
      panel: "#211d18",
      panel2: "#262119",
    },
  },
  {
    key: "moon",
    label: "月白",
    swatch: "#cfdde6",
    light: {
      sidebar: "#dbe7ee",
      sidebarLine: "#c3d4de",
      sidebarInk: "#24333c",
      sidebarInkMuted: "#4e6472",
      sidebarInkFaint: "#7a8e9b",
      sidebarHover: "#e9f2f7",
      canvas: "#f6fafc",
      panel: "#ffffff",
      panel2: "#f4f9fb",
    },
    dark: {
      sidebar: "#28323a",
      sidebarLine: "#36434d",
      sidebarInk: "#e6eef3",
      sidebarInkMuted: "#aebfcb",
      sidebarInkFaint: "#82949f",
      sidebarHover: "#313d46",
      canvas: "#141a1e",
      panel: "#1a2126",
      panel2: "#1e262c",
    },
  },
  {
    key: "indigo",
    label: "黛蓝",
    swatch: "#33465c",
    light: {
      sidebar: "#33465c",
      sidebarLine: "#415570",
      sidebarInk: "#eaf0f6",
      sidebarInkMuted: "#b8c6d6",
      sidebarInkFaint: "#93a5b8",
      sidebarHover: "#3d5270",
      canvas: "#f2f5f8",
      panel: "#ffffff",
      panel2: "#f5f8fa",
    },
    dark: {
      sidebar: "#22303f",
      sidebarLine: "#2e3f52",
      sidebarInk: "#e2eaf2",
      sidebarInkMuted: "#a8b8c8",
      sidebarInkFaint: "#7b8b9c",
      sidebarHover: "#2a3a4b",
      canvas: "#121821",
      panel: "#181f29",
      panel2: "#1c242f",
    },
  },
  {
    key: "lotus",
    label: "藕荷",
    swatch: "#c6b2c2",
    light: {
      sidebar: "#dcccd8",
      sidebarLine: "#cbb5c6",
      sidebarInk: "#3a2c36",
      sidebarInkMuted: "#6b5663",
      sidebarInkFaint: "#8b7482",
      sidebarHover: "#eadde7",
      canvas: "#faf5f8",
      panel: "#ffffff",
      panel2: "#f8f2f6",
    },
    dark: {
      sidebar: "#352a33",
      sidebarLine: "#453741",
      sidebarInk: "#f0e6ee",
      sidebarInkMuted: "#c6b0bf",
      sidebarInkFaint: "#9a8492",
      sidebarHover: "#41333e",
      canvas: "#181418",
      panel: "#1f1a1f",
      panel2: "#231d23",
    },
  },
  {
    key: "tea",
    label: "茶褐",
    swatch: "#6b5442",
    light: {
      sidebar: "#6b5442",
      sidebarLine: "#7d6553",
      sidebarInk: "#f3ede6",
      sidebarInkMuted: "#d2c1b2",
      sidebarInkFaint: "#b3a08f",
      sidebarHover: "#7a614d",
      canvas: "#f7f2ec",
      panel: "#ffffff",
      panel2: "#faf6f1",
    },
    dark: {
      sidebar: "#3b2f26",
      sidebarLine: "#4c3d31",
      sidebarInk: "#f0e8df",
      sidebarInkMuted: "#c3b2a1",
      sidebarInkFaint: "#94826f",
      sidebarHover: "#46382c",
      canvas: "#171310",
      panel: "#1e1915",
      panel2: "#231d18",
    },
  },
  {
    key: "sky",
    label: "天空蓝",
    swatch: "#3c7fa6",
    light: {
      sidebar: "#3c7fa6",
      sidebarLine: "#4e91b8",
      sidebarInk: "#eaf4fa",
      sidebarInkMuted: "#c2ddec",
      sidebarInkFaint: "#9dc2d6",
      sidebarHover: "#4a8db4",
      canvas: "#f1f8fc",
      panel: "#ffffff",
      panel2: "#f5fafd",
    },
    dark: {
      sidebar: "#26485c",
      sidebarLine: "#31596f",
      sidebarInk: "#e2eff6",
      sidebarInkMuted: "#a8c6d6",
      sidebarInkFaint: "#7e9cad",
      sidebarHover: "#2d5468",
      canvas: "#111a20",
      panel: "#172128",
      panel2: "#1b262e",
    },
  },
  {
    key: "ash",
    label: "瓦灰色",
    swatch: "#5a5c5e",
    light: {
      sidebar: "#5a5c5e",
      sidebarLine: "#6a6c6e",
      sidebarInk: "#f0f0ef",
      sidebarInkMuted: "#cbccc9",
      sidebarInkFaint: "#a5a6a2",
      sidebarHover: "#67696b",
      canvas: "#f5f5f3",
      panel: "#ffffff",
      panel2: "#f8f8f6",
    },
    dark: {
      sidebar: "#333537",
      sidebarLine: "#424446",
      sidebarInk: "#e9e9e7",
      sidebarInkMuted: "#b4b5b2",
      sidebarInkFaint: "#888a86",
      sidebarHover: "#3d3f41",
      canvas: "#141515",
      panel: "#1a1b1b",
      panel2: "#1e1f1f",
    },
  },
];

export function findSidebarTheme(key: string | undefined): SidebarTheme {
  return SIDEBAR_THEMES.find((t) => t.key === key) ?? SIDEBAR_THEMES[0];
}

/** 把配色写到 <html> 上；主题（浅/深）变化时需要重新调用。 */
export function applySidebarTheme(key: string | undefined, isDark: boolean) {
  const theme = findSidebarTheme(key);
  const palette = isDark ? theme.dark : theme.light;
  const root = document.documentElement;
  root.style.setProperty("--c-sidebar", palette.sidebar);
  root.style.setProperty("--c-sidebar-line", palette.sidebarLine);
  root.style.setProperty("--c-sidebar-ink", palette.sidebarInk);
  root.style.setProperty("--c-sidebar-ink-muted", palette.sidebarInkMuted);
  root.style.setProperty("--c-sidebar-ink-faint", palette.sidebarInkFaint);
  root.style.setProperty("--c-sidebar-hover", palette.sidebarHover);
  root.style.setProperty("--c-canvas", palette.canvas);
  root.style.setProperty("--c-panel", palette.panel);
  root.style.setProperty("--c-panel-2", palette.panel2);
}
