// Shared design tokens for truST webviews.
//
// VS Code theme variables are the source of truth. Fallbacks mirror Devices &
// Connections so standalone/test rendering still looks like the shipped product.
// Colour means status, selection, or protocol identity; never decoration.

const v = (name: string, fallback: string): string => `var(${name}, ${fallback})`;

export const t = {
  canvas: v("--vscode-editor-background", "#0f1116"),
  surface: v("--vscode-editorWidget-background", "#1b1f28"),
  surfaceRaised: v("--vscode-editorHoverWidget-background", "#222732"),
  overlay: v("--vscode-editorHoverWidget-background", "#12151c"),

  text: v("--vscode-foreground", "#cfd6e0"),
  textMuted: v("--trust-text-muted", v("--vscode-descriptionForeground", "#949cab")),
  textSubtle: v("--trust-text-subtle", v("--vscode-disabledForeground", "#6b7480")),
  onAccent: v("--vscode-button-foreground", "#ffffff"),

  border: v("--vscode-editorWidget-border", v("--vscode-panel-border", "#2a2f3a")),
  borderSubtle: v("--vscode-panel-border", "#23272f"),
  accent: v("--vscode-focusBorder", "#4a9eff"),

  ok: v("--vscode-charts-green", v("--vscode-testing-iconPassed", "#46c265")),
  warn: v("--vscode-charts-yellow", v("--vscode-editorWarning-foreground", "#e0b341")),
  danger: v("--vscode-charts-red", v("--vscode-errorForeground", "#f0584f")),
  idle: v("--trust-idle", v("--vscode-descriptionForeground", "#6b7480")),
  breakpoint: v("--trust-breakpoint", v("--vscode-debugIcon-breakpointForeground", "#e51400")),

  inputBg: v("--vscode-input-background", "#10141b"),
  inputBorder: v("--vscode-input-border", v("--vscode-editorWidget-border", "#343b47")),

  protocolBlue: v("--trust-protocol-blue", v("--vscode-charts-blue", "#5b9bd5")),
  protocolOrange: v("--trust-protocol-orange", v("--vscode-charts-orange", "#d29152")),
  protocolGreen: v("--trust-protocol-green", v("--vscode-charts-green", "#5cb46c")),
  protocolCyan: v("--trust-protocol-cyan", "color-mix(in srgb, var(--trust-protocol-blue) 62%, var(--trust-protocol-green) 38%)"),
  protocolRed: v("--trust-protocol-red", v("--vscode-charts-red", "#d2756f")),
  protocolPurple: v("--trust-protocol-purple", v("--vscode-charts-purple", "#b39bef")),
  protocolMuted: v("--trust-protocol-muted", v("--vscode-descriptionForeground", "#6b7480")),

  roleHostBg: v("--trust-role-host-bg", "color-mix(in srgb, var(--trust-protocol-blue) 10%, var(--trust-surface) 90%)"),
  roleHostBorder: v("--trust-role-host-border", "color-mix(in srgb, var(--trust-protocol-blue) 36%, var(--trust-border) 64%)"),
  roleRuntimeBg: v("--trust-role-runtime-bg", "color-mix(in srgb, var(--trust-protocol-green) 9%, var(--trust-surface-raised) 91%)"),
  roleRuntimeBorder: v("--trust-role-runtime-border", "color-mix(in srgb, var(--trust-protocol-green) 34%, var(--trust-border) 66%)"),
  roleEndpointBg: v("--trust-role-endpoint-bg", "color-mix(in srgb, var(--trust-protocol-cyan) 8%, var(--trust-surface) 92%)"),
  roleExternalBg: v("--trust-role-external-bg", "color-mix(in srgb, var(--trust-protocol-orange) 8%, var(--trust-surface) 92%)"),
  roleExternalBorder: v("--trust-role-external-border", "color-mix(in srgb, var(--trust-protocol-orange) 32%, var(--trust-border) 68%)"),

  gridLine: v("--trust-grid-line", "#2a2f3a"),
  rail: v("--trust-rail", "#8a93a3"),
  selectedBg: v("--trust-selected-bg", "rgba(74, 158, 255, 0.18)"),
  selectedStrongBg: v("--trust-selected-strong-bg", "rgba(74, 158, 255, 0.28)"),
  ladderWire: v("--trust-ladder-wire", "#6fba8a"),
  ladderWireLive: v("--trust-ladder-live", "#e0b341"),
  ladderPreview: v("--trust-ladder-preview", "#f0b36b"),
  ladderBody: v("--trust-ladder-body", "#131821"),
  ladderText: v("--trust-ladder-text", "#cfd6e0"),
  blockLogic: v("--trust-block-logic", "#5c81a6"),
  blockLoop: v("--trust-block-loop", "#5ca65c"),
  blockMath: v("--trust-block-math", "#5c68a6"),
  blockVariables: v("--trust-block-variables", "#a55b99"),
  blockFunctions: v("--trust-block-functions", "#9a5ca6"),
  blockIo: v("--trust-block-io", "#d19a4d"),
  blockTimer: v("--trust-block-timer", "#d1684d"),
  blockCounter: v("--trust-block-counter", "#4d97d1"),

  mono: v("--vscode-editor-font-family", "ui-monospace, SFMono-Regular, Menlo, monospace"),

  radiusSm: 4,
  radius: 6,
  radiusLg: 8,
  pill: 999,

  ease: "150ms cubic-bezier(.4, 0, .2, 1)",
  shadow: "0 1px 2px rgba(0, 0, 0, .14), 0 3px 10px rgba(0, 0, 0, .10)",
  shadowOverlay: "0 8px 28px rgba(0, 0, 0, .28)",
} as const;

export function tint(color: string, alpha: number): string {
  return `color-mix(in srgb, ${color} ${Math.round(alpha * 100)}%, transparent)`;
}

export function canvasColor(color: string): string {
  const runtime = globalThis as {
    document?: {
      body?: {
        appendChild: (node: unknown) => void;
      };
      createElement: (tagName: string) => {
        style: Record<string, string>;
        remove: () => void;
      };
    };
    getComputedStyle?: (node: unknown) => { color?: string };
  };
  const doc = runtime.document;

  if (!doc?.body || typeof runtime.getComputedStyle !== "function") {
    return color;
  }

  const probe = doc.createElement("span");
  probe.style.position = "absolute";
  probe.style.visibility = "hidden";
  probe.style.pointerEvents = "none";
  probe.style.color = color;
  doc.body.appendChild(probe);
  const resolved = runtime.getComputedStyle(probe).color;
  probe.remove();

  return resolved || color;
}
