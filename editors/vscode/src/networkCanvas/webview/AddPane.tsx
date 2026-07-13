import React, { useMemo, useState } from "react";
import type { CommSchemaResponse } from "../../communication/schemaForm";
import { protocolColor } from "./protocolMeta";
import { groupForAddPicker, type AddPickerGroup } from "./grouping";

// S-09 projects the backend protocol catalog into user-intent groups. The schema still owns ids and
// setup fields; the picker owns only first-time-user wording and the beginner/advanced split.
export function AddPane({
  schema,
  target,
  onChoose,
  onDiscover,
  onClose,
}: {
  schema?: CommSchemaResponse;
  target?: { id: string; name: string };
  onChoose: (protocolId: string) => void;
  onDiscover: () => void;
  onClose: () => void;
}) {
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const protocols = useMemo(() => schema?.protocols ?? [], [schema]);
  const groups = useMemo(() => groupForAddPicker(protocols), [protocols]);
  const visibleGroups = groups.filter((group) => !group.advanced || advancedOpen);
  const advancedCount = groups
    .filter((group) => group.advanced)
    .reduce((count, group) => count + group.items.length, 0);

  const renderItem = (item: AddPickerGroup<(typeof protocols)[number]>["items"][number]) => (
    <button
      key={item.protocol.id}
      data-role="add-picker-item"
      data-protocol={item.protocol.id}
      onClick={() => onChoose(item.protocol.id)}
      style={ITEM}
      title={item.purpose}
    >
      <span style={{ ...BADGE, background: protocolColor(item.protocol.id) }}>{item.badge}</span>
      <span style={{ display: "flex", flexDirection: "column", minWidth: 0 }}>
        <span data-role="add-picker-title" style={ITEM_TITLE}>{item.title}</span>
        {item.purpose && (
          <span style={ITEM_PURPOSE}>
            {item.purpose}
          </span>
        )}
      </span>
    </button>
  );

  return (
    <aside className="trust-inspector" style={PANEL} aria-label="Add device or connection">
      <div className="trust-inspector__header">
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="trust-inspector__eyebrow" style={{ whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
            {target?.name ?? "runtime"}
          </div>
          <div className="trust-inspector__title">
            Add device or connection
          </div>
        </div>
        <button onClick={onClose} aria-label="Close" style={iconBtn}>✕</button>
      </div>
      <div className="trust-section">
        <button className="trust-button trust-button--primary" onClick={onDiscover} style={{ width: "100%" }}>
          Discover ADS devices
        </button>
        <p className="trust-help" style={{ marginTop: 6, fontSize: 10.5, lineHeight: 1.3 }}>
          Search this computer and the local network now. Other discovery types
          stay under their collapsed section.
        </p>
      </div>
      <div data-testid="add-picker-list" style={{ flex: 1, overflow: "auto", padding: "8px 10px" }}>
        {schema === undefined ? (
          <p style={EMPTY}>Catalog unavailable (needs a runtime that serves the schema).</p>
        ) : groups.length === 0 ? (
          <p style={EMPTY}>No device or connection types are available.</p>
        ) : (
          <>
          {visibleGroups.map((g) => (
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
          ))}
          {advancedCount > 0 && (
            <section style={SECTION}>
              <button
                data-testid="add-picker-advanced-toggle"
                className="trust-button"
                onClick={() => setAdvancedOpen((open) => !open)}
                aria-expanded={advancedOpen}
                style={ADVANCED_TOGGLE}
              >
                <span>{advancedOpen ? "Hide" : "Show"} advanced integrations</span>
                <span style={SECTION_COUNT}>{advancedCount}</span>
              </button>
            </section>
          )}
          </>
        )}
      </div>
    </aside>
  );
}

const PANEL: React.CSSProperties = {
  position: "absolute",
  top: 0,
  right: 0,
  bottom: 0,
  width: 360,
  zIndex: 7,
};
const ITEM: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "54px 1fr",
  alignItems: "center",
  gap: 9,
  width: "100%",
  textAlign: "left",
  padding: "7px 10px",
  marginBottom: 5,
  borderRadius: "var(--trust-radius)",
  border: "1px solid var(--trust-border)",
  background: "var(--trust-surface)",
  color: "var(--trust-text)",
  cursor: "pointer",
};
const BADGE: React.CSSProperties = {
  boxSizing: "border-box",
  fontSize: 9,
  fontWeight: 800,
  color: "var(--trust-canvas)",
  borderRadius: 4,
  padding: "2px 5px",
  textTransform: "uppercase",
  textAlign: "center",
};
const ITEM_TITLE: React.CSSProperties = { fontSize: 12, color: "var(--trust-text)", fontWeight: 650 };
const ITEM_PURPOSE: React.CSSProperties = { fontSize: 10.5, color: "var(--trust-text-muted)", lineHeight: 1.28, whiteSpace: "normal" };
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
const SECTION_HEADER: React.CSSProperties = { display: "flex", alignItems: "center", gap: 8, padding: "8px 6px 5px 6px" };
const SECTION_LABEL: React.CSSProperties = {
  flex: "none",
  margin: 0,
  whiteSpace: "nowrap",
  textTransform: "none",
  letterSpacing: 0,
};
const SECTION_RULE: React.CSSProperties = { flex: 1, height: 1, background: "var(--trust-border)" };
const SECTION_COUNT: React.CSSProperties = { flex: "none", fontSize: 9, fontWeight: 700, color: "var(--trust-text-subtle)", border: "1px solid var(--trust-border)", borderRadius: 999, padding: "0 6px", minWidth: 14, textAlign: "center", lineHeight: "15px" };
const ADVANCED_TOGGLE: React.CSSProperties = { width: "100%", justifyContent: "space-between", marginTop: 8 };
