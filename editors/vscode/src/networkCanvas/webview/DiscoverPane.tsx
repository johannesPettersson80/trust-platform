import React, { useEffect, useRef, useState } from "react";
import type { DiscoverCandidate } from "../offlineComm";
import type { AdsServiceProbeViewState } from "../adsServiceProbeModel";
import {
  offersAdsManualIdentityRecovery,
  type DiscoveryErrorCode,
} from "../discoveryErrors";
import { candidateDisabledReason } from "../discoverySession";
import {
  AdsDiscoveryComputerCard,
  AdsDiscoveryControls,
} from "./AdsDiscoveryFlow";
import { discoveryConfidenceLabel, discoverySourceLabel } from "./connectorPresentation";
import type {
  DiscoverOrigin,
  DiscoverProgressRow,
  DiscoverRequest,
  DiscoverRequestItem,
  AdsDiscoveryDraft,
} from "./discoverPaneModel";
import {
  DEFAULT_ADS_DISCOVERY_DRAFT,
  adsServiceProbeResultsNeedRecheck,
  adsEmptyIdentityCopy,
  applyAdsEmptyRecovery,
  createAdsDiscoveryScanSnapshot,
  discoveryProgressCopy,
  shouldShowDiscoveryUnavailable,
  shouldShowScanSelected,
  validateAdsDiscoveryDraft,
  type AdsDiscoveryScanSnapshot,
} from "./discoverPaneModel";
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
  { key: "ads", protocol: "ads", label: "TwinCAT", note: "find a TwinCAT computer" },
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
  adsServiceProbes,
  error,
  errorCode,
  sessionCurrent,
  onScan,
  onProbeAdsServices,
  onReset,
  onAdd,
  onAdopt,
  onClose,
}: {
  origins: DiscoverOrigin[];
  discoverProtocols: ReadonlySet<string>;
  scanning: boolean;
  progress: readonly DiscoverProgressRow[];
  results: readonly DiscoverCandidate[];
  adsServiceProbes: Readonly<Record<string, AdsServiceProbeViewState>>;
  error?: string;
  errorCode?: DiscoveryErrorCode;
  sessionCurrent: boolean;
  onScan: (req: DiscoverRequest) => void;
  onProbeAdsServices: (
    candidate: DiscoverCandidate,
    ports: readonly number[],
    origin: string
  ) => void;
  onReset: () => void;
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
  const [adsDraft, setAdsDraft] = useState<AdsDiscoveryDraft>(
    DEFAULT_ADS_DISCOVERY_DRAFT
  );
  const lastAdsProbePortPlans = useRef<Map<string, string>>(new Map());
  const adsScanSnapshot = useRef<AdsDiscoveryScanSnapshot | undefined>(undefined);
  const [scanMode, setScanMode] = useState<"ads" | "selected">("selected");
  const [showTargeted, setShowTargeted] = useState(false);
  const [showRuntime, setShowRuntime] = useState(false);
  const selectedOrigin = origins.find((o) => o.id === origin) ?? origins[0];
  const hasRuntimeOriginReady = origins.some((o) => o.runtimeDiscoveryReady === true);
  const selectedStoppedRuntimeReason =
    selectedOrigin && selectedOrigin.id !== "this_host" && selectedOrigin.runtimeDiscoveryReady !== true
      ? selectedOrigin.runtimeDiscoveryDisabledReason ??
        `Start or connect ${selectedOrigin.label} before scanning from it.`
      : undefined;
  const clearStaleIdentityResults = () => {
    if (!sessionCurrent) {
      return;
    }
    onReset();
    adsScanSnapshot.current = undefined;
    lastAdsProbePortPlans.current.clear();
  };
  const changeAdsDraft = (next: AdsDiscoveryDraft) => {
    const identityChanged =
      next.location !== adsDraft.location ||
      next.host !== adsDraft.host ||
      next.amsNetId !== adsDraft.amsNetId;
    if (identityChanged) {
      clearStaleIdentityResults();
    }
    setAdsDraft(next);
  };
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
  const hasSelectedNonAdsScan = shouldShowScanSelected(
    selectedScanRows.map((row) => row.protocol)
  );
  const adsSelected = checked.has("ads") && can(RECOMMENDED[0]);
  const adsValidation = validateAdsDiscoveryDraft(adsDraft);
  const adsHostError = adsSelected ? adsValidation.hostError : undefined;
  const adsCustomPortError = adsSelected
    ? adsValidation.customPortError
    : undefined;
  const adsAmsNetIdError = adsSelected
    ? adsValidation.amsNetIdError
    : undefined;
  const adsProbeRunning = Object.values(adsServiceProbes).some(
    (probe) => probe.probing
  );
  const discoveryBusy = scanning || adsProbeRunning;
  const scanDisabled =
    discoveryBusy ||
    selectedScanRows.length === 0 ||
    Boolean(adsHostError) ||
    Boolean(adsAmsNetIdError) ||
    Boolean(adsCustomPortError);

  // The schema can resolve after this pane mounts; seed the Recommended defaults once known (only
  // while nothing is checked yet, so it never overrides a user's choice).
  const recKeys = recommended.map((r) => r.key).join(",");
  useEffect(() => {
    if (recommended.length > 0) {
      setChecked((prev) => (prev.size === 0 ? new Set(recommended.map((r) => r.key)) : prev));
    }
  }, [recKeys]);

  useEffect(() => {
    if (scanning) {
      lastAdsProbePortPlans.current.clear();
    }
  }, [scanning]);

  const toggle = (key: string) =>
    setChecked((prev) => {
      const next = new Set(prev);
      next.has(key) ? next.delete(key) : next.add(key);
      return next;
    });

  const startScan = (rows: readonly Row[], mode: "ads" | "selected") => {
    const snapshot = createAdsDiscoveryScanSnapshot(origin, adsDraft);
    adsScanSnapshot.current = snapshot;
    lastAdsProbePortPlans.current.clear();
    const items: DiscoverRequestItem[] = rows
      .filter((r) => !runtimeScanDisabledReason(r))
      .map((r) => {
        if (r.protocol === "ads") {
          return {
            protocol: "ads",
            host: snapshot.host,
            targetAmsNetId: snapshot.targetAmsNetId,
            amsPort: 851,
          };
        }
        return {
          protocol: r.protocol,
          cidr: r.input === "cidr" ? inputs[r.key]?.trim() || undefined : undefined,
          host: r.input === "host" ? inputs[r.key]?.trim() || undefined : undefined,
        };
      });
    if (items.length > 0) {
      setScanMode(mode);
      onScan({
        origin: snapshot.origin,
        originEndpoint: selectedOrigin?.controlEndpoint,
        items,
      });
    }
  };

  const scan = () => startScan(selectedScanRows, "selected");
  const findTwinCat = () => {
    const adsRow = allRows.find((row) => row.protocol === "ads");
    if (adsRow) {
      startScan([adsRow], "ads");
    }
  };
  const findPhase =
    scanMode === "ads" && scanning
      ? "finding"
      : adsProbeRunning
        ? "probing"
        : "idle";
  const adsFindDisabledReason =
    adsHostError ??
    adsAmsNetIdError ??
    adsCustomPortError ??
    runtimeScanDisabledReason(RECOMMENDED[0]);
  const adsEmptyIdentityScan = Boolean(
    !scanning &&
      !error &&
      progress.some(
        (row) =>
          row.protocol === "ads" &&
          row.status === "done" &&
          (row.count ?? 0) === 0
      ) &&
      !results.some((candidate) => candidate.protocol === "ads")
  );
  const adsIdentityRecoveryError = Boolean(
    !scanning &&
      error &&
      offersAdsManualIdentityRecovery(errorCode) &&
      adsScanSnapshot.current &&
      !results.some((candidate) => candidate.protocol === "ads")
  );
  const showAdsIdentityRecovery =
    adsEmptyIdentityScan || adsIdentityRecoveryError;

  const renderRow = (r: Row) => {
    const disabledReason = runtimeScanDisabledReason(r);
    return (
      <div key={r.key} style={{ ...ROW, opacity: disabledReason ? 0.68 : 1 }}>
        <input
          aria-label={`Include ${r.label}`}
          data-role={r.protocol === "ads" ? "ads-discovery-flow" : undefined}
          type="checkbox"
          checked={checked.has(r.key) && !disabledReason}
          disabled={Boolean(disabledReason) || discoveryBusy}
          onChange={() => {
            if (!disabledReason && !discoveryBusy) {
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
          {r.protocol === "ads" && (
            <AdsDiscoveryControls
              checked={checked.has(r.key)}
              draft={adsDraft}
              hostError={adsHostError}
              amsNetIdError={adsAmsNetIdError}
              customPortError={adsCustomPortError}
              disabled={discoveryBusy}
              findPhase={findPhase}
              findDisabledReason={adsFindDisabledReason}
              onFind={findTwinCat}
              onChange={changeAdsDraft}
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
          <label style={LABEL}>Discovery runs from</label>
          <select
            disabled={discoveryBusy}
            value={origin}
            onChange={(event) => {
              if (event.target.value !== origin) {
                clearStaleIdentityResults();
                setOrigin(event.target.value);
              }
            }}
            className="trust-input"
            style={{ marginTop: 4 }}
          >
            {origins.map((o) => (
              <option key={o.id} value={o.id}>{o.label}</option>
            ))}
          </select>
          <p className="trust-help" style={{ marginTop: 6 }}>
            Choose where discovery commands run. TwinCAT location is selected separately below.
          </p>
        </div>

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
                {discoveryProgressCopy(p)}
              </div>
            ))}
            {showAdsIdentityRecovery && adsScanSnapshot.current && (
              <div
                data-role="ads-empty-result"
                data-state={adsIdentityRecoveryError ? "error" : "empty"}
                style={EMPTY_RECOVERY}
              >
                <p className="trust-help" style={{ margin: 0 }}>
                  {adsEmptyIdentityCopy(adsScanSnapshot.current)}
                </p>
                <button
                  type="button"
                  data-role="ads-empty-recovery"
                  className="trust-button"
                  onClick={() =>
                    setAdsDraft((draft) =>
                      adsScanSnapshot.current
                        ? applyAdsEmptyRecovery(draft, adsScanSnapshot.current)
                        : draft
                    )
                  }
                >
                  {adsScanSnapshot.current.location === "known_address"
                    ? adsScanSnapshot.current.targetAmsNetId
                      ? "Review ADS settings"
                      : "Enter AMS Net ID"
                    : "Use a known address"}
                </button>
              </div>
            )}
            {!scanning &&
              !error &&
              !adsEmptyIdentityScan &&
              !progress.some((row) => row.status === "failed") &&
              progress.length > 0 &&
              results.length === 0 && (
              <p className="trust-help" style={{ marginTop: 6 }}>
                {emptyResultCopy(selectedOrigin, hasRuntimeOriginReady)}
              </p>
            )}
            {results.map((c) => {
              const isRuntime = typeof c.params.control_endpoint === "string";
              const endpoint = typeof c.params.control_endpoint === "string" ? c.params.control_endpoint : "";
              const host = typeof c.params.host === "string" ? c.params.host : "";
              const displayEndpoint = formatDiscoveredEndpoint(endpoint);
              const sessionDisabledReason = candidateDisabledReason(
                c.protocol,
                discoverProtocols,
                sessionCurrent,
                !c.originRuntimeId ||
                  origins.some((candidateOrigin) => candidateOrigin.id === c.originRuntimeId)
              );

              if (c.protocol === "ads") {
                const snapshot = adsScanSnapshot.current;
                const currentPorts = snapshot
                  ? createAdsDiscoveryScanSnapshot(snapshot.origin, adsDraft).ports
                  : [];
                const currentPortPlanKey = adsPortPlanKey(currentPorts);
                const previousPortPlanKey = lastAdsProbePortPlans.current.get(c.id);
                const serviceResultsStale = Boolean(
                  snapshot &&
                    adsServiceProbeResultsNeedRecheck(
                      previousPortPlanKey,
                      currentPortPlanKey,
                      adsCustomPortError
                    )
                );
                return (
                  <AdsDiscoveryComputerCard
                    key={c.id}
                    candidate={c}
                    probe={adsServiceProbes[c.id]}
                    servicePorts={currentPorts}
                    discoveryOriginLabel={selectedOrigin?.label ?? "This computer"}
                    disabledReason={sessionDisabledReason ?? adsCustomPortError}
                    serviceResultsStale={serviceResultsStale}
                    onCheckServices={() => {
                      if (!snapshot || adsCustomPortError) {
                        return;
                      }
                      lastAdsProbePortPlans.current.set(c.id, currentPortPlanKey);
                      onProbeAdsServices(c, currentPorts, snapshot.origin);
                    }}
                    onBrowse={(port) =>
                      onAdd({
                        ...c,
                        params: {
                          ...c.params,
                          ams_port: port,
                          ads_port_confirmed: true,
                          ...(c.originRuntimeId
                            ? { discovery_origin_runtime_id: c.originRuntimeId }
                            : {}),
                        },
                      })
                    }
                  />
                );
              }

              const detail = isRuntime
                ? runtimeDiscoveryDetail(host, displayEndpoint)
                : [
                    protocolName(c.protocol),
                    discoverySourceLabel(c.source),
                    discoveryConfidenceLabel(c.confidence),
                  ]
                    .filter(Boolean)
                    .join(" · ");
              return (
                <div key={c.id} style={CARD}>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontSize: 12, color: "var(--trust-text)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{c.label}</div>
                    <div style={{ fontSize: 10, color: "var(--trust-text-muted)", lineHeight: 1.25, overflowWrap: "anywhere" }}>{detail}</div>
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
                    className="trust-button"
                  >
                    {isRuntime ? "Adopt" : "+ Add"}
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {hasSelectedNonAdsScan && (
        <div className="trust-section" style={{ display: "flex", gap: 8 }}>
          <button
            data-role="scan-selected"
            onClick={scan}
            disabled={scanDisabled}
            title={
              adsHostError ??
              adsAmsNetIdError ??
              adsCustomPortError ??
              (selectedScanRows.length === 0
                ? "Select at least one available scan type."
                : undefined)
            }
            className="trust-button"
            style={SCAN_BUTTON}
          >
            {scanning && scanMode === "selected"
              ? `Scanning ${selectedScanRows.length} selected type${selectedScanRows.length === 1 ? "" : "s"}…`
              : `Scan ${selectedScanRows.length} selected type${selectedScanRows.length === 1 ? "" : "s"}`}
          </button>
        </div>
      )}
    </aside>
  );
}

function adsPortPlanKey(ports: readonly number[]): string {
  return ports.join(",");
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
  width: 340,
  zIndex: 7,
};
const ROW: React.CSSProperties = { display: "flex", alignItems: "flex-start", gap: 8, padding: "6px 2px" };
const SECTION: React.CSSProperties = { fontSize: 11, fontWeight: 600, color: "var(--trust-text-subtle)", letterSpacing: 0.2, margin: "2px 0 4px" };
const TOGGLE: React.CSSProperties = { display: "block", width: "100%", textAlign: "left", border: "none", background: "transparent", color: "var(--trust-text-muted)", fontSize: 11, cursor: "pointer", padding: "8px 2px 4px" };
const LABEL: React.CSSProperties = { display: "block", fontSize: 11, color: "var(--trust-text)", marginBottom: 4, fontWeight: 600 };
const CARD: React.CSSProperties = { display: "flex", alignItems: "center", gap: 8, padding: "8px 9px", marginTop: 6, borderRadius: "var(--trust-radius-lg)", border: "1px solid var(--trust-border)", background: "var(--trust-surface)" };
const ICON: React.CSSProperties = { minHeight: 24, padding: 0, width: 26 };
const SCAN_BUTTON: React.CSSProperties = { flex: 1 };
const EMPTY_RECOVERY: React.CSSProperties = { display: "grid", gap: 7, marginTop: 7, padding: 8, border: "1px solid var(--trust-border)", borderRadius: "var(--trust-radius-md)", background: "var(--trust-surface)" };
