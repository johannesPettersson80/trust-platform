import type React from "react";

import { t, tint } from "./theme";

export type CanvasDrawerKind = "device" | "setup" | "runtime-scaffold" | "host";

export function activeDrawerWidth(
  hasDraft: boolean,
  hasSelection: boolean,
  hasBrowse: boolean,
  hasDiscover: boolean,
  addKind: CanvasDrawerKind | undefined,
  hasFilter: boolean
): number {
  if (hasDraft) return 360; // AddDevicePanel
  if (hasSelection || hasBrowse) return 340; // NodeInspector / BrowseTagsPanel
  if (hasDiscover) return 340; // DiscoverPane
  if (addKind === "setup") return 252; // SetUpRuntimePanel
  if (addKind === "host") return 300; // AddHostPanel
  if (addKind === "device") return 360; // AddPane
  if (addKind) return 232; // AddRuntimePanel / fallback
  return hasFilter ? 184 : 0; // FilterPanel
}

export function toolbarButtonStyle(
  active: boolean,
  variant: "default" | "primary" = "default",
  disabled = false
): React.CSSProperties {
  return {
    border: `1px solid ${active || variant === "primary" ? t.accent : t.border}`,
    background:
      variant === "primary"
        ? t.accent
        : active
          ? tint(t.accent, 0.14)
          : "transparent",
    color:
      disabled
        ? t.textSubtle
        : variant === "primary"
          ? t.onAccent
          : t.text,
    borderRadius: t.radius,
    padding: "6px 12px",
    fontSize: 12,
    fontWeight: variant === "primary" ? 650 : 500,
    cursor: disabled ? "not-allowed" : "pointer",
    opacity: disabled ? 0.62 : 1,
    whiteSpace: "nowrap",
    transition: `background ${t.ease}, border-color ${t.ease}`,
  };
}

export const issuePillStyle: React.CSSProperties = {
  border: `1px solid ${tint(t.danger, 0.5)}`,
  background: tint(t.danger, 0.12),
  color: t.danger,
  borderRadius: t.radius,
  padding: "6px 10px",
  fontSize: 11,
  fontWeight: 600,
  whiteSpace: "nowrap",
  maxWidth: 360,
  overflow: "hidden",
  textOverflow: "ellipsis",
};
