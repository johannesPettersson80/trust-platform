// Styles for the browse panes: the OPC UA node, EtherCAT channel and exposed-globals browsers, and
// the ADS tag browser with its port cards (design §4.5 "D5: browse panes", specification 25 §5.4 and
// §11.2). The pane shares the add dialog's header and the form's notices, so the right-hand panes
// look alike. Colours come from the shared roles in `src/webview/theme.ts`; every word that carries
// meaning comes from the view JSON, never from here (design §3.4).

import type React from "react";

import { t } from "../../../webview/theme";

/** The browse pane: the form pane's chrome, so the right-hand panes look alike (specification 25 §5.11). */
export const browsePaneStyle: React.CSSProperties = {
  width: 380,
  flex: "none",
  display: "flex",
  flexDirection: "column",
  overflowY: "auto",
  borderLeft: `1px solid ${t.border}`,
  background: t.surface,
};

/** The dot that shows a browse still runs; the view's own wait reason says why in words. */
export const loadingDotStyle: React.CSSProperties = {
  display: "inline-block",
  width: 8,
  height: 8,
  borderRadius: t.pill,
  background: t.accent,
};

/** One tree row; its indentation follows its depth in the tree. */
export function browseRowStyle(depth: number): React.CSSProperties {
  return {
    display: "flex",
    alignItems: "center",
    gap: 8,
    padding: `5px 16px 5px ${16 + depth * 14}px`,
    borderBottom: `1px solid ${t.borderSubtle}`,
    fontSize: 12,
    color: t.text,
  };
}

/** A tree row's name column: the name, with its path under it. */
export const browseNameColumnStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  flex: "1 1 auto",
  minWidth: 0,
};

/** A leaf's name, drawn as the view gives it. */
export const browseNameStyle: React.CSSProperties = {
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  color: t.text,
};

/** A group's name, drawn in bold. */
export const browseGroupNameStyle: React.CSSProperties = {
  ...browseNameStyle,
  fontWeight: 600,
};

/** A row's path, under its name. */
export const browsePathStyle: React.CSSProperties = {
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  fontFamily: t.mono,
  fontSize: 11,
  color: t.textMuted,
};

/** A row's type and access words. */
export const browseMetaStyle: React.CSSProperties = {
  flex: "none",
  fontSize: 11,
  color: t.textMuted,
  whiteSpace: "nowrap",
};

/** A row's tick box. */
export const browseTickStyle: React.CSSProperties = {
  flex: "none",
  margin: 0,
};

/** One ADS port card: its header, its error or route help, its own action and its tags. */
export const portCardStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 8,
  margin: "12px 16px",
  padding: 10,
  border: `1px solid ${t.borderSubtle}`,
  borderRadius: t.radius,
  background: t.surfaceRaised,
};

/** A port card's header: its title and its state text, side by side. */
export const portHeaderStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "baseline",
  justifyContent: "space-between",
  gap: 8,
};

/** A port card's title. */
export const portTitleStyle: React.CSSProperties = {
  fontSize: 12.5,
  fontWeight: 650,
  color: t.text,
};

/** A port card's state text, such as `2 tags` or `Loading…`. */
export const portStateStyle: React.CSSProperties = {
  fontSize: 11.5,
  color: t.textMuted,
};

/** The route help's lines, under its text. */
export const routeLineStyle: React.CSSProperties = {
  fontSize: 11.5,
  lineHeight: 1.4,
  color: t.textMuted,
};

/** The actions a card or a route help offers, two per row. */
export const cardActionsStyle: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
  gap: 8,
};

/** The port suggestions, drawn as chips in a row. */
export const suggestionsStyle: React.CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  gap: 6,
  padding: "8px 16px 0",
};

/** One port suggestion. */
export const suggestionStyle: React.CSSProperties = {
  padding: "2px 8px",
  border: `1px solid ${t.border}`,
  borderRadius: t.pill,
  fontSize: 11.5,
  color: t.textMuted,
};

/** The port help under the suggestions. */
export const portHelpStyle: React.CSSProperties = {
  margin: 0,
  padding: "8px 16px 0",
  fontSize: 11,
  lineHeight: 1.4,
  color: t.textMuted,
};

/** The Add row: the button, its reason and the write toggle. */
export const addRowStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 8,
  padding: "12px 16px",
  borderTop: `1px solid ${t.border}`,
};

/** The write toggle's row: its check box and its label. */
export const writeToggleStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  fontSize: 12,
  color: t.text,
};

/** The write toggle's reason, drawn under it when the toggle cannot be changed. */
export const writeReasonStyle: React.CSSProperties = {
  fontSize: 11,
  lineHeight: 1.4,
  color: t.textMuted,
};

/** The pane's breadcrumb, above its title. */
export const browseBreadcrumbStyle: React.CSSProperties = {
  display: "block",
  fontSize: 10.5,
  color: t.textMuted,
  whiteSpace: "nowrap",
  overflow: "hidden",
  textOverflow: "ellipsis",
};

/** The search box, drawn in the input chrome of the form fields. */
export const browseSearchStyle: React.CSSProperties = {
  boxSizing: "border-box",
  width: "100%",
  margin: "8px 0 0",
  padding: "5px 8px",
  border: `1px solid ${t.inputBorder}`,
  borderRadius: t.radiusSm,
  background: t.inputBg,
  color: t.text,
  fontSize: 12,
};

/** The line that says nothing matched, or that the pane has no tags or symbols. */
export const browseEmptyStyle: React.CSSProperties = {
  margin: 0,
  padding: "8px 16px",
  fontSize: 11.5,
  lineHeight: 1.5,
  color: t.textMuted,
};

/** A group's toggle row: its glyph and its name, indented by depth. */
export function browseGroupButtonStyle(depth: number): React.CSSProperties {
  return {
    display: "flex",
    alignItems: "center",
    gap: 6,
    width: "100%",
    padding: `5px 16px 5px ${16 + depth * 14}px`,
    border: "none",
    background: "transparent",
    color: t.text,
    fontSize: 12,
    fontWeight: 600,
    textAlign: "left",
    cursor: "pointer",
  };
}

/** A port card's header button: its glyph and title, its state at the end. */
export const portHeaderButtonStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  width: "100%",
  padding: "8px 10px",
  border: "none",
  background: t.surfaceRaised,
  color: t.text,
  fontSize: 12,
  fontWeight: 650,
  textAlign: "left",
  cursor: "pointer",
};

/** A port card's body, under its header. */
export const portBodyStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
  padding: "6px 10px",
};

/** The summary line above the ports, such as `3 ADS ports`. */
export const portSummaryStyle: React.CSSProperties = {
  display: "block",
  margin: "8px 16px 0",
  fontSize: 12,
  color: t.text,
};

/** The disclosure of the connection's details and the typed port's toggle. */
export const disclosureStyle: React.CSSProperties = {
  margin: "8px 16px 0",
  fontSize: 11.5,
  color: t.textMuted,
  cursor: "pointer",
};

/** One row of the connection's details: its label and its value. */
export const detailRowStyle: React.CSSProperties = {
  display: "flex",
  gap: 8,
  padding: "2px 16px",
  fontSize: 11.5,
  color: t.text,
};

/** A detail's label, muted. */
export const detailLabelStyle: React.CSSProperties = {
  flex: "none",
  color: t.textMuted,
};

/** A detail's value, in the monospace of the paths. */
export const detailValueStyle: React.CSSProperties = {
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  fontFamily: t.mono,
};

/** The typed port's toggle, drawn like the footer's buttons. */
export const advancedToggleStyle: React.CSSProperties = {
  width: "100%",
  padding: "6px 10px",
  border: `1px solid ${t.border}`,
  borderRadius: t.radius,
  background: "transparent",
  color: t.text,
  fontSize: 12,
  cursor: "pointer",
};

/** The typed port's panel: its field, the suggestions, their help and the browse button. */
export const portInputPanelStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
  padding: "4px 16px 8px",
};

/** The typed port's label, above its field. */
export const portInputLabelStyle: React.CSSProperties = {
  fontSize: 11.5,
  color: t.text,
};

/** A suggested port, drawn as a button; choosing it fills the field. */
export const suggestionButtonStyle: React.CSSProperties = {
  padding: "2px 8px",
  border: `1px solid ${t.border}`,
  borderRadius: t.pill,
  background: "transparent",
  color: t.textMuted,
  fontSize: 11.5,
  cursor: "pointer",
};

/** A button in the footer that closes an ADS pane. */
export const doneButtonStyle: React.CSSProperties = {
  width: "100%",
  padding: "6px 10px",
  border: `1px solid ${t.border}`,
  borderRadius: t.radius,
  background: "transparent",
  color: t.text,
  fontSize: 12,
  cursor: "pointer",
};

