import React, { useMemo, useState } from "react";
import type { CommSchemaResponse } from "../../communication/schemaForm";
import { protocolBadgeLabel, protocolColor } from "./nodes";
import { groupByCategory } from "./grouping";

// §0.4 add: a searchable list of the REAL protocols comm.schema returns (title + its own `purpose`),
// grouped by the schema's own `category` into Field devices / Supervisory services / Peer links so the
// long catalog reads by role, not as one undifferentiated wall. Search filters first, then we group;
// empty groups disappear. The user still just picks a protocol, fills its fields, and names it.
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
  // Group the filtered set by the schema's `category` (Field / Supervisory / Peer). Pure logic lives in
  // ./grouping so it is unit-tested without a DOM.
  const groups = groupByCategory(filtered);

  const renderItem = (p: (typeof filtered)[number]) => (
    <button key={p.id} onClick={() => onChoose(p.id)} style={ITEM} title={p.purpose}>
      <span style={{ ...BADGE, background: protocolColor(p.id) }}>{protocolBadgeLabel(p.id)}</span>
      <span style={{ display: "flex", flexDirection: "column", minWidth: 0 }}>
        <span style={{ fontSize: 12, color: "var(--trust-text)" }}>{p.title}</span>
        {p.purpose && (
          <span style={{ fontSize: 10, color: "var(--trust-text-muted)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
            {p.purpose}
          </span>
        )}
      </span>
    </button>
  );

  return (
    <aside className="trust-inspector" style={PANEL} aria-label="Add to runtime">
      <div className="trust-inspector__header">
        <div className="trust-inspector__title" style={{ flex: 1, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
          Add to {target?.name ?? "runtime"}
        </div>
        <button onClick={onClose} aria-label="Close" style={iconBtn}>✕</button>
      </div>
      <div className="trust-section">
        <input className="trust-input" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search protocols" />
      </div>
      <div style={{ flex: 1, overflow: "auto", padding: 8 }}>
        {schema === undefined ? (
          <p style={EMPTY}>Catalog unavailable (needs a runtime that serves the schema).</p>
        ) : filtered.length === 0 ? (
          <p style={EMPTY}>No matching protocols.</p>
        ) : (
          groups.map((g) => (
            <section key={g.key} role="group" aria-label={g.label} style={SECTION}>
              <div style={SECTION_HEADER}>
                <span className="trust-section__title" style={SECTION_LABEL}>
                  {g.label}
                </span>
                <span style={SECTION_RULE} aria-hidden="true" />
                <span style={SECTION_COUNT}>{g.items.length}</span>
              </div>
              {g.items.map(renderItem)}
            </section>
          ))
        )}
      </div>
      <div style={{ padding: "8px 12px", borderTop: "1px solid var(--trust-border)", color: "var(--trust-text-subtle)", fontSize: 10, lineHeight: 1.4 }}>
        Pick a protocol, then fill its settings and name it.
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
  zIndex: 7,
};
const ITEM: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 9,
  width: "100%",
  textAlign: "left",
  padding: "8px 9px",
  marginBottom: 6,
  borderRadius: "var(--trust-radius)",
  border: "1px solid var(--trust-border)",
  background: "var(--trust-surface)",
  color: "var(--trust-text)",
  cursor: "pointer",
};
const BADGE: React.CSSProperties = {
  flex: "none",
  fontSize: 9,
  fontWeight: 800,
  color: "var(--trust-canvas)",
  borderRadius: 4,
  padding: "2px 5px",
  textTransform: "uppercase",
};
const iconBtn: React.CSSProperties = {
  border: "none",
  background: "transparent",
  color: "var(--trust-text-muted)",
  fontSize: 14,
  cursor: "pointer",
  padding: 0,
};
const EMPTY: React.CSSProperties = { color: "var(--trust-text-muted)", fontSize: 11, padding: "6px 8px", lineHeight: 1.5 };
const SECTION: React.CSSProperties = { marginBottom: 2 };
// Editorial group header: an uppercase role label, a hairline rule that runs to the edge, then a count
// pill — so the catalog reads as three deliberate shelves, not one undifferentiated list.
const SECTION_HEADER: React.CSSProperties = { display: "flex", alignItems: "center", gap: 8, padding: "10px 6px 7px 6px" };
const SECTION_LABEL: React.CSSProperties = { flex: "none", margin: 0, whiteSpace: "nowrap" };
const SECTION_RULE: React.CSSProperties = { flex: 1, height: 1, background: "var(--trust-border)" };
const SECTION_COUNT: React.CSSProperties = { flex: "none", fontSize: 9, fontWeight: 700, color: "var(--trust-text-subtle)", border: "1px solid var(--trust-border)", borderRadius: 999, padding: "0 6px", minWidth: 14, textAlign: "center", lineHeight: "15px" };
