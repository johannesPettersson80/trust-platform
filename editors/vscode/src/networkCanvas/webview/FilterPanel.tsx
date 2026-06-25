import React from "react";
import { protocolColor, protocolName } from "./nodes";

// §5: hide/show connections by protocol. A checkbox per protocol present on the canvas.
export function FilterPanel({
  protocols,
  hidden,
  onToggle,
}: {
  protocols: string[];
  hidden: ReadonlySet<string>;
  onToggle: (protocol: string) => void;
}) {
  return (
    <aside style={PANEL} aria-label="Filter connections">
      <div style={{ padding: "11px 12px", borderBottom: "1px solid var(--vscode-editorWidget-border, #2a2f3a)", fontSize: 11, fontWeight: 700, color: "var(--vscode-foreground, #cfd6e0)" }}>
        Show protocols
      </div>
      <div style={{ flex: 1, overflow: "auto", padding: 8 }}>
        {protocols.length === 0 ? (
          <p style={{ color: "var(--vscode-descriptionForeground, #7f8794)", fontSize: 11, padding: "4px 6px" }}>No connections.</p>
        ) : (
          protocols.map((p) => {
            const on = !hidden.has(p);
            return (
              <label key={p} style={ROW}>
                <input type="checkbox" checked={on} onChange={() => onToggle(p)} />
                <span style={{ width: 10, height: 10, borderRadius: 3, background: protocolColor(p), flex: "none" }} />
                <span style={{ fontSize: 12, opacity: on ? 1 : 0.45 }}>{protocolName(p)}</span>
              </label>
            );
          })
        )}
      </div>
    </aside>
  );
}

const PANEL: React.CSSProperties = {
  position: "absolute",
  top: 0,
  left: 0,
  bottom: 0,
  width: 184,
  background: "var(--vscode-editorHoverWidget-background, rgba(16,19,26,.96))",
  borderRight: "1px solid var(--vscode-editorWidget-border, #2a2f3a)",
  zIndex: 7,
  display: "flex",
  flexDirection: "column",
};
const ROW: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  padding: "6px 8px",
  borderRadius: 7,
  cursor: "pointer",
};
