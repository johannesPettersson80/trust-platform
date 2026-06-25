import React, { useState } from "react";

// §0.4 runtime slot: add a second runtime (PLC) to this host. A runtime is its own PROJECT (the
// compiler rejects multiple CONFIGURATIONs in one), so this scaffolds a sibling project via
// `trust-runtime fleet runtime add` and tracks its control endpoint in the fleet view. It appears
// on the canvas once started.
const TEMPLATES = [
  { id: "simulate", label: "Simulator (no hardware)" },
  { id: "empty", label: "Empty (loopback I/O)" },
];

export function AddRuntimePanel({
  post,
  onClose,
}: {
  post: (message: unknown) => void;
  onClose: () => void;
}) {
  const [name, setName] = useState("");
  const [template, setTemplate] = useState("simulate");
  const submit = () => {
    const value = name.trim();
    if (!value) {
      return;
    }
    post({ type: "addRuntime", name: value, template });
    onClose();
  };

  return (
    <aside style={PANEL} aria-label="Add a runtime">
      <div style={{ display: "flex", alignItems: "center", padding: "11px 12px", borderBottom: "1px solid var(--vscode-editorWidget-border, #2a2f3a)" }}>
        <div style={{ flex: 1, fontSize: 12, fontWeight: 700, color: "var(--vscode-foreground, #cfd6e0)" }}>Add a runtime</div>
        <button onClick={onClose} aria-label="Close" style={ICON}>✕</button>
      </div>
      <div style={{ flex: 1, overflow: "auto", padding: 12 }}>
        <p style={{ color: "var(--vscode-descriptionForeground, #9aa6b6)", fontSize: 11, lineHeight: 1.5, margin: "0 0 12px" }}>
          A second runtime (PLC) on this host. It's created as its own project; start it to bring it
          online on the canvas.
        </p>
        <label style={LABEL}>Name</label>
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              submit();
            }
          }}
          placeholder="cell1"
          style={INPUT}
          autoFocus
        />
        <label style={{ ...LABEL, marginTop: 12 }}>Template</label>
        <select value={template} onChange={(e) => setTemplate(e.target.value)} style={INPUT}>
          {TEMPLATES.map((t) => (
            <option key={t.id} value={t.id}>
              {t.label}
            </option>
          ))}
        </select>
      </div>
      <div style={{ display: "flex", gap: 8, padding: 12, borderTop: "1px solid var(--vscode-editorWidget-border, #2a2f3a)" }}>
        <button
          onClick={submit}
          disabled={!name.trim()}
          style={{ ...PRIMARY, flex: 1, opacity: name.trim() ? 1 : 0.5, cursor: name.trim() ? "pointer" : "default" }}
        >
          Add runtime
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
