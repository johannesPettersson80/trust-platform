// Styles for the Add dialog (design §4.5 "D3: forms and the Add dialog", specification 25 §5.9 and
// §5.14). It shares the inspector's chrome, so its pane sits beside the canvas the way the form pane
// does (specification 25 §5.11). Colours come from the shared roles in `src/webview/theme.ts`; the
// words that carry meaning always come from the view JSON, never from here (design §3.4).

import type React from "react";

import { t, tint } from "../../../webview/theme";

/** The Add dialog pane: the form pane's chrome, so the two right-hand panes look alike (specification 25 §5.11). */
export const addPaneStyle: React.CSSProperties = {
  width: 380,
  flex: "none",
  display: "flex",
  flexDirection: "column",
  overflowY: "auto",
  borderLeft: `1px solid ${t.border}`,
  background: t.surface,
};

/** The dialog itself: its header, then its body. */
export const addDialogStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
};

/** The dialog's header: its title and its close button. */
export const addHeaderStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "16px 16px 10px",
  borderBottom: `1px solid ${t.border}`,
};

/** The dialog's title. */
export const addTitleStyle: React.CSSProperties = {
  flex: "1 1 auto",
  minWidth: 0,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  fontSize: 15,
  fontWeight: 650,
  color: t.text,
};

/** The dialog's close button, drawn like the inspector's. */
export const addCloseStyle: React.CSSProperties = {
  flex: "none",
  border: "none",
  background: "transparent",
  color: t.textMuted,
  cursor: "pointer",
  fontSize: 14,
  lineHeight: 1,
  padding: 4,
};

/** The dialog's body: the target, the discover entry and the groups. */
export const addBodyStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
};

/** The runtime a new connection belongs to. */
export const addTargetStyle: React.CSSProperties = {
  padding: "10px 16px 0",
  fontSize: 12,
  color: t.textMuted,
};

/** The runtimes to choose from, marked at the target. */
export const addChoicesStyle: React.CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  gap: 6,
  padding: "8px 16px 0",
};

/** One runtime to choose from; the target is drawn as selected. */
export function addChoiceStyle(selected: boolean): React.CSSProperties {
  return {
    padding: "2px 8px",
    border: `1px solid ${selected ? t.accent : t.border}`,
    borderRadius: t.pill,
    background: selected ? tint(t.accent, 0.14) : "transparent",
    color: selected ? t.text : t.textMuted,
    fontSize: 11.5,
    fontWeight: selected ? 600 : 500,
  };
}

/** The discover entry and its help. */
export const addDiscoverStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 4,
  padding: "12px 16px",
  borderBottom: `1px solid ${t.border}`,
};

/** The discover button, drawn by its own `enabled`. */
export function addActionButtonStyle(enabled: boolean): React.CSSProperties {
  return {
    alignSelf: "flex-start",
    border: `1px solid ${t.border}`,
    background: "transparent",
    color: enabled ? t.text : t.textSubtle,
    borderRadius: t.radius,
    padding: "6px 12px",
    fontSize: 12,
    fontWeight: 500,
    cursor: enabled ? "pointer" : "not-allowed",
    opacity: enabled ? 1 : 0.62,
    whiteSpace: "nowrap",
  };
}

/** The discover help. */
export const addHelpStyle: React.CSSProperties = {
  fontSize: 11,
  lineHeight: 1.4,
  color: t.textMuted,
};

/** One found entry of the last search. */
export const addFoundEntryStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 4,
  padding: 8,
  border: `1px solid ${t.borderSubtle}`,
  borderRadius: t.radius,
  background: t.surfaceRaised,
};

/** A found entry's title. */
export const addFoundTitleStyle: React.CSSProperties = {
  fontSize: 12,
  fontWeight: 600,
  color: t.text,
};

/** The catalog's notice, drawn in place of the groups (specification 25 §5.14). */
export const addCatalogNoticeStyle: React.CSSProperties = {
  margin: "12px 16px",
  padding: "8px 10px",
  border: `1px solid ${tint(t.warn, 0.5)}`,
  borderRadius: t.radius,
  background: tint(t.warn, 0.1),
  fontSize: 11.5,
  lineHeight: 1.4,
  color: t.text,
};

/** One entry of a group, drawn by its own `enabled`. */
export function addEntryStyle(enabled: boolean): React.CSSProperties {
  return {
    display: "flex",
    flexDirection: "column",
    gap: 3,
    width: "100%",
    boxSizing: "border-box",
    padding: 8,
    border: `1px solid ${t.borderSubtle}`,
    borderRadius: t.radius,
    background: t.surfaceRaised,
    textAlign: "left",
    cursor: enabled ? "pointer" : "not-allowed",
    opacity: enabled ? 1 : 0.62,
  };
}

/** An entry's title, drawn as the view gives it. */
export const addEntryTitleStyle: React.CSSProperties = {
  fontSize: 12,
  fontWeight: 600,
  color: t.text,
};

/** An entry's badge, drawn as the view gives it. */
export const addEntryBadgeStyle: React.CSSProperties = {
  alignSelf: "flex-start",
  padding: "1px 6px",
  border: `1px solid ${t.border}`,
  borderRadius: t.pill,
  fontSize: 10,
  fontWeight: 700,
  letterSpacing: 0.3,
  color: t.textMuted,
};

/** An entry's purpose, drawn as the view gives it. */
export const addEntryPurposeStyle: React.CSSProperties = {
  fontSize: 11,
  lineHeight: 1.4,
  color: t.textMuted,
};

/** Why a disabled entry cannot be chosen, drawn under it. */
export const addEntryReasonStyle: React.CSSProperties = {
  fontSize: 11,
  lineHeight: 1.4,
  color: t.textMuted,
};

/** The toggle that shows or hides the advanced groups. */
export const addAdvancedToggleStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  margin: "12px 16px",
  padding: "6px 10px",
  border: `1px solid ${t.border}`,
  background: "transparent",
  borderRadius: t.radius,
  cursor: "pointer",
};

/** The toggle's own text: `show_label` or `hide_label`. */
export const addAdvancedLabelStyle: React.CSSProperties = {
  flex: "1 1 auto",
  minWidth: 0,
  textAlign: "left",
  fontSize: 12,
  color: t.text,
};

/** The number of hidden entries, drawn as the view gives it. */
export const addAdvancedCountStyle: React.CSSProperties = {
  flex: "none",
  padding: "1px 7px",
  border: `1px solid ${t.border}`,
  borderRadius: t.pill,
  fontSize: 10.5,
  fontWeight: 700,
  color: t.textMuted,
};
