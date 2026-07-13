import React from "react";

import type { NCGraph } from "./types";
import { t, tint } from "./theme";

export function NetworkCanvasOverlays({
  empty,
  banner,
  summary,
  onAction,
}: {
  empty: boolean;
  banner?: NCGraph["banner"];
  summary: string;
  onAction: (action: string) => void;
}) {
  return (
    <>
      {empty && (
        <div
          className="trust-empty-state"
          data-role="canvas-empty-state"
          style={EMPTY_STATE}
        >
          <svg
            width="38"
            height="38"
            viewBox="0 0 24 24"
            fill="none"
            stroke={t.textSubtle}
            strokeWidth={1.4}
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <rect x="3" y="4.5" width="18" height="6" rx="1.5" />
            <rect x="3" y="13.5" width="18" height="6" rx="1.5" />
            <circle cx="6.6" cy="7.5" r="1" fill={t.textSubtle} stroke="none" />
            <circle cx="6.6" cy="16.5" r="1" fill={t.textSubtle} stroke="none" />
          </svg>
          <div style={EMPTY_TITLE}>No devices or runtimes yet</div>
          <div style={EMPTY_DETAIL}>
            Select Discover ADS devices to search this computer and the local
            network. Start the Simulator to show this project here.
          </div>
        </div>
      )}
      {banner && (
        <div
          style={{
            ...BANNER,
            border: `1px solid ${banner.kind === "info" ? t.border : tint(t.danger, 0.5)}`,
          }}
        >
          <span
            style={{
              color: banner.kind === "info" ? t.text : t.danger,
              fontSize: 12,
              fontWeight: 600,
            }}
          >
            {banner.text}
          </span>
          {banner.actions.map((action) => (
            <button
              key={action.action}
              onClick={() => onAction(action.action)}
              style={ACTION}
            >
              {action.label}
            </button>
          ))}
        </div>
      )}
      {summary && <div className="trust-canvas-summary">{summary}</div>}
    </>
  );
}

const EMPTY_STATE: React.CSSProperties = {
  position: "absolute",
  inset: 0,
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  justifyContent: "center",
  gap: 12,
  pointerEvents: "none",
};
const EMPTY_TITLE: React.CSSProperties = {
  fontSize: 13.5,
  fontWeight: 600,
  color: t.textMuted,
};
const EMPTY_DETAIL: React.CSSProperties = {
  fontSize: 12,
  color: t.textSubtle,
  maxWidth: 300,
  textAlign: "center",
};
const BANNER: React.CSSProperties = {
  position: "absolute",
  top: 12,
  left: "50%",
  transform: "translateX(-50%)",
  display: "flex",
  alignItems: "center",
  gap: 12,
  background: t.overlay,
  borderRadius: t.radiusLg,
  padding: "8px 12px",
  boxShadow: t.shadowOverlay,
  zIndex: 6,
};
const ACTION: React.CSSProperties = {
  border: `1px solid ${t.border}`,
  background: "transparent",
  color: t.text,
  borderRadius: t.radiusSm,
  padding: "4px 10px",
  fontSize: 11,
  cursor: "pointer",
};
