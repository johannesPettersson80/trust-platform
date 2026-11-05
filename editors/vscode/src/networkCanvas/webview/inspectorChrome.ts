import type React from "react";

import { t } from "./theme";

/** The right-hand inspector panel that every selected node opens in. */
export const PANEL_STYLE: React.CSSProperties = {
  position: "absolute",
  top: 0,
  right: 0,
  bottom: 0,
  width: 340,
  maxWidth: "92vw",
  background: t.overlay,
  borderLeft: `1px solid ${t.border}`,
  boxShadow: t.shadowOverlay,
  zIndex: 8,
  display: "flex",
  flexDirection: "column",
  overflow: "hidden",
};

/** The inspector header's close button. */
export const iconBtn: React.CSSProperties = {
  border: "none",
  background: "transparent",
  color: t.textMuted,
  fontSize: 14,
  cursor: "pointer",
};
