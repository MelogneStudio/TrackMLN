export type ThemeId =
  | "grey"
  | "green"
  | "mint"
  | "blue"
  | "purple"
  | "pink"
  | "orange"
  | "yellow"
  | "black"
  | "light";

export type ThemeDef = {
  id: ThemeId;
  label: string;
  mode: "dark" | "light";
  /** Accent used for charts, rings, buttons. */
  accent: string;
  /** How strongly the accent tints the surfaces (0..1). */
  tint: number;
  /** Colour of the swatch in the picker. */
  swatch: string;
  /** Overrides the base background (dark/black/light bases). */
  base?: string;
};

export const THEMES: ThemeDef[] = [
  { id: "grey", label: "Grey", mode: "dark", accent: "#8c8c8c", tint: 0, swatch: "#7a7a7a" },
  { id: "green", label: "Green", mode: "dark", accent: "#a7dc7b", tint: 0.05, swatch: "#a7dc7b" },
  { id: "mint", label: "Mint", mode: "dark", accent: "#74d6b0", tint: 0.05, swatch: "#74d6b0" },
  { id: "blue", label: "Blue", mode: "dark", accent: "#7ab6ff", tint: 0.05, swatch: "#7ab6ff" },
  { id: "purple", label: "Purple", mode: "dark", accent: "#b9a2ff", tint: 0.06, swatch: "#b9a2ff" },
  { id: "pink", label: "Pink", mode: "dark", accent: "#ff9fcd", tint: 0.06, swatch: "#ff9fcd" },
  { id: "orange", label: "Orange", mode: "dark", accent: "#ffb272", tint: 0.05, swatch: "#ffb272" },
  { id: "yellow", label: "Yellow", mode: "dark", accent: "#f1d46a", tint: 0.05, swatch: "#f1d46a" },
  { id: "black", label: "Black", mode: "dark", accent: "#9a9a9a", tint: 0, swatch: "#2b2b2b", base: "#050505" },
  { id: "light", label: "Light", mode: "light", accent: "#4f8a3a", tint: 0.03, swatch: "#f1f1ee", base: "#f4f4f1" }
];

export function themeById(id: string): ThemeDef {
  return THEMES.find((theme) => theme.id === id) ?? THEMES[0];
}

type Rgb = [number, number, number];

function hexToRgb(hex: string): Rgb {
  const value = hex.replace("#", "");
  const full = value.length === 3 ? value.split("").map((c) => c + c).join("") : value;
  return [
    parseInt(full.slice(0, 2), 16),
    parseInt(full.slice(2, 4), 16),
    parseInt(full.slice(4, 6), 16)
  ];
}

function mix(a: Rgb, b: Rgb, amount: number): Rgb {
  return [
    Math.round(a[0] + (b[0] - a[0]) * amount),
    Math.round(a[1] + (b[1] - a[1]) * amount),
    Math.round(a[2] + (b[2] - a[2]) * amount)
  ];
}

const css = (rgb: Rgb) => `rgb(${rgb[0]} ${rgb[1]} ${rgb[2]})`;
const cssA = (rgb: Rgb, alpha: number) => `rgb(${rgb[0]} ${rgb[1]} ${rgb[2]} / ${alpha})`;

function luminance([r, g, b]: Rgb): number {
  return (0.299 * r + 0.587 * g + 0.114 * b) / 255;
}

/** Translucency slider (0 = solid sidebar, 100 = very see-through) -> sidebar alpha. */
export function sidebarAlpha(translucency: number): number {
  const clamped = Math.max(0, Math.min(100, translucency));
  return Math.max(0.1, 1 - clamped * 0.0085);
}

export function applyTheme(themeId: ThemeId, translucency: number): ThemeDef {
  const theme = themeById(themeId);
  const root = document.documentElement;
  const accent = hexToRgb(theme.accent);
  const dark = theme.mode === "dark";

  const white: Rgb = [255, 255, 255];
  const black: Rgb = [0, 0, 0];

  const baseBg = hexToRgb(theme.base ?? (dark ? "#1a1a1a" : "#f4f4f1"));
  const bg = mix(baseBg, accent, theme.tint);
  const raised = dark ? mix(bg, white, 0.035) : mix(bg, white, 0.7);
  const sunken = dark ? mix(bg, black, 0.25) : mix(bg, black, 0.04);
  const border = dark ? mix(bg, white, 0.17) : mix(bg, black, 0.14);
  const borderStrong = dark ? mix(bg, white, 0.3) : mix(bg, black, 0.26);
  const text: Rgb = dark ? [244, 244, 244] : [24, 24, 24];
  const accentUi: Rgb = dark ? accent : mix(accent, black, 0.1);
  const accentFg: Rgb = luminance(accentUi) > 0.58 ? [16, 16, 16] : [255, 255, 255];

  const sidebarBase = dark ? mix(bg, black, theme.id === "black" ? 0 : 0.18) : mix(bg, white, 0.35);

  const set = (name: string, value: string) => root.style.setProperty(name, value);

  set("--bg", css(bg));
  set("--bg-rgb", bg.join(" "));
  set("--bg-raised", css(raised));
  set("--bg-sunken", css(sunken));
  set("--border", css(border));
  set("--border-strong", css(borderStrong));
  set("--text", css(text));
  set("--text-dim", cssA(text, 0.64));
  set("--text-faint", cssA(text, 0.4));
  set("--accent", css(accentUi));
  set("--accent-rgb", accentUi.join(" "));
  set("--accent-soft", cssA(accentUi, 0.16));
  set("--accent-line", cssA(accentUi, 0.45));
  set("--accent-fg", css(accentFg));
  set("--track", dark ? cssA(white, 0.09) : cssA(black, 0.09));
  set("--hover", dark ? cssA(white, 0.06) : cssA(black, 0.06));
  set("--danger", dark ? "#ff7b7b" : "#c93a3a");
  set("--warning", dark ? "#f2c35b" : "#a87410");
  set("--sidebar-bg", cssA(sidebarBase, sidebarAlpha(translucency)));
  set("--sidebar-border", dark ? "rgb(255 255 255 / 0.08)" : "rgb(0 0 0 / 0.08)");
  set("--shadow", dark ? "0 18px 50px rgb(0 0 0 / 0.45)" : "0 18px 50px rgb(0 0 0 / 0.18)");
  set("--overlay-bg", cssA(sidebarBase, Math.max(0.28, sidebarAlpha(translucency) * 0.72)));

  root.dataset.theme = theme.id;
  root.dataset.mode = theme.mode;
  root.style.colorScheme = theme.mode;

  return theme;
}
