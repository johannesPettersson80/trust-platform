// Styles for the panel's shell: the toolbar and the canvas of computer frames, runtime cards,
// connection pills, devices and mesh buses. Colours come from the shared roles in
// `src/webview/theme.ts`; the words that carry meaning always come from the view JSON, never from
// here (design §3.4).

import type React from "react";

import { t, tint } from "../../webview/theme";
import type { Rect } from "./layout";
import type { Emphasis, Look, Mark, Tone } from "./types";

/** The panel: the toolbar above the canvas. */
export const appStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  width: "100%",
  height: "100%",
  background: t.canvas,
  color: t.text,
};

/** The toolbar strip. */
export const toolbarStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  flex: "none",
  padding: "10px 16px",
  borderBottom: `1px solid ${t.border}`,
  background: t.surface,
};

/** One toolbar item, drawn from the action's own `emphasis` and `enabled`. */
export function toolbarButtonStyle(emphasis: Emphasis, enabled: boolean): React.CSSProperties {
  const primary = emphasis === "primary";
  return {
    border: `1px solid ${primary ? t.accent : t.border}`,
    background: primary ? t.accent : "transparent",
    color: enabled ? (primary ? t.onAccent : t.text) : t.textSubtle,
    borderRadius: t.radius,
    padding: "6px 12px",
    fontSize: 12,
    fontWeight: primary ? 650 : 500,
    cursor: enabled ? "pointer" : "not-allowed",
    opacity: enabled ? 1 : 0.62,
    whiteSpace: "nowrap",
  };
}

/** The canvas: the summary and banner above a graph of absolutely placed items. */
export const canvasStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  alignItems: "stretch",
  flex: "1 1 auto",
  minHeight: 0,
  overflow: "auto",
  background: t.canvas,
};

/** The graph: a positioned area holding the items and the SVG edge layer. */
export const graphStyle: React.CSSProperties = {
  position: "relative",
  flex: "none",
};

/** The SVG edge layer over the graph; it takes no pointer events, so items stay clickable. */
export const svgStyle: React.CSSProperties = {
  position: "absolute",
  left: 0,
  top: 0,
  overflow: "visible",
  pointerEvents: "none",
};

/** Places an item at its layout rectangle, dimmed when the view marks it so. */
export function positionedStyle(rect: Rect, dimmed: boolean): React.CSSProperties {
  return {
    position: "absolute",
    left: rect.x,
    top: rect.y,
    width: rect.width,
    height: rect.height,
    boxSizing: "border-box",
    opacity: dimmed ? 0.45 : 1,
  };
}

/** The summary strip above the graph. */
export const summaryBarStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  flex: "none",
  padding: "6px 16px",
  borderBottom: `1px solid ${t.border}`,
  background: t.surface,
};

/** The summary line. */
export const summaryTextStyle: React.CSSProperties = {
  fontSize: 12,
  color: t.textMuted,
};

/** The banner, coloured by its own tone. */
export function bannerStyle(tone: Tone): React.CSSProperties {
  const colour = toneColour(tone);
  return {
    display: "flex",
    alignItems: "center",
    gap: 10,
    flex: "none",
    padding: "8px 16px",
    borderBottom: `1px solid ${tint(colour, 0.5)}`,
    background: tint(colour, 0.12),
  };
}

/** The banner's text. */
export const bannerTextStyle: React.CSSProperties = {
  flex: "1 1 auto",
  minWidth: 0,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  fontSize: 12,
  color: t.text,
};

/** A banner action, drawn like a toolbar button. */
export function bannerButtonStyle(emphasis: Emphasis, enabled: boolean): React.CSSProperties {
  return toolbarButtonStyle(emphasis, enabled);
}

/** A connection pill, dashed or solid by its own look. */
export function connectionPillStyle(look: Look): React.CSSProperties {
  return {
    display: "flex",
    alignItems: "center",
    gap: 6,
    overflow: "hidden",
    boxSizing: "border-box",
    padding: "3px 8px",
    border: `1px ${look === "normal" ? "solid" : "dashed"} ${t.roleRuntimeBorder}`,
    borderRadius: t.radiusSm,
    background: t.roleEndpointBg,
  };
}

/** The status dot of a connection pill; it pulses when the view says `live`. */
export function connectionDotStyle(tone: Tone, live: boolean): React.CSSProperties {
  const colour = toneColour(tone);
  return {
    flex: "none",
    width: 8,
    height: 8,
    borderRadius: "50%",
    background: colour,
    boxShadow: live ? `0 0 0 3px ${tint(colour, 0.3)}` : "none",
  };
}

/** The connection's own name on its pill. */
export const connectionNameStyle: React.CSSProperties = {
  flex: "1 1 auto",
  minWidth: 0,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  fontSize: 11.5,
  color: t.text,
};

/** The role word on a connection pill. */
export const connectionRoleStyle: React.CSSProperties = {
  flex: "none",
  fontSize: 10,
  fontWeight: 700,
  letterSpacing: 0.4,
  color: t.textMuted,
};

/**
 * A `draft` or `disabled` mark: it shows the view's own word, with the product's casing applied in
 * CSS (`DRAFT`, `Disabled`), so no word is decided here (design §2.2).
 */
export function markStyle(mark: Mark): React.CSSProperties {
  return {
    flex: "none",
    padding: "1px 5px",
    border: `1px solid ${tint(t.warn, 0.5)}`,
    borderRadius: t.pill,
    fontSize: 9.5,
    fontWeight: 700,
    color: t.warn,
    textTransform: mark === "draft" ? "uppercase" : "capitalize",
  };
}

/** A device card. */
export const deviceCardStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 4,
  overflow: "hidden",
  padding: 10,
  border: `1px solid ${t.roleExternalBorder}`,
  borderRadius: t.radius,
  background: t.roleExternalBg,
};

/** The device's name. */
export const deviceNameStyle: React.CSSProperties = {
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  fontSize: 12.5,
  fontWeight: 600,
  color: t.text,
};

/** What the device presents. */
export const devicePresentsStyle: React.CSSProperties = {
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  fontSize: 11,
  color: t.textMuted,
};

/** A mesh bus. */
export const meshBusStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  overflow: "hidden",
  padding: "8px 12px",
  border: `2px solid ${t.borderSubtle}`,
  borderRadius: t.pill,
  background: t.surfaceRaised,
};

/** The mesh bus's label, drawn only when the view gives one. */
export const meshLabelStyle: React.CSSProperties = {
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  fontSize: 12,
  fontWeight: 600,
  color: t.text,
};

/** An Edit slot, drawn like a quiet toolbar button. */
export function slotButtonStyle(emphasis: Emphasis, enabled: boolean): React.CSSProperties {
  return { ...toolbarButtonStyle(emphasis, enabled), padding: "3px 8px", fontSize: 11 };
}

/** A computer frame. Its size comes from `layout.ts`. */
export const frameStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 12,
  padding: 14,
  border: `1px solid ${t.roleHostBorder}`,
  borderRadius: t.radiusLg,
  background: t.roleHostBg,
};

/** The frame's title block. */
export const frameHeaderStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
  minWidth: 0,
};

/** The frame's heading. */
export const headingStyle: React.CSSProperties = {
  margin: 0,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  fontSize: 13,
  fontWeight: 600,
  color: t.text,
};

/** The runtimes of a frame, side by side. */
export const runtimeRowStyle: React.CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  gap: 10,
  alignItems: "stretch",
};

/** A runtime card. */
export function cardStyle(selected: boolean): React.CSSProperties {
  return {
    display: "flex",
    flexDirection: "column",
    gap: 6,
    flex: "1 1 200px",
    minWidth: 0,
    padding: 12,
    border: `1px solid ${selected ? t.accent : t.roleRuntimeBorder}`,
    borderRadius: t.radius,
    background: t.roleRuntimeBg,
    cursor: "pointer",
  };
}

/** Long names stay readable at 1280 × 800: one line, cut with an ellipsis, full text on hover. */
export const runtimeNameStyle: React.CSSProperties = {
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  fontSize: 13,
  fontWeight: 600,
  color: t.text,
};

/** The computer platform under a runtime's name. */
export const platformStyle: React.CSSProperties = {
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  fontSize: 11,
  color: t.textMuted,
};

const toneColours: Record<Tone, string> = {
  ok: t.ok,
  warning: t.warn,
  error: t.danger,
  neutral: t.textMuted,
  muted: t.textSubtle,
};

/** The colour of a tone, for an item or an edge that carries one. */
export function toneColour(tone: Tone): string {
  return toneColours[tone];
}

/** A status pill, coloured by the pill's own tone. */
export function pillStyle(tone: Tone): React.CSSProperties {
  const colour = toneColours[tone];
  return {
    display: "inline-flex",
    alignItems: "center",
    width: "fit-content",
    padding: "3px 9px",
    border: `1px solid ${tint(colour, 0.5)}`,
    borderRadius: t.pill,
    background: tint(colour, 0.12),
    color: colour,
    fontSize: 11.5,
    fontWeight: 600,
  };
}
