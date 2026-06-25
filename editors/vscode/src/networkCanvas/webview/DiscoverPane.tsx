import React, { useEffect, useState } from "react";
import type { DiscoverCandidate } from "../offlineComm";

// §0.5 Discover pane: goal-first device discovery. Recommended tier (zero input, safe) is checked
// by default; Targeted (needs a host/subnet) and Runtime-only (hardware, confirm) are opt-in. Scan
// origin is explicit — field devices live on the runtime's OT network, not the laptop, so the user
// chooses where the scan runs. Results → Add (opens the prefilled form) or Adopt (a truST runtime).

export interface DiscoverRequestItem {
  protocol: string;
  cidr?: string;
  host?: string;
}
export interface DiscoverRequest {
  origin: string;
  items: DiscoverRequestItem[];
}
export interface DiscoverProgressRow {
  protocol: string;
  label: string;
  status: "scanning" | "done";
  count?: number;
}

interface Row {
  key: string;
  protocol: string;
  label: string;
  note: string;
  input?: "host" | "cidr";
  confirm?: boolean;
}

const RECOMMENDED: Row[] = [
  { key: "ads", protocol: "ads", label: "TwinCAT (ADS)", note: "broadcast" },
  { key: "discovery", protocol: "discovery", label: "truST runtimes", note: "mDNS" },
  { key: "modbus-local", protocol: "modbus_tcp", label: "Modbus", note: "origin's local subnet · connect-only" },
];
const TARGETED: Row[] = [
  // Discovering an external OPC-UA server to READ from is the opcua_client flow (the opcua server/
  // expose flow no longer advertises discover). Label names the thing being found.
  { key: "opcua", protocol: "opcua_client", label: "OPC UA server", note: "at host", input: "host" },
  { key: "mqtt", protocol: "mqtt", label: "MQTT broker", note: "at host", input: "host" },
  { key: "modbus-custom", protocol: "modbus_tcp", label: "Modbus (custom subnet)", note: "", input: "cidr" },
];
const RUNTIME_ONLY: Row[] = [
  { key: "ethercat", protocol: "ethercat", label: "EtherCAT slaves", note: "bus enumeration", confirm: true },
  { key: "gpio", protocol: "gpio", label: "GPIO lines", note: "local pins on the runtime" },
];

export function DiscoverPane({
  origins,
  discoverProtocols,
  scanning,
  progress,
  results,
  onScan,
  onAdd,
  onAdopt,
  onClose,
}: {
  origins: { id: string; label: string }[];
  discoverProtocols: ReadonlySet<string>;
  scanning: boolean;
  progress: DiscoverProgressRow[];
  results: DiscoverCandidate[];
  onScan: (req: DiscoverRequest) => void;
  onAdd: (candidate: DiscoverCandidate) => void;
  onAdopt: (candidate: DiscoverCandidate) => void;
  onClose: () => void;
}) {
  // Honesty: offer ONLY what the backend advertises as discover-capable (comm.schema actions ∋
  // "discover"). EtherCAT/GPIO/etc. surface automatically the moment the runtime advertises them —
  // never a hardcoded list, never a tier that scans nothing.
  const can = (r: Row) => discoverProtocols.has(r.protocol);
  const recommended = RECOMMENDED.filter(can);
  const targeted = TARGETED.filter(can);
  const runtimeOnly = RUNTIME_ONLY.filter(can);
  const knownProtocols = new Set(
    [...RECOMMENDED, ...TARGETED, ...RUNTIME_ONLY].map((r) => r.protocol)
  );
  const extra: Row[] = [...discoverProtocols]
    .filter((p) => !knownProtocols.has(p))
    .sort()
    .map((p) => ({ key: `extra:${p}`, protocol: p, label: p, note: "discoverable", input: "host" }));
  const targetedRows = [...targeted, ...extra];
  const allRows = [...recommended, ...targetedRows, ...runtimeOnly];

  const [origin, setOrigin] = useState(origins[0]?.id ?? "this_host");
  const [checked, setChecked] = useState<ReadonlySet<string>>(
    new Set(recommended.map((r) => r.key))
  );
  const [inputs, setInputs] = useState<Record<string, string>>({});
  const [showTargeted, setShowTargeted] = useState(false);
  const [showRuntime, setShowRuntime] = useState(false);

  // The schema can resolve after this pane mounts; seed the Recommended defaults once known (only
  // while nothing is checked yet, so it never overrides a user's choice).
  const recKeys = recommended.map((r) => r.key).join(",");
  useEffect(() => {
    if (recommended.length > 0) {
      setChecked((prev) => (prev.size === 0 ? new Set(recommended.map((r) => r.key)) : prev));
    }
  }, [recKeys]);

  const toggle = (key: string) =>
    setChecked((prev) => {
      const next = new Set(prev);
      next.has(key) ? next.delete(key) : next.add(key);
      return next;
    });

  const scan = () => {
    const items: DiscoverRequestItem[] = allRows
      .filter((r) => checked.has(r.key))
      .map((r) => ({
        protocol: r.protocol,
        cidr: r.input === "cidr" ? inputs[r.key]?.trim() || undefined : undefined,
        host: r.input === "host" ? inputs[r.key]?.trim() || undefined : undefined,
      }));
    if (items.length > 0) {
      onScan({ origin, items });
    }
  };

  const renderRow = (r: Row) => (
    <div key={r.key} style={ROW}>
      <input type="checkbox" checked={checked.has(r.key)} onChange={() => toggle(r.key)} style={{ flex: "none", marginTop: 2 }} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 12, color: "var(--vscode-foreground, #eef1f5)" }}>
          {r.label}
          {r.confirm && <span title="Can disturb a live bus, so confirm before scanning" style={{ color: "var(--vscode-charts-yellow, #e0b341)", marginLeft: 5 }}>⚠</span>}
        </div>
        {r.note && <div style={{ fontSize: 10, color: "var(--vscode-descriptionForeground, #7f8794)" }}>{r.note}</div>}
        {r.input && checked.has(r.key) && (
          <input
            value={inputs[r.key] ?? ""}
            onChange={(e) => setInputs((p) => ({ ...p, [r.key]: e.target.value }))}
            placeholder={r.input === "host" ? "host:port" : "192.168.1.0/24"}
            style={INPUT}
          />
        )}
      </div>
    </div>
  );

  return (
    <aside style={PANEL} aria-label="Discover devices">
      <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "11px 12px", borderBottom: "1px solid var(--vscode-editorWidget-border, #2a2f3a)" }}>
        <div style={{ flex: 1, fontSize: 12, fontWeight: 700, color: "var(--vscode-foreground, #cfd6e0)" }}>Discover</div>
        <button onClick={onClose} aria-label="Close" style={ICON}>✕</button>
      </div>

      <div style={{ flex: 1, overflow: "auto" }}>
        <div style={{ padding: "10px 12px", borderBottom: "1px solid var(--vscode-editorWidget-border, #232833)" }}>
          <label style={LABEL}>Scan from</label>
          <select value={origin} onChange={(e) => setOrigin(e.target.value)} style={INPUT}>
            {origins.map((o) => (
              <option key={o.id} value={o.id}>{o.label}</option>
            ))}
          </select>
          <p style={{ fontSize: 10, color: "var(--vscode-descriptionForeground, #7f8794)", margin: "4px 0 0" }}>
            Field devices live on the runtime's network, so scan from the runtime to find them.
          </p>
        </div>

        <div style={{ padding: "8px 12px" }}>
          {allRows.length === 0 && (
            <p style={{ fontSize: 11, color: "var(--vscode-descriptionForeground, #7f8794)", margin: "4px 2px", lineHeight: 1.5 }}>
              No discovery is available yet. Load a project or connect a runtime that advertises
              discoverable protocols.
            </p>
          )}

          {recommended.length > 0 && <div style={SECTION}>Recommended</div>}
          {recommended.map(renderRow)}

          {targetedRows.length > 0 && (
            <button style={TOGGLE} onClick={() => setShowTargeted((v) => !v)}>{showTargeted ? "▾" : "▸"} Targeted (needs a host/subnet)</button>
          )}
          {showTargeted && targetedRows.map(renderRow)}

          {runtimeOnly.length > 0 && (
            <button style={TOGGLE} onClick={() => setShowRuntime((v) => !v)}>{showRuntime ? "▾" : "▸"} Runtime-only ⚠ (hardware)</button>
          )}
          {showRuntime && runtimeOnly.map(renderRow)}
        </div>

        {(scanning || progress.length > 0 || results.length > 0) && (
          <div style={{ borderTop: "1px solid var(--vscode-editorWidget-border, #2a2f3a)", padding: "8px 12px" }}>
            {progress.map((p) => (
              <div key={p.protocol + p.label} style={{ fontSize: 10.5, color: "var(--vscode-descriptionForeground, #9aa6b6)", lineHeight: 1.5 }}>
                {p.label} … {p.status === "scanning" ? "scanning" : `${p.count ?? 0} found`}
              </div>
            ))}
            {!scanning && progress.length > 0 && results.length === 0 && (
              <p style={{ fontSize: 11, color: "var(--vscode-descriptionForeground, #7f8794)", marginTop: 6 }}>
                Nothing found. (Discovery needs a runtime that serves it; try scanning from the runtime.)
              </p>
            )}
            {results.map((c) => {
              const isRuntime = typeof c.params.control_endpoint === "string";
              return (
                <div key={c.id} style={CARD}>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontSize: 12, color: "var(--vscode-foreground, #eef1f5)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{c.label}</div>
                    <div style={{ fontSize: 10, color: "var(--vscode-descriptionForeground, #7f8794)" }}>{c.protocol} · {c.source}</div>
                  </div>
                  <button onClick={() => (isRuntime ? onAdopt(c) : onAdd(c))} style={ADDBTN}>
                    {isRuntime ? "Adopt" : "+ Add"}
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>

      <div style={{ display: "flex", gap: 8, padding: 12, borderTop: "1px solid var(--vscode-editorWidget-border, #2a2f3a)" }}>
        <button onClick={scan} disabled={scanning || checked.size === 0} style={{ ...PRIMARY, flex: 1, opacity: scanning || checked.size === 0 ? 0.5 : 1 }}>
          {scanning ? "Scanning…" : "Scan"}
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
  width: 290,
  background: "var(--vscode-editorHoverWidget-background, rgba(16,19,26,.97))",
  borderLeft: "1px solid var(--vscode-editorWidget-border, #2a2f3a)",
  zIndex: 7,
  display: "flex",
  flexDirection: "column",
};
const ROW: React.CSSProperties = { display: "flex", alignItems: "flex-start", gap: 8, padding: "6px 2px" };
const SECTION: React.CSSProperties = { fontSize: 11, fontWeight: 600, color: "var(--vscode-disabledForeground, #6a7280)", letterSpacing: 0.2, margin: "2px 0 4px" };
const TOGGLE: React.CSSProperties = { display: "block", width: "100%", textAlign: "left", border: "none", background: "transparent", color: "var(--vscode-descriptionForeground, #9aa6b6)", fontSize: 11, cursor: "pointer", padding: "8px 2px 4px" };
const LABEL: React.CSSProperties = { display: "block", fontSize: 11, color: "var(--vscode-foreground, #cfd6e0)", marginBottom: 4, fontWeight: 600 };
const INPUT: React.CSSProperties = { width: "100%", background: "var(--vscode-input-background, #10141b)", border: "1px solid var(--vscode-input-border, #343b47)", borderRadius: 7, color: "var(--vscode-foreground, #eef1f5)", padding: "5px 8px", fontSize: 11.5, marginTop: 4 };
const CARD: React.CSSProperties = { display: "flex", alignItems: "center", gap: 8, padding: "8px 9px", marginTop: 6, borderRadius: 8, border: "1px solid var(--vscode-editorWidget-border, #2a2f3a)", background: "var(--vscode-editorWidget-background, rgba(29,33,42,.7))" };
const ADDBTN: React.CSSProperties = { flex: "none", border: "1px solid var(--vscode-focusBorder, #2f81f7)", background: "rgba(47,129,247,.16)", color: "var(--vscode-foreground, #cfe0ff)", borderRadius: 6, padding: "4px 9px", fontSize: 11, cursor: "pointer" };
const PRIMARY: React.CSSProperties = { border: "1px solid var(--vscode-focusBorder, #2f81f7)", background: "var(--vscode-focusBorder, #2f81f7)", color: "var(--vscode-button-foreground, #fff)", borderRadius: 7, padding: "8px 13px", fontSize: 12, fontWeight: 650, cursor: "pointer" };
const ICON: React.CSSProperties = { border: "none", background: "transparent", color: "var(--vscode-descriptionForeground, #949cab)", fontSize: 14, cursor: "pointer", padding: 0 };
