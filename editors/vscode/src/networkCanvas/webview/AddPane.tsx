import React, { useMemo, useState } from "react";
import type { CommSchemaResponse } from "../../communication/schemaForm";
import { protocolBadgeLabel, protocolColor } from "./nodes";

// §0.4 add: a flat, searchable list of the REAL protocols comm.schema returns (title + its own
// `purpose`). No categories, no archetypes — the user picks a protocol, fills its fields, names it.
export function AddPane({
  schema,
  target,
  onChoose,
  onClose,
}: {
  schema?: CommSchemaResponse;
  target?: { id: string; name: string };
  onChoose: (protocolId: string) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const protocols = useMemo(() => schema?.protocols ?? [], [schema]);
  const q = query.trim().toLowerCase();
  const filtered = protocols.filter(
    (p) =>
      p.title.toLowerCase().includes(q) ||
      p.id.toLowerCase().includes(q) ||
      (p.purpose ?? "").toLowerCase().includes(q)
  );

  return (
    <aside style={PANEL} aria-label="Add to runtime">
      <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "11px 12px", borderBottom: "1px solid var(--vscode-editorWidget-border, #2a2f3a)" }}>
        <div style={{ flex: 1, fontSize: 12, fontWeight: 700, color: "var(--vscode-foreground, #cfd6e0)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
          Add to {target?.name ?? "runtime"}
        </div>
        <button onClick={onClose} aria-label="Close" style={iconBtn}>✕</button>
      </div>
      <div style={{ padding: "9px 12px", borderBottom: "1px solid var(--vscode-editorWidget-border, #2a2f3a)" }}>
        <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search protocols" style={SEARCH} />
      </div>
      <div style={{ flex: 1, overflow: "auto", padding: 8 }}>
        {schema === undefined ? (
          <p style={EMPTY}>Catalog unavailable (needs a runtime that serves the schema).</p>
        ) : filtered.length === 0 ? (
          <p style={EMPTY}>No matching protocols.</p>
        ) : (
          filtered.map((p) => (
            <button key={p.id} onClick={() => onChoose(p.id)} style={ITEM} title={p.purpose}>
              <span style={{ ...BADGE, background: protocolColor(p.id) }}>{protocolBadgeLabel(p.id)}</span>
              <span style={{ display: "flex", flexDirection: "column", minWidth: 0 }}>
                <span style={{ fontSize: 12, color: "var(--vscode-foreground, #eef1f5)" }}>{p.title}</span>
                {p.purpose && (
                  <span style={{ fontSize: 10, color: "var(--vscode-descriptionForeground, #7f8794)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {p.purpose}
                  </span>
                )}
              </span>
            </button>
          ))
        )}
      </div>
      <div style={{ padding: "8px 12px", borderTop: "1px solid var(--vscode-editorWidget-border, #2a2f3a)", color: "var(--vscode-disabledForeground, #6a7280)", fontSize: 10, lineHeight: 1.4 }}>
        Pick a protocol, then fill its settings and name it.
      </div>
    </aside>
  );
}

const PANEL: React.CSSProperties = {
  position: "absolute",
  top: 0,
  left: 0,
  bottom: 0,
  width: 232,
  background: "var(--vscode-editorHoverWidget-background, rgba(16,19,26,.97))",
  borderRight: "1px solid var(--vscode-editorWidget-border, #2a2f3a)",
  zIndex: 7,
  display: "flex",
  flexDirection: "column",
};
const ITEM: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 9,
  width: "100%",
  textAlign: "left",
  padding: "8px 9px",
  marginBottom: 6,
  borderRadius: 8,
  border: "1px solid var(--vscode-editorWidget-border, #2a2f3a)",
  background: "var(--vscode-editorWidget-background, rgba(29,33,42,.7))",
  cursor: "pointer",
};
const BADGE: React.CSSProperties = {
  flex: "none",
  fontSize: 9,
  fontWeight: 800,
  color: "var(--vscode-editor-background, #0c0f14)",
  borderRadius: 4,
  padding: "2px 5px",
  textTransform: "uppercase",
};
const SEARCH: React.CSSProperties = {
  width: "100%",
  background: "var(--vscode-input-background, #10141b)",
  border: "1px solid var(--vscode-input-border, #343b47)",
  borderRadius: 7,
  color: "var(--vscode-foreground, #eef1f5)",
  padding: "6px 9px",
  fontSize: 12,
};
const iconBtn: React.CSSProperties = {
  border: "none",
  background: "transparent",
  color: "var(--vscode-descriptionForeground, #949cab)",
  fontSize: 14,
  cursor: "pointer",
  padding: 0,
};
const EMPTY: React.CSSProperties = { color: "var(--vscode-descriptionForeground, #7f8794)", fontSize: 11, padding: "6px 8px", lineHeight: 1.5 };
