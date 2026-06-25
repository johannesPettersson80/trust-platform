// Shared design tokens for the truST webviews — VS Code-native, 2026-refined.
//
// Every value resolves to a VS Code theme variable, so the UI adapts to the user's theme (light OR
// dark) instead of the old hard-coded dark literals. Each token carries a dark fallback for
// standalone/test rendering (and themes that don't define a given variable). Tokens are plain CSS
// strings so they drop straight into inline `React.CSSProperties` — no CSS-in-JS runtime.
//
// North star: Linear-grade calm precision + n8n/Figma canvas craft. Color means status (the only
// saturated accent on a node body) or protocol-on-wire — never decoration. Hairline borders + soft
// elevation, ~6px radii, restrained type, motion as feedback.

const v = (name: string, fallback: string): string => `var(${name}, ${fallback})`;

export const t = {
  // — Surfaces (theme-aware elevation: editor < widget < hover) —
  canvas: v("--vscode-editor-background", "#0f1116"),
  surface: v("--vscode-editorWidget-background", "#1b1f28"),
  surfaceRaised: v("--vscode-editorHoverWidget-background", "#222732"),
  overlay: v("--vscode-editorHoverWidget-background", "#12151c"),

  // — Text —
  text: v("--vscode-foreground", "#cfd6e0"),
  textMuted: v("--vscode-descriptionForeground", "#949cab"),
  textSubtle: v("--vscode-disabledForeground", "#6b7480"),
  onAccent: v("--vscode-button-foreground", "#ffffff"),

  // — Lines —
  border: v("--vscode-editorWidget-border", v("--vscode-panel-border", "#2a2f3a")),
  borderSubtle: v("--vscode-panel-border", "#23272f"),
  accent: v("--vscode-focusBorder", "#4a9eff"),

  // — Status: the one saturated signal on a node body (paired with a label elsewhere) —
  ok: v("--vscode-charts-green", v("--vscode-testing-iconPassed", "#46c265")),
  warn: v("--vscode-charts-yellow", v("--vscode-editorWarning-foreground", "#e0b341")),
  danger: v("--vscode-charts-red", v("--vscode-errorForeground", "#f0584f")),
  idle: v("--vscode-descriptionForeground", "#6b7480"),

  // — Inputs (search, fields) —
  inputBg: v("--vscode-input-background", "#10141b"),
  inputBorder: v("--vscode-input-border", v("--vscode-editorWidget-border", "#343b47")),

  // — Type —
  mono: v("--vscode-editor-font-family", "ui-monospace, SFMono-Regular, Menlo, monospace"),

  // — Shape —
  radiusSm: 4,
  radius: 6,
  radiusLg: 8,
  pill: 999,

  // — Motion + elevation (soft; reads in light and dark) —
  ease: "150ms cubic-bezier(.4, 0, .2, 1)",
  shadow: "0 1px 2px rgba(0, 0, 0, .14), 0 3px 10px rgba(0, 0, 0, .10)",
  shadowOverlay: "0 8px 28px rgba(0, 0, 0, .28)",
} as const;

// A faint status tint for borders/fills — e.g. `tint(t.ok, 0.4)` → a translucent layer over the
// surface. Used sparingly so status reads without flooding the node body.
export function tint(color: string, alpha: number): string {
  return `color-mix(in srgb, ${color} ${Math.round(alpha * 100)}%, transparent)`;
}
