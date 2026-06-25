import React, { useState } from "react";

// §0.4 host slot: add another machine to the fleet view. A host is client-side fleet membership —
// it writes the runtime's control endpoint to `trust-lsp.runtime.fleetEndpoints` (extension config);
// the canvas then fetches + merges that runtime's topology. (Network discovery is a later slice.)
export function AddHostPanel({
  post,
  onClose,
}: {
  post: (message: unknown) => void;
  onClose: () => void;
}) {
  const [endpoint, setEndpoint] = useState("");
  const submit = () => {
    const value = endpoint.trim();
    if (!value) {
      return;
    }
    post({ type: "addHost", endpoint: value });
    onClose();
  };

  return (
    <aside style={PANEL} aria-label="Add a host">
      <div style={{ display: "flex", alignItems: "center", padding: "11px 12px", borderBottom: "1px solid var(--vscode-editorWidget-border, #2a2f3a)" }}>
        <div style={{ flex: 1, fontSize: 12, fontWeight: 700, color: "var(--vscode-foreground, #cfd6e0)" }}>Add a host</div>
        <button onClick={onClose} aria-label="Close" style={ICON}>✕</button>
      </div>
      <div style={{ flex: 1, overflow: "auto", padding: 12 }}>
        <p style={{ color: "var(--vscode-descriptionForeground, #9aa6b6)", fontSize: 11, lineHeight: 1.5, margin: "0 0 12px" }}>
          Point at another runtime's control endpoint. It joins your fleet view and appears on the
          canvas once it's reachable.
        </p>
        <label style={LABEL}>Control endpoint</label>
        <input
          value={endpoint}
          onChange={(e) => setEndpoint(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              submit();
            }
          }}
          placeholder="10.0.0.5:5510"
          style={INPUT}
          autoFocus
        />
        <p style={{ color: "var(--vscode-descriptionForeground, #7f8794)", fontSize: 10.5, marginTop: 4 }}>
          The host:port (or socket) of the runtime's control endpoint.
        </p>
        <p style={{ color: "var(--vscode-disabledForeground, #6a7280)", fontSize: 10, lineHeight: 1.4, marginTop: 12 }}>
          Discover hosts on the network (coming next).
        </p>
      </div>
      <div style={{ display: "flex", gap: 8, padding: 12, borderTop: "1px solid var(--vscode-editorWidget-border, #2a2f3a)" }}>
        <button
          onClick={submit}
          disabled={!endpoint.trim()}
          style={{ ...PRIMARY, flex: 1, opacity: endpoint.trim() ? 1 : 0.5, cursor: endpoint.trim() ? "pointer" : "default" }}
        >
          Add host
        </button>
      </div>
    </aside>
  );
}

const PANEL: React.CSSProperties = {
  position: "absolute",
  top: 0,
  right: 0,
  bottom: 0,
  width: 232,
  background: "var(--vscode-editorHoverWidget-background, rgba(16,19,26,.97))",
  borderLeft: "1px solid var(--vscode-editorWidget-border, #2a2f3a)",
  zIndex: 7,
  display: "flex",
  flexDirection: "column",
};
const LABEL: React.CSSProperties = { display: "block", fontSize: 11, color: "var(--vscode-foreground, #cfd6e0)", marginBottom: 4, fontWeight: 600 };
const INPUT: React.CSSProperties = {
  width: "100%",
  background: "var(--vscode-input-background, #10141b)",
  border: "1px solid var(--vscode-input-border, #343b47)",
  borderRadius: 7,
  color: "var(--vscode-foreground, #eef1f5)",
  padding: "7px 9px",
  fontSize: 12,
};
const PRIMARY: React.CSSProperties = {
  border: "1px solid var(--vscode-focusBorder, #2f81f7)",
  background: "var(--vscode-focusBorder, #2f81f7)",
  color: "var(--vscode-button-foreground, #fff)",
  borderRadius: 7,
  padding: "8px 13px",
  fontSize: 12,
  fontWeight: 650,
};
const ICON: React.CSSProperties = { border: "none", background: "transparent", color: "var(--vscode-descriptionForeground, #949cab)", fontSize: 14, cursor: "pointer", padding: 0 };
