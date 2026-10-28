// Styles for the Discover pane (design §4.5 "D4: Discover pane", specification 25 §2). The pane sits in the
// Add dialog's chrome, so the search looks like the other right-hand panes. Colours come from the shared roles
// in `src/webview/theme.ts`; every word that carries meaning comes from the view JSON, never from here
// (design §3.4).

import type React from "react";

import { t } from "../../../webview/theme";

/** A group of choices or rows, one under the other. */
export const discoverSectionStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
};

/** One choice (an origin or an `Only search` source); a disabled one is drawn muted. */
export function discoverChoiceStyle(enabled: boolean): React.CSSProperties {
  return {
    display: "flex",
    alignItems: "flex-start",
    gap: 8,
    fontSize: 12,
    color: enabled ? t.text : t.textSubtle,
    cursor: enabled ? "pointer" : "not-allowed",
  };
}

/** The reason a choice, a source or a card carries, drawn under it in muted text. */
export const discoverReasonStyle: React.CSSProperties = {
  fontSize: 11,
  lineHeight: 1.4,
  color: t.textMuted,
  overflowWrap: "anywhere",
};

/** A source row: its name, its word and its reason. */
export const discoverRowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  flexWrap: "wrap",
  gap: 8,
  fontSize: 12,
  color: t.text,
};

/** A source row's name, which takes the room the word leaves. */
export const discoverLabelStyle: React.CSSProperties = {
  flex: "1 1 auto",
  minWidth: 0,
};

/** One labelled input of `Known address or subnet`. */
export const discoverFieldStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 4,
  fontSize: 11.5,
  color: t.text,
};

/** The text input of `Known address or subnet`, drawn in the form fields' chrome. */
export const discoverInputStyle: React.CSSProperties = {
  boxSizing: "border-box",
  width: "100%",
  padding: "5px 8px",
  border: `1px solid ${t.inputBorder}`,
  borderRadius: t.radiusSm,
  background: t.inputBg,
  color: t.text,
  fontSize: 12,
};

/** A device card: its title, its addresses, its protocols and identity, and its actions. */
export const discoverCardStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
  padding: 10,
  border: `1px solid ${t.borderSubtle}`,
  borderRadius: t.radius,
  background: t.surfaceRaised,
};

/** A card's title. */
export const discoverTitleStyle: React.CSSProperties = {
  fontSize: 12.5,
  fontWeight: 650,
  color: t.text,
  overflowWrap: "anywhere",
};

/** A card's address, in the monospace of the endpoints; a long one wraps. */
export const discoverAddressStyle: React.CSSProperties = {
  fontFamily: t.mono,
  fontSize: 11.5,
  color: t.text,
  overflowWrap: "anywhere",
};

/** A protocol line of a card: its name, how it was found and its confidence. */
export const discoverProtocolStyle: React.CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  alignItems: "baseline",
  gap: 8,
  fontSize: 11.5,
  color: t.text,
};

/** An identity row of a card: its label, then its value. */
export const discoverIdentityStyle: React.CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  gap: 8,
  fontSize: 11.5,
};

/** An identity row's value, which wraps. */
export const discoverIdentityValueStyle: React.CSSProperties = {
  color: t.text,
  overflowWrap: "anywhere",
};

/** A warning of a card, in the warning role. */
export const discoverWarningStyle: React.CSSProperties = {
  fontSize: 11.5,
  lineHeight: 1.4,
  color: t.warn,
};

/** A card's actions, one under the other. */
export const discoverCardActionsStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
};
