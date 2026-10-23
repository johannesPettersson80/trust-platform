import React, { useEffect, useState } from "react";
import type { DiscoverCandidate } from "../offlineComm";
import { respondingAdsPorts } from "../adsDiscoveryPorts";
import { candidateDisabledReason } from "../discoverySession";
import {
  adsTargetNetId,
} from "./adsTargetPort";
import { discoveryConfidenceLabel, discoverySourceLabel } from "./connectorPresentation";
import type {
  DiscoverOrigin,
  DiscoverProgressRow,
  DiscoverRequest,
  DiscoverRequestItem,
} from "./discoverPaneModel";
import { shouldShowDiscoveryUnavailable } from "./discoverPaneModel";
import { protocolName } from "./protocolMeta";

// §0.5 Discover pane: goal-first device discovery. Recommended tier (zero input, safe) is checked
// by default; targeted scans and runtime hardware scans are opt-in. Scan origin is explicit because
// the computer, a runtime, and a remote host can see different networks. Results → Add (opens the
// prefilled form) or Adopt (a truST runtime).

interface Row {
  key: string;
  protocol: string;
  label: string;
  note: string;
  input?: "host" | "cidr";
  confirm?: boolean;
}

const RECOMMENDED: Row[] = [
  { key: "ads", protocol: "ads", label: "ADS devices", note: "network broadcast" },
  { key: "discovery", protocol: "discovery", label: "truST runtimes", note: "mDNS" },
  { key: "modbus-local", protocol: "modbus_tcp", label: "Modbus devices", note: "local network scan" },
];
const DISCOVERY_SELECTION_STORAGE_KEY = "trust.discovery.protocols";
const TARGETED: Row[] = [
  { key: "ads-host", protocol: "ads", label: "ADS device", note: "at host", input: "host" },
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
  error,
  sessionCurrent,
  onScan,
  onAdd,
  isOnCanvas,
  onAdopt,
  onOpenAdsPortSettings,
  onClose,
}: {
  origins: DiscoverOrigin[];
  discoverProtocols: ReadonlySet<string>;
  scanning: boolean;
  progress: readonly DiscoverProgressRow[];
  results: readonly DiscoverCandidate[];
  error?: string;
  sessionCurrent: boolean;
  onScan: (req: DiscoverRequest) => void;
  onAdd: (candidate: DiscoverCandidate) => void;
  isOnCanvas: (candidate: DiscoverCandidate) => boolean;
  onAdopt: (candidate: DiscoverCandidate) => void;
  onOpenAdsPortSettings: () => void;
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
    () => restoredProtocolSelection()
  );
  const [inputs, setInputs] = useState<Record<string, string>>({});
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [showTargeted, setShowTargeted] = useState(false);
  const [showRuntime, setShowRuntime] = useState(false);
  const selectedOrigin = origins.find((o) => o.id === origin) ?? origins[0];
  const hasRuntimeOriginReady = origins.some((o) => o.runtimeDiscoveryReady === true);
  const selectedStoppedRuntimeReason =
    selectedOrigin && selectedOrigin.id !== "this_host" && selectedOrigin.runtimeDiscoveryReady !== true
      ? selectedOrigin.runtimeDiscoveryDisabledReason ??
        `Start or connect ${selectedOrigin.label} before scanning from it.`
      : undefined;
  const runtimeScanDisabledReason = (r: Row): string | undefined => {
    if (selectedStoppedRuntimeReason) {
      return selectedStoppedRuntimeReason;
    }
    if (!RUNTIME_ONLY.some((runtimeRow) => runtimeRow.key === r.key)) {
      return undefined;
    }
    if (selectedOrigin?.runtimeDiscoveryReady === true) {
      return undefined;
    }
    return (
      selectedOrigin?.runtimeDiscoveryDisabledReason ??
      "Start or connect a runtime before scanning EtherCAT or GPIO."
    );
  };
  const selectedScanRows = allRows.filter((r) => checked.has(r.key) && !runtimeScanDisabledReason(r));
  const scanDisabled = scanning || selectedScanRows.length === 0;
  const visibleResults = results.filter(
    (candidate) =>
      candidate.protocol !== "ads" ||
      respondingAdsPorts(candidate.params).length > 0
  );

  // The schema can resolve after this pane mounts. ADS is the one first-run default; after the user
  // changes the visible choices, keep that choice for the next discovery session.
  const recKeys = recommended.map((r) => r.key).join(",");
  useEffect(() => {
    if (recommended.length > 0) {
      setChecked((prev) => {
        const availableKeys = new Set(allRows.map((row) => row.key));
        const availableSelection = new Set(
          [...prev].filter((key) => availableKeys.has(key))
        );
        if (availableSelection.size > 0) {
          return availableSelection;
        }
        const firstRunDefault = recommended.some((row) => row.key === "ads")
          ? "ads"
          : recommended[0]?.key;
        return firstRunDefault ? new Set([firstRunDefault]) : availableSelection;
      });
    }
  }, [recKeys]);

  useEffect(() => {
    persistProtocolSelection(checked);
  }, [checked]);

  const toggle = (key: string) =>
    setChecked((prev) => {
      const next = new Set(prev);
      next.has(key) ? next.delete(key) : next.add(key);
      return next;
    });

  const scan = () => {
    const items: DiscoverRequestItem[] = allRows
      .filter((r) => checked.has(r.key) && !runtimeScanDisabledReason(r))
      .map((r) => ({
        protocol: r.protocol,
        cidr: r.input === "cidr" ? inputs[r.key]?.trim() || undefined : undefined,
        host: r.input === "host" ? inputs[r.key]?.trim() || undefined : undefined,
      }));
    if (items.length > 0) {
      onScan({
        origin,
        originEndpoint: selectedOrigin?.controlEndpoint,
        items,
      });
    }
  };

  const renderRow = (r: Row) => {
    const disabledReason = runtimeScanDisabledReason(r);
    return (
      <div key={r.key} style={{ ...ROW, opacity: disabledReason ? 0.68 : 1 }}>
        <input
          type="checkbox"
          checked={checked.has(r.key) && !disabledReason}
          disabled={Boolean(disabledReason)}
          onChange={() => {
            if (!disabledReason) {
              toggle(r.key);
            }
          }}
          title={disabledReason}
          style={{ flex: "none", marginTop: 2 }}
        />
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontSize: 12, color: "var(--trust-text)" }}>
            {r.label}
            {r.confirm && <span title="Can disturb a live bus, so confirm before scanning" style={{ color: "var(--trust-warn)", marginLeft: 5 }}>⚠</span>}
          </div>
          {r.note && <div style={{ fontSize: 10, color: "var(--trust-text-muted)" }}>{r.note}</div>}
          {disabledReason && (
            <div style={{ fontSize: 10, color: "var(--trust-text-muted)", marginTop: 3 }}>
              {disabledReason}
            </div>
          )}
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
  };

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
          {error && (
            <div className="trust-field__message trust-field__message--error">
              {error}
            </div>
          )}
          {shouldShowDiscoveryUnavailable(
            allRows.length,
            scanning,
            progress.length,
            results.length,
            error
          ) && (
            <p className="trust-help" style={{ margin: "4px 2px" }}>
              No discovery is available yet. Load a project or connect a runtime that advertises
              discoverable protocols.
            </p>
          )}
          {allRows.length > 0 && (
            <p className="trust-help" style={{ margin: "4px 2px 8px" }}>
              Choose what to find, then scan. Only the selected device types are searched.
            </p>
          )}
          {recommended.length > 0 && (
            <div>
              <div style={SECTION}>What do you want to find?</div>
              {recommended.map(renderRow)}
            </div>
          )}
          <button
            type="button"
            style={TOGGLE}
            onClick={() => setShowAdvanced((value) => !value)}
          >
            {showAdvanced ? "▾" : "▸"} Advanced scan settings
          </button>
          {showAdvanced && (
            <div style={ADVANCED_BOX}>
              <label style={LABEL}>Scan from</label>
              <select
                value={origin}
                onChange={(event) => setOrigin(event.target.value)}
                className="trust-input"
                style={{ marginTop: 4 }}
              >
                {origins.map((candidateOrigin) => (
                  <option key={candidateOrigin.id} value={candidateOrigin.id}>
                    {candidateOrigin.label}
                  </option>
                ))}
              </select>
              {discoverProtocols.has("ads") && (
                <button
                  type="button"
                  onClick={onOpenAdsPortSettings}
                  className="trust-button"
                  style={{ width: "100%", marginTop: 5 }}
                >
                  Configure ADS ports to scan
                </button>
              )}
              {targetedRows.length > 0 && (
                <button style={TOGGLE} onClick={() => setShowTargeted((value) => !value)}>
                  {showTargeted ? "▾" : "▸"} Known address or subnet
                </button>
              )}
              {showTargeted && targetedRows.map(renderRow)}
              {runtimeOnly.length > 0 && (
                <button style={TOGGLE} onClick={() => setShowRuntime((value) => !value)}>
                  {showRuntime ? "▾" : "▸"} Runtime hardware scans ⚠
                </button>
              )}
              {showRuntime && runtimeOnly.map(renderRow)}
            </div>
          )}
        </div>

        {(scanning || progress.length > 0 || visibleResults.length > 0) && (
          <div className="trust-section">
            {progress.map((p) => (
              <div key={p.protocol + p.label} style={{ fontSize: 10.5, color: "var(--trust-text-muted)", lineHeight: 1.5 }}>
                {p.label} … {p.status === "scanning" ? "scanning" : `${p.count ?? 0} found`}
              </div>
            ))}
            {!scanning && progress.length > 0 && results.length === 0 && (
              <p className="trust-help" style={{ marginTop: 6 }}>
                {emptyResultCopy(selectedOrigin, hasRuntimeOriginReady)}
              </p>
            )}
            {visibleResults.map((c) => {
              const isRuntime = typeof c.params.control_endpoint === "string";
              const isAds = c.protocol === "ads";
              const endpoint = typeof c.params.control_endpoint === "string" ? c.params.control_endpoint : "";
              const host = typeof c.params.host === "string" ? c.params.host : "";
              const displayEndpoint = formatDiscoveredEndpoint(endpoint);
              const respondingPorts = isAds ? respondingAdsPorts(c.params) : [];
              const onCanvas = isAds && isOnCanvas(c);
              const sessionDisabledReason = candidateDisabledReason(
                c.protocol,
                discoverProtocols,
                sessionCurrent,
                !c.originRuntimeId ||
                  origins.some((candidateOrigin) => candidateOrigin.id === c.originRuntimeId)
              );
              const detail = isRuntime
                ? runtimeDiscoveryDetail(host, displayEndpoint)
                : [
                    isAds
                      ? `${respondingPorts.length} responding ADS ${respondingPorts.length === 1 ? "port" : "ports"}`
                      : protocolName(c.protocol),
                  ]
                    .filter(Boolean)
                    .join(" · ");
              return (
                <div key={c.id} style={CARD}>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontSize: 12, color: "var(--trust-text)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{c.label}</div>
                    <div style={{ fontSize: 10, color: "var(--trust-text-muted)", lineHeight: 1.25, overflowWrap: "anywhere" }}>{detail}</div>
                    {onCanvas && (
                      <div style={ON_CANVAS_LABEL}>On canvas</div>
                    )}
                    {isAds && respondingPorts.length > 0 && (
                      <div data-role="responding-ads-ports" style={ADS_RESPONDING_PORTS}>
                        <span style={ADS_RESPONDING_LABEL}>ADS ports</span>
                        <div style={ADS_PORT_BUTTONS}>
                          {respondingPorts.map((port) => (
                            <span
                              key={port}
                              data-ads-port={port}
                              style={ADS_PORT_BUTTON}
                            >
                              {port}
                            </span>
                          ))}
                        </div>
                      </div>
                    )}
                    {!isRuntime && (
                      <details style={{ marginTop: 6 }}>
                        <summary style={DETAILS_SUMMARY}>Details</summary>
                        <div style={DETAILS_BODY}>
                          {isAds && <span>AMS Net ID: {adsTargetNetId(c.params)}</span>}
                          {host && <span>Host: {host}</span>}
                          <span>{discoverySourceLabel(c.source)}</span>
                          <span>{discoveryConfidenceLabel(c.confidence)}</span>
                        </div>
                      </details>
                    )}
                    {sessionDisabledReason && (
                      <div className="trust-field__message trust-field__message--error">
                        {sessionDisabledReason}
                      </div>
                    )}
                  </div>
                  <button
                    onClick={() => {
                      if (sessionDisabledReason) {
                        return;
                      }
                      isRuntime ? onAdopt(c) : onAdd(c);
                    }}
                    disabled={Boolean(sessionDisabledReason)}
                    title={sessionDisabledReason}
                    className={isAds ? "trust-button trust-button--primary" : "trust-button"}
                  >
                    {isRuntime
                      ? "Adopt"
                      : isAds
                        ? onCanvas
                          ? "Manage tags"
                          : "Add to canvas"
                        : "+ Add"}
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>

      <div className="trust-section" style={{ display: "flex", gap: 8 }}>
        <button
          onClick={scan}
          disabled={scanDisabled}
          title={selectedScanRows.length === 0 ? "Select at least one available scan type." : undefined}
          className={scanDisabled ? "trust-button" : "trust-button trust-button--primary"}
          style={SCAN_BUTTON}
        >
          {scanning ? "Scanning…" : "Scan"}
        </button>
      </div>
    </aside>
  );
}

function emptyResultCopy(origin: DiscoverOrigin | undefined, hasRuntimeOriginReady: boolean): string {
  const checks =
    "Nothing found. Check that the device is powered on, on the same network, and not blocked by a port or firewall. Verify the address or subnet for known-address scans.";
  if (origin?.runtimeDiscoveryReady === true) {
    return `${checks} Hardware scans are running from the selected runtime.`;
  }
  if (hasRuntimeOriginReady) {
    return `${checks} For EtherCAT or GPIO, choose a running runtime in Scan from.`;
  }
  return `${checks} For EtherCAT or GPIO, start or connect a runtime first.`;
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
const ADVANCED_BOX: React.CSSProperties = { marginTop: 6, padding: "8px 9px", border: "1px solid var(--trust-border)", borderRadius: "var(--trust-radius-lg)", background: "var(--trust-surface)" };
const CARD: React.CSSProperties = { display: "flex", alignItems: "center", gap: 8, padding: "8px 9px", marginTop: 6, borderRadius: "var(--trust-radius-lg)", border: "1px solid var(--trust-border)", background: "var(--trust-surface)" };
const ICON: React.CSSProperties = { minHeight: 24, padding: 0, width: 26 };
const SCAN_BUTTON: React.CSSProperties = { flex: 1 };
const ADS_RESPONDING_PORTS: React.CSSProperties = {
  marginTop: 7,
};
const ADS_RESPONDING_LABEL: React.CSSProperties = {
  display: "block",
  color: "var(--trust-text-muted)",
  fontSize: 10,
  marginBottom: 4,
};
const ADS_PORT_BUTTONS: React.CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  gap: 4,
};
const ADS_PORT_BUTTON: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  minHeight: 21,
  padding: "1px 7px",
  fontSize: 10.5,
  border: "1px solid var(--trust-border)",
  borderRadius: 999,
  background: "var(--trust-surface-raised)",
  color: "var(--trust-text)",
};
const DETAILS_SUMMARY: React.CSSProperties = { color: "var(--trust-text-muted)", cursor: "pointer", fontSize: 10 };
const DETAILS_BODY: React.CSSProperties = { display: "flex", flexDirection: "column", gap: 2, marginTop: 4, color: "var(--trust-text-muted)", fontSize: 9.5 };
const ON_CANVAS_LABEL: React.CSSProperties = {
  display: "inline-block",
  marginTop: 5,
  color: "var(--trust-success)",
  fontSize: 10,
  fontWeight: 600,
};

function restoredProtocolSelection(): ReadonlySet<string> {
  try {
    const raw = window.localStorage.getItem(DISCOVERY_SELECTION_STORAGE_KEY);
    const value = raw ? JSON.parse(raw) : undefined;
    if (Array.isArray(value)) {
      return new Set(value.filter((item): item is string => typeof item === "string"));
    }
  } catch {
    // Webview storage can be unavailable in isolated test hosts; use the first-run default.
  }
  return new Set(["ads"]);
}

function persistProtocolSelection(selection: ReadonlySet<string>): void {
  try {
    window.localStorage.setItem(
      DISCOVERY_SELECTION_STORAGE_KEY,
      JSON.stringify([...selection])
    );
  } catch {
    // Discovery still works when the host does not provide persistent webview storage.
  }
}
