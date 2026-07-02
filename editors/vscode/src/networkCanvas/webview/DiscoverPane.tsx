import React, { useEffect, useState } from "react";
import type { DiscoverCandidate } from "../offlineComm";
import { protocolName } from "./protocolMeta";

// §0.5 Discover pane: goal-first device discovery. Recommended tier (zero input, safe) is checked
// by default; targeted scans and runtime hardware scans are opt-in. Scan origin is explicit because
// the computer, a runtime, and a remote host can see different networks. Results → Add (opens the
// prefilled form) or Adopt (a truST runtime).

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
  { key: "ads", protocol: "ads", label: "TwinCAT (ADS)", note: "network broadcast" },
  { key: "discovery", protocol: "discovery", label: "truST runtimes", note: "mDNS" },
  { key: "modbus-local", protocol: "modbus_tcp", label: "Modbus", note: "local network scan" },
];
const TARGETED: Row[] = [
  // Discovering an external OPC-UA server to READ from is the opcua_client flow (the opcua server/
  // expose flow no longer advertises discover). Label names the thing being found.
  { key: "opcua", protocol: "opcua_client", label: "OPC UA server", note: "at host", input: "host" },
  { key: "mqtt", protocol: "mqtt", label: "MQTT broker", note: "at host", input: "host" },
  { key: "modbus-host", protocol: "modbus_tcp", label: "Modbus device", note: "at host", input: "host" },
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
    .map((p) => ({ key: `extra:${p}`, protocol: p, label: protocolName(p), note: "discoverable", input: "host" }));
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
        <div style={{ fontSize: 12, color: "var(--trust-text)" }}>
          {r.label}
          {r.confirm && <span title="Can disturb a live bus, so confirm before scanning" style={{ color: "var(--trust-warn)", marginLeft: 5 }}>⚠</span>}
        </div>
        {r.note && <div style={{ fontSize: 10, color: "var(--trust-text-muted)" }}>{r.note}</div>}
        {r.input && checked.has(r.key) && (
          <input
            value={inputs[r.key] ?? ""}
            onChange={(e) => setInputs((p) => ({ ...p, [r.key]: e.target.value }))}
            placeholder={r.input === "host" ? "host:port" : "192.168.1.0/24"}
            className="trust-input"
            style={{ marginTop: 4 }}
          />
        )}
      </div>
    </div>
  );

  return (
    <aside className="trust-inspector" style={PANEL} aria-label="Discover devices">
      <div className="trust-inspector__header">
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="trust-inspector__eyebrow">Devices & Connections</div>
          <div className="trust-inspector__title">Discover</div>
        </div>
        <button onClick={onClose} aria-label="Close" className="trust-button" style={ICON}>✕</button>
      </div>

      <div style={{ flex: 1, overflow: "auto" }}>
        <div className="trust-section">
          <label style={LABEL}>Scan from</label>
          <select value={origin} onChange={(e) => setOrigin(e.target.value)} className="trust-input" style={{ marginTop: 4 }}>
            {origins.map((o) => (
              <option key={o.id} value={o.id}>{o.label}</option>
            ))}
          </select>
          <p className="trust-help" style={{ marginTop: 6 }}>
            Choose the computer or runtime that can see the target network.
          </p>
        </div>

        <div className="trust-section">
          {allRows.length === 0 && (
            <p className="trust-help" style={{ margin: "4px 2px" }}>
              No discovery is available yet. Load a project or connect a runtime that advertises
              discoverable protocols.
            </p>
          )}

          {recommended.length > 0 && <div style={SECTION}>Recommended</div>}
          {recommended.map(renderRow)}

          {targetedRows.length > 0 && (
            <button style={TOGGLE} onClick={() => setShowTargeted((v) => !v)}>{showTargeted ? "▾" : "▸"} Known address or subnet</button>
          )}
          {showTargeted && targetedRows.map(renderRow)}

          {runtimeOnly.length > 0 && (
            <button style={TOGGLE} onClick={() => setShowRuntime((v) => !v)}>{showRuntime ? "▾" : "▸"} Runtime hardware scans ⚠</button>
          )}
          {showRuntime && runtimeOnly.map(renderRow)}
        </div>

        {(scanning || progress.length > 0 || results.length > 0) && (
          <div className="trust-section">
            {progress.map((p) => (
              <div key={p.protocol + p.label} style={{ fontSize: 10.5, color: "var(--trust-text-muted)", lineHeight: 1.5 }}>
                {p.label} … {p.status === "scanning" ? "scanning" : `${p.count ?? 0} found`}
              </div>
            ))}
            {!scanning && progress.length > 0 && results.length === 0 && (
              <p className="trust-help" style={{ marginTop: 6 }}>
                Nothing found. (Discovery needs a runtime that serves it; try scanning from the runtime.)
              </p>
            )}
            {results.map((c) => {
              const isRuntime = typeof c.params.control_endpoint === "string";
              const endpoint = typeof c.params.control_endpoint === "string" ? c.params.control_endpoint : "";
              const host = typeof c.params.host === "string" ? c.params.host : "";
              const displayEndpoint = formatDiscoveredEndpoint(endpoint);
              const detail = isRuntime
                ? runtimeDiscoveryDetail(host, displayEndpoint)
                : [protocolName(c.protocol), c.source, c.confidence].filter(Boolean).join(" · ");
              return (
                <div key={c.id} style={CARD}>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontSize: 12, color: "var(--trust-text)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{c.label}</div>
                    <div style={{ fontSize: 10, color: "var(--trust-text-muted)", lineHeight: 1.25, overflowWrap: "anywhere" }}>{detail}</div>
                  </div>
                  <button onClick={() => (isRuntime ? onAdopt(c) : onAdd(c))} className="trust-button">
                    {isRuntime ? "Adopt" : "+ Add"}
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>

      <div className="trust-section" style={{ display: "flex", gap: 8 }}>
        <button onClick={scan} disabled={scanning || checked.size === 0} className="trust-button trust-button--primary" style={{ flex: 1 }}>
          {scanning ? "Scanning…" : "Scan"}
        </button>
      </div>
    </aside>
  );
}

function formatDiscoveredEndpoint(endpoint: string): string {
  const value = endpoint.trim();
  if (!value) {
    return "";
  }
  if (value.startsWith("tcp://")) {
    return value.slice("tcp://".length);
  }
  return value;
}

function runtimeDiscoveryDetail(host: string, endpoint: string): string {
  const cleanHost = host.trim();
  const cleanEndpoint = endpoint.trim();
  return cleanEndpoint || cleanHost;
}

const PANEL: React.CSSProperties = {
  position: "absolute",
  top: 0,
  right: 0,
  bottom: 0,
  width: 290,
  zIndex: 7,
};
const ROW: React.CSSProperties = { display: "flex", alignItems: "flex-start", gap: 8, padding: "6px 2px" };
const SECTION: React.CSSProperties = { fontSize: 11, fontWeight: 600, color: "var(--trust-text-subtle)", letterSpacing: 0.2, margin: "2px 0 4px" };
const TOGGLE: React.CSSProperties = { display: "block", width: "100%", textAlign: "left", border: "none", background: "transparent", color: "var(--trust-text-muted)", fontSize: 11, cursor: "pointer", padding: "8px 2px 4px" };
const LABEL: React.CSSProperties = { display: "block", fontSize: 11, color: "var(--trust-text)", marginBottom: 4, fontWeight: 600 };
const CARD: React.CSSProperties = { display: "flex", alignItems: "center", gap: 8, padding: "8px 9px", marginTop: 6, borderRadius: "var(--trust-radius-lg)", border: "1px solid var(--trust-border)", background: "var(--trust-surface)" };
const ICON: React.CSSProperties = { minHeight: 24, padding: 0, width: 26 };
