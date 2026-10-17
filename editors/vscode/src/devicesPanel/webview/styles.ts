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

/** The canvas and the right-hand inspector side by side. */
export const bodyStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "row",
  alignItems: "stretch",
  flex: "1 1 auto",
  minHeight: 0,
};

/** The inspector: a fixed right-hand panel that fills the height and scrolls. */
export const inspectorStyle: React.CSSProperties = {
  width: 380,
  flex: "none",
  display: "flex",
  flexDirection: "column",
  overflowY: "auto",
  borderLeft: `1px solid ${t.border}`,
  background: t.surface,
};

/** The inspector's header: the breadcrumb, the title, the kind and the close button. */
export const inspectorHeaderStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "flex-start",
  gap: 10,
  padding: "16px 16px 10px",
};

/** The header's text block. */
export const inspectorHeadingStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 2,
  flex: "1 1 auto",
  minWidth: 0,
};

/** The item's breadcrumb, such as `Devices & Connections / Runtime`. */
export const inspectorBreadcrumbStyle: React.CSSProperties = {
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  fontSize: 11,
  color: t.textMuted,
};

/** The item's name. */
export const inspectorTitleStyle: React.CSSProperties = {
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  fontSize: 15,
  fontWeight: 650,
  color: t.text,
};

/** The item's kind, drawn under its name. */
export const inspectorKindStyle: React.CSSProperties = {
  fontSize: 11.5,
  color: t.textMuted,
};

/** The item's health as a dot in its own tone. */
export function inspectorHealthStyle(tone: Tone): React.CSSProperties {
  return {
    flex: "none",
    width: 10,
    height: 10,
    marginTop: 5,
    borderRadius: "50%",
    background: toneColour(tone),
  };
}

/** The inspector's close button. */
export const inspectorCloseStyle: React.CSSProperties = {
  flex: "none",
  border: "none",
  background: "transparent",
  color: t.textMuted,
  cursor: "pointer",
  fontSize: 14,
  lineHeight: 1,
  padding: 4,
};

/** The inspector's primary and secondary actions, in a two-column grid. */
export const inspectorActionsStyle: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
  gap: 8,
  padding: "0 16px 14px",
  borderBottom: `1px solid ${t.border}`,
};

/** One action and, when it has one, its reason, in its grid cell. */
export const actionCellStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 4,
  minWidth: 0,
};

/** One inspector action, drawn by its own `emphasis` and `enabled`. */
export function inspectorActionStyle(emphasis: Emphasis, enabled: boolean): React.CSSProperties {
  const filled = emphasis === "primary";
  const outline = emphasis === "danger" ? t.danger : emphasis === "primary" ? t.accent : t.border;
  return {
    width: "100%",
    border: `1px solid ${enabled ? outline : t.border}`,
    background: filled && enabled ? t.accent : "transparent",
    color: !enabled
      ? t.textSubtle
      : filled
        ? t.onAccent
        : emphasis === "danger"
          ? t.danger
          : t.text,
    borderRadius: t.radius,
    padding: "7px 10px",
    fontSize: 12,
    fontWeight: filled ? 650 : 500,
    cursor: enabled ? "pointer" : "not-allowed",
    opacity: enabled ? 1 : 0.62,
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
  };
}

/** The reason a disabled action carries, shown under it. */
export const actionReasonStyle: React.CSSProperties = {
  fontSize: 11,
  lineHeight: 1.35,
  color: t.textMuted,
};

/** The two-step confirmation of an action. */
export const confirmStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
  padding: 8,
  border: `1px solid ${tint(t.danger, 0.5)}`,
  borderRadius: t.radius,
  background: tint(t.danger, 0.08),
};

/** The confirmation's text. */
export const confirmTextStyle: React.CSSProperties = {
  fontSize: 11.5,
  lineHeight: 1.35,
  color: t.text,
};

/** The confirmation's buttons, side by side. */
export const confirmRowStyle: React.CSSProperties = {
  display: "flex",
  gap: 6,
};

/** One button of a confirmation. */
export function confirmButtonStyle(danger: boolean): React.CSSProperties {
  return {
    flex: "1 1 0",
    border: `1px solid ${danger ? t.danger : t.border}`,
    background: "transparent",
    color: danger ? t.danger : t.text,
    borderRadius: t.radiusSm,
    padding: "4px 8px",
    fontSize: 11.5,
    fontWeight: 600,
    cursor: "pointer",
  };
}

/** One inspector section. */
export const sectionStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
  padding: "12px 16px",
  borderBottom: `1px solid ${t.border}`,
};

/** A section's heading. */
export const sectionHeadingStyle: React.CSSProperties = {
  margin: 0,
  fontSize: 12,
  fontWeight: 650,
  color: t.text,
};

/** One inspector row; a `wide` row stacks its value under its label. */
export function rowStyle(wide: boolean): React.CSSProperties {
  return wide
    ? { display: "flex", flexDirection: "column", gap: 2 }
    : { display: "flex", alignItems: "baseline", justifyContent: "space-between", gap: 10 };
}

/** A row's label. */
export const rowLabelStyle: React.CSSProperties = {
  flex: "none",
  fontSize: 11.5,
  color: t.textMuted,
};

/** A row's value, coloured by its own tone. */
export function rowValueStyle(tone: Tone | undefined, wide: boolean): React.CSSProperties {
  return {
    flex: "1 1 auto",
    minWidth: 0,
    textAlign: wide ? "left" : "right",
    fontSize: 12,
    color: tone === undefined ? t.text : toneColour(tone),
    overflowWrap: "anywhere",
  };
}

/** The copy button of a row that carries copyable text. */
export const copyButtonStyle: React.CSSProperties = {
  flex: "none",
  alignSelf: "flex-start",
  border: `1px solid ${t.border}`,
  background: "transparent",
  color: t.textMuted,
  borderRadius: t.radiusSm,
  padding: "1px 6px",
  fontSize: 10.5,
  cursor: "pointer",
};

/** The right-click menu, placed at the pointer. */
export function contextMenuStyle(x: number, y: number): React.CSSProperties {
  return {
    position: "fixed",
    left: x,
    top: y,
    zIndex: 20,
    display: "flex",
    flexDirection: "column",
    gap: 4,
    minWidth: 220,
    padding: 6,
    border: `1px solid ${t.border}`,
    borderRadius: t.radius,
    background: t.overlay,
    boxShadow: t.shadowOverlay,
  };
}

/** The toolbar's issues button, drawn in the tone the view gives it. */
export function issuesButtonStyle(tone: Tone): React.CSSProperties {
  const colour = toneColour(tone);
  return {
    ...toolbarButtonStyle("secondary", true),
    border: `1px solid ${tint(colour, 0.5)}`,
    color: colour,
  };
}

/** The `Topology`/`List` switch. */
export const viewSwitchStyle: React.CSSProperties = {
  display: "flex",
  gap: 2,
  padding: 2,
  border: `1px solid ${t.border}`,
  borderRadius: t.radius,
};

/** One button of the switch, filled while it is the shown view. */
export function viewSwitchButtonStyle(selected: boolean): React.CSSProperties {
  return {
    border: "none",
    background: selected ? t.surfaceRaised : "transparent",
    color: selected ? t.text : t.textMuted,
    borderRadius: t.radiusSm,
    padding: "4px 10px",
    fontSize: 12,
    fontWeight: selected ? 600 : 500,
    cursor: "pointer",
    whiteSpace: "nowrap",
  };
}

/** The summary strip's issues action, pushed to the strip's end. */
export function summaryIssuesButtonStyle(enabled: boolean): React.CSSProperties {
  return {
    marginLeft: "auto",
    border: `1px solid ${t.border}`,
    background: "transparent",
    color: enabled ? t.text : t.textSubtle,
    borderRadius: t.radiusSm,
    padding: "2px 8px",
    fontSize: 11.5,
    cursor: enabled ? "pointer" : "not-allowed",
    opacity: enabled ? 1 : 0.62,
    whiteSpace: "nowrap",
  };
}

/** The issues list: a fixed right-hand pane that fills the height and scrolls. */
export const issuesListStyle: React.CSSProperties = {
  width: 380,
  flex: "none",
  display: "flex",
  flexDirection: "column",
  overflowY: "auto",
  borderLeft: `1px solid ${t.border}`,
  background: t.surface,
};

/** The issues list's header: the button's text and the close button. */
export const issuesHeaderStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "16px 16px 10px",
};

/** The list's title, in the tone the view gives the button. */
export function issuesTitleStyle(tone: Tone): React.CSSProperties {
  return {
    flex: "1 1 auto",
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    fontSize: 15,
    fontWeight: 650,
    color: toneColour(tone),
  };
}

/** One item's group of issues. */
export const issueGroupStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 8,
  padding: "12px 16px",
  borderBottom: `1px solid ${t.border}`,
};

/** The group's item name. */
export const issueGroupNameStyle: React.CSSProperties = {
  margin: 0,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  fontSize: 12,
  fontWeight: 650,
  color: t.text,
};

/** One fault or warning. */
export const issueStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
  padding: "8px 10px",
  border: `1px solid ${t.borderSubtle}`,
  borderRadius: t.radius,
  background: t.surfaceRaised,
};

/** The issue's severity, drawn as the view gives it. */
export const issueSeverityStyle: React.CSSProperties = {
  alignSelf: "flex-start",
  padding: "1px 6px",
  border: `1px solid ${t.border}`,
  borderRadius: t.pill,
  fontSize: 10,
  fontWeight: 700,
  letterSpacing: 0.3,
  color: t.textMuted,
};

/** The issue's text. */
export const issueTextStyle: React.CSSProperties = {
  fontSize: 12,
  lineHeight: 1.4,
  color: t.text,
  overflowWrap: "anywhere",
};

/** The issue's `Show` and recovery actions, side by side. */
export const issuesActionsStyle: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
  gap: 8,
};

/** The list: the same items as the canvas in one table, filling the left. */
export const listStyle: React.CSSProperties = {
  flex: "1 1 auto",
  minWidth: 0,
  minHeight: 0,
  overflow: "auto",
  background: t.canvas,
};

/** The list's table. */
export const listTableStyle: React.CSSProperties = {
  width: "100%",
  borderCollapse: "collapse",
  fontSize: 12,
};

/** A heading cell. */
export const listHeadingStyle: React.CSSProperties = {
  position: "sticky",
  top: 0,
  padding: 0,
  borderBottom: `1px solid ${t.border}`,
  background: t.surface,
  textAlign: "left",
};

/** The heading's button, which sorts the rows by the column's text. */
export const listHeadingButtonStyle: React.CSSProperties = {
  width: "100%",
  border: "none",
  background: "transparent",
  color: t.textMuted,
  padding: "8px 12px",
  fontSize: 11.5,
  fontWeight: 650,
  textAlign: "left",
  cursor: "pointer",
  whiteSpace: "nowrap",
};

/** One row; a dimmed item's row is drawn dimmed. */
export function listRowStyle(dimmed: boolean): React.CSSProperties {
  return { opacity: dimmed ? 0.45 : 1, cursor: "pointer" };
}

/** One cell. */
export const listCellStyle: React.CSSProperties = {
  padding: "7px 12px",
  borderBottom: `1px solid ${t.borderSubtle}`,
  color: t.text,
  verticalAlign: "top",
};

/** A list field's editor: its items, then the button that adds one (design §4.5 D3). */
export const listEditorStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 8,
};

/** The text shown while a list holds no items. */
export const listEmptyStyle: React.CSSProperties = {
  fontSize: 11.5,
  color: t.textMuted,
};

/** One item of a list. */
export const listItemStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
  padding: 8,
  border: `1px solid ${t.borderSubtle}`,
  borderRadius: t.radius,
  background: t.surfaceRaised,
};

/** An item's header: its numbered title and its remove button. */
export const listItemHeaderStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: 8,
};

/** An item's numbered title, such as `Peer address 1`. */
export const listItemTitleStyle: React.CSSProperties = {
  fontSize: 11.5,
  fontWeight: 600,
  color: t.text,
};

/** An item's remove button, drawn like the inspector's close button. */
export const listRemoveStyle: React.CSSProperties = {
  flex: "none",
  border: "none",
  background: "transparent",
  color: t.textMuted,
  cursor: "pointer",
  fontSize: 12,
  lineHeight: 1,
  padding: 2,
};

/** An object item's properties, one row each. */
export const listPropertyGridStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
};

/** One property of an object item. */
export const listPropertyStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 2,
};

/** A property's label line: its label and, when required, the marker. */
export const listPropertyLabelLineStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "baseline",
};

/** A property's label. */
export const listPropertyLabelStyle: React.CSSProperties = {
  fontSize: 11,
  color: t.textMuted,
};

/** The marker of a required property. */
export const listPropertyRequiredStyle: React.CSSProperties = {
  fontSize: 11,
  fontWeight: 600,
  color: t.danger,
};

/** A property and, beside it, the way to set one of its alternatives instead. */
export const listPropertyRowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 6,
};

/** The input of a property, or of a scalar item. */
export const listPropertyInputStyle: React.CSSProperties = {
  flex: "1 1 auto",
  minWidth: 0,
  boxSizing: "border-box",
  padding: "4px 6px",
  border: `1px solid ${t.inputBorder}`,
  borderRadius: t.radiusSm,
  background: t.inputBg,
  color: t.text,
  fontSize: 12,
};

/** The button that adds an item; drawn neutral-disabled while the list cannot grow. */
export function listAddStyle(canAdd: boolean): React.CSSProperties {
  return {
    alignSelf: "flex-start",
    border: `1px solid ${t.border}`,
    background: "transparent",
    color: canAdd ? t.text : t.textSubtle,
    borderRadius: t.radiusSm,
    padding: "4px 10px",
    fontSize: 11.5,
    cursor: canAdd ? "pointer" : "not-allowed",
    opacity: canAdd ? 1 : 0.62,
  };
}
