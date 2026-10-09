// Design tokens of the HMI (specification 39, §14.1): the only colours the drawing uses.
//
// Dark-first: a near-black page with a cool tint, cards one step lighter, off-white text, one
// vivid cyan-to-indigo accent for live values, and separate hues for stale data (amber),
// commanded targets (violet), warnings (amber) and alarms (rose red), with red, orange and yellow
// for the alarm priorities. Trend pens get four hues that keep clear of the warning and alarm
// colours. The light theme takes the deeper shades of the same hues, so every text and sign the
// drawing draws keeps its contrast on the light page (§14.1, item 4).

import { FLOW_PERIOD_MS, SPIN_PERIOD_MS } from "./symbols/motion";

export type TokenTheme = Record<string, string>;

/**
 * The Chinese, Japanese and Korean fonts the HMI ships beside Inter (§14.5 item 2), each drawing
 * its language's forms of the characters the three share, and the code points they cover, so a
 * browser loads one only for a page that shows such text.
 */
export const CJK_FONTS = [
  { code: "SC", family: "truST CJK SC", file: "NotoSansSC-Subset.woff2" },
  { code: "JP", family: "truST CJK JP", file: "NotoSansJP-Subset.woff2" },
  { code: "KR", family: "truST CJK KR", file: "NotoSansKR-Subset.woff2" },
] as const;
const CJK_RANGE =
  "U+1100-11FF, U+2E80-2FDF, U+3000-33FF, U+3400-4DBF, U+4E00-9FFF, U+A960-A97F, U+AC00-D7AF, U+F900-FAFF, U+FE30-FE4F, U+FF00-FFEF";

/** The HMI's font: Inter, then the CJK fonts with a language's own first, then the device's. */
function fontStack(first: (typeof CJK_FONTS)[number]["code"][]): string {
  const cjk = first.map((code) => `"${CJK_FONTS.find((font) => font.code === code)?.family}"`).join(", ");
  return `Inter, ${cjk}, "Segoe UI Variable Display", "Segoe UI", system-ui, -apple-system, Roboto, "Helvetica Neue", Arial, sans-serif`;
}

const FONT = fontStack(["SC", "JP", "KR"]);
/** Japanese and Korean take their own forms first; Chinese and every other language the default. */
const LANGUAGE_FONTS: Array<[string, string]> = [
  ["ja", fontStack(["JP", "SC", "KR"])],
  ["ko", fontStack(["KR", "JP", "SC"])],
];

export const TOKEN_THEMES: Record<"light" | "dark", TokenTheme> = {
  dark: {
    page: "#06080c",
    surface: "#0d1117",
    raised: "#161c26",
    border: "#1d2430",
    line: "#1a212c",
    text: "#eceff4",
    muted: "#8a94a8",
    accent: "#22d3ee",
    "accent-2": "#6366f1",
    "accent-3": "#a855f7",
    "pen-1": "#22d3ee",
    "pen-2": "#a3e635",
    "pen-3": "#818cf8",
    "pen-4": "#e2e8f0",
    unknown: "#667085",
    stale: "#e3b24f",
    pending: "#a78bfa",
    warning: "#fbbf24",
    alarm: "#fb4f6e",
    "alarm-high": "#fb4f6e",
    "alarm-medium": "#fb923c",
    "alarm-low": "#facc15",
    font: FONT,
  },
  light: {
    page: "#f3f5f9",
    surface: "#ffffff",
    raised: "#eef1f6",
    border: "#e3e8ef",
    line: "#e6eaf1",
    text: "#0b1220",
    muted: "#475569",
    accent: "#0e7490",
    "accent-2": "#4f46e5",
    "accent-3": "#9333ea",
    "pen-1": "#0e7490",
    "pen-2": "#4d7c0f",
    "pen-3": "#4f46e5",
    "pen-4": "#334155",
    unknown: "#64748b",
    stale: "#92400e",
    pending: "#5b21b6",
    warning: "#92400e",
    alarm: "#be123c",
    "alarm-high": "#be123c",
    "alarm-medium": "#ea580c",
    "alarm-low": "#a16207",
    font: FONT,
  },
};

/** A project's own colours (§14.1): token name to hex colour, per theme. */
export interface HmiColorsModel {
  dark?: Record<string, string>;
  light?: Record<string, string>;
}

const HEX_COLOUR = /^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;
/** Inter at the given address, and the CJK fonts beside it, each only for the text it covers. */
function fontFaces(fontUrl: string): string {
  const folder = fontUrl.slice(0, fontUrl.lastIndexOf("/") + 1);
  const face = (family: string, url: string, range?: string): string =>
    `@font-face {\n  font-family: "${family}";\n  src: url("${url}") format("woff2");\n  font-weight: 100 900;\n  font-style: normal;\n  font-display: swap;\n${range ? `  unicode-range: ${range};\n` : ""}}\n`;
  return (
    face("Inter", fontUrl) + CJK_FONTS.map((font) => face(font.family, `${folder}${font.file}`, CJK_RANGE)).join("")
  );
}

/** A font address that cannot leave its `url("…")`. */
const SAFE_URL = /^[^"'()\\\s<>]+$/;

/**
 * The tokens as CSS: one class per theme, applied to the drawing's container. A project's own
 * colours replace the defaults; only known colour tokens with hex values are taken, so nothing
 * else can reach the page's CSS. Given the address of the bundled Inter font, it is declared
 * first so the drawing does not depend on the fonts installed on the device.
 */
export function tokensCss(colors: HmiColorsModel = {}, fontUrl?: string): string {
  const themes = (Object.entries(TOKEN_THEMES) as Array<["light" | "dark", TokenTheme]>).map(([theme, defaults]) => {
    const tokens = { ...defaults };
    for (const [name, value] of Object.entries(colors[theme] ?? {})) {
      if (name !== "font" && name in defaults && HEX_COLOUR.test(value)) tokens[name] = value;
    }
    const declarations = Object.entries(tokens)
      .map(([name, value]) => `  --hmi-${name}: ${value};`)
      .join("\n");
    return `.hmi-theme-${theme} {\n${declarations}\n  background: var(--hmi-page);\n  color-scheme: ${theme};\n}`;
  });
  const fonts = fontUrl !== undefined && SAFE_URL.test(fontUrl) ? fontFaces(fontUrl) : "";
  // A language's font goes where the theme sets the tokens; the page or drawing says its language.
  const languages = LANGUAGE_FONTS.map(
    ([language, stack]) => `:is(.hmi-theme-dark, .hmi-theme-light):lang(${language}) {\n  --hmi-font: ${stack};\n}`,
  ).join("\n");
  return `${fonts}${themes.join("\n")}\n${languages}\n.hmi-page { font-family: var(--hmi-font); font-optical-sizing: none; }\n${MOTION_CSS}`;
}

/**
 * What moves in the plant moves in its symbol (§14.2): an impeller turns once per period, and the
 * drawing gives each turning part a negative delay so a redraw continues the turn. When the
 * viewer's system asks for reduced motion, nothing turns.
 */
const MOTION_CSS = `@keyframes hmi-spin { to { transform: rotate(360deg); } }
.hmi-spin { transform-box: fill-box; transform-origin: center; animation: hmi-spin ${SPIN_PERIOD_MS}ms linear infinite; }
@keyframes hmi-stroke { to { transform: translateX(8px); } }
.hmi-stroke { animation: hmi-stroke ${SPIN_PERIOD_MS / 2}ms ease-in-out infinite alternate; }
@keyframes hmi-flow { to { stroke-dashoffset: -16; } }
.hmi-flow { animation: hmi-flow ${FLOW_PERIOD_MS}ms linear infinite; }
.hmi-flow-back { animation: hmi-flow ${FLOW_PERIOD_MS}ms linear infinite reverse; }
@media (prefers-reduced-motion: reduce) { .hmi-spin { animation: none; } .hmi-stroke, .hmi-flow, .hmi-flow-back { animation: none; } }
`;
