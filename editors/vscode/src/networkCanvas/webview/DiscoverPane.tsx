import React, { useCallback, useEffect, useRef, useState } from "react";
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
  adsEmptyRecoveryFocusRole,
  applyAdsEmptyRecovery,
  createAutomaticAdsDiscoveryItems,
  createAdsDiscoveryScanSnapshot,
  discoveryOriginForMode,
  discoveryProgressCopy,
  shouldShowDiscoveryUnavailable,
  shouldShowScanSelected,
  validateAdsDiscoveryDraft,
  type AdsDiscoveryScanSnapshot,
} from "./discoverPaneModel";
import { protocolName } from "./protocolMeta";

// §0.5 Discover pane: goal-first device discovery. ADS is the zero-input default; targeted inputs,
// scan origin, and runtime hardware scans are progressive disclosure. Results → Add (opens the
// prefilled form) or Adopt (a truST runtime).

interface Row {
  key: string;
  protocol: string;
  label: string;
  note: string;
  input?: "host" | "cidr";
  confirm?: boolean;
}

const ADS: Row = {
  key: "ads",
  protocol: "ads",
  label: "ADS devices",
  note: "this computer and local network",
};
const OTHER_AUTOMATIC: Row[] = [
  { key: "discovery", protocol: "discovery", label: "truST runtimes", note: "mDNS" },
  { key: "modbus-local", protocol: "modbus_tcp", label: "Modbus", note: "local network scan" },
];
const OTHER_KNOWN_ADDRESS: Row[] = [
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
  autoStartAds,
  origins,
  discoverProtocols,
  scanning,
  progress,
  results,
  adsServiceProbes,
  warning,
  warningDetails,
  error,
  errorDetails,
  errorCode,
  sessionCurrent,
  onScan,
  onProbeAdsServices,
  onReset,
  onAdd,
  onAdopt,
  onClose,
}: {
  autoStartAds: boolean;
  origins: DiscoverOrigin[];
  discoverProtocols: ReadonlySet<string>;
  scanning: boolean;
  progress: readonly DiscoverProgressRow[];
  results: readonly DiscoverCandidate[];
  adsServiceProbes: Readonly<Record<string, AdsServiceProbeViewState>>;
  warning?: string;
  warningDetails?: readonly string[];
  error?: string;
  errorDetails?: readonly string[];
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
  const adsRows = can(ADS) ? [ADS] : [];
  const otherAutomatic = OTHER_AUTOMATIC.filter(can);
  const otherKnownAddress = OTHER_KNOWN_ADDRESS.filter(can);
  const runtimeOnly = RUNTIME_ONLY.filter(can);
  const knownProtocols = new Set(
    [ADS, ...OTHER_AUTOMATIC, ...OTHER_KNOWN_ADDRESS, ...RUNTIME_ONLY].map(
      (r) => r.protocol
    )
  );
  const extra: Row[] = [...discoverProtocols]
    .filter((p) => !knownProtocols.has(p))
    .sort()
    .map((p) => ({ key: `extra:${p}`, protocol: p, label: protocolName(p), note: "discoverable", input: "host" }));
  const otherKnownAddressRows = [...otherKnownAddress, ...extra];
  const otherDiscoveryRows = [
    ...otherAutomatic,
    ...otherKnownAddressRows,
    ...runtimeOnly,
  ];
  const allRows = [...adsRows, ...otherDiscoveryRows];

  const [hardwareOrigin, setHardwareOrigin] = useState(
    origins[0]?.id ?? "this_host"
  );
  const [checked, setChecked] = useState<ReadonlySet<string>>(
    new Set(adsRows.map((r) => r.key))
  );
  const [inputs, setInputs] = useState<Record<string, string>>({});
  const [adsDraft, setAdsDraft] = useState<AdsDiscoveryDraft>(
    DEFAULT_ADS_DISCOVERY_DRAFT
  );
  const [adsRecoveryFocusRole, setAdsRecoveryFocusRole] = useState<
    "ads-host" | "ads-ams-net-id" | "ads-custom-ports" | undefined
  >(undefined);
  const lastAdsProbePortPlans = useRef<Map<string, string>>(new Map());
  const autoAdsProbeCandidates = useRef<Set<string>>(new Set());
  const autoAdsStartConsumed = useRef(false);
  const adsScanSnapshot = useRef<AdsDiscoveryScanSnapshot | undefined>(undefined);
  const [scanMode, setScanMode] = useState<"ads" | "selected">("selected");
  const [showOtherDiscoveryTypes, setShowOtherDiscoveryTypes] = useState(false);
  const selectedHardwareOrigin =
    origins.find((o) => o.id === hardwareOrigin) ?? origins[0];
  const hasRuntimeOriginReady = origins.some((o) => o.runtimeDiscoveryReady === true);
  const selectedStoppedRuntimeReason =
    selectedHardwareOrigin &&
    selectedHardwareOrigin.id !== "this_host" &&
    selectedHardwareOrigin.runtimeDiscoveryReady !== true
      ? selectedHardwareOrigin.runtimeDiscoveryDisabledReason ??
        `Start or connect ${selectedHardwareOrigin.label} before scanning from it.`
      : undefined;
  const clearStaleIdentityResults = () => {
    if (!sessionCurrent) {
      return;
    }
    onReset();
    adsScanSnapshot.current = undefined;
    lastAdsProbePortPlans.current.clear();
    autoAdsProbeCandidates.current.clear();
  };
  const changeAdsDraft = (next: AdsDiscoveryDraft) => {
    const identityChanged =
      next.host !== adsDraft.host ||
      next.amsNetId !== adsDraft.amsNetId;
    if (identityChanged) {
      clearStaleIdentityResults();
    }
    setAdsDraft(next);
  };
  const runtimeScanDisabledReason = useCallback((r: Row): string | undefined => {
    if (r.protocol === "ads") {
      return undefined;
    }
    if (selectedStoppedRuntimeReason) {
      return selectedStoppedRuntimeReason;
    }
    if (!RUNTIME_ONLY.some((runtimeRow) => runtimeRow.key === r.key)) {
      return undefined;
    }
    if (selectedHardwareOrigin?.runtimeDiscoveryReady === true) {
      return undefined;
    }
    return (
      selectedHardwareOrigin?.runtimeDiscoveryDisabledReason ??
      "Start or connect a runtime before scanning EtherCAT or GPIO."
    );
  }, [selectedHardwareOrigin, selectedStoppedRuntimeReason]);
  const selectedScanRows = allRows.filter((r) => checked.has(r.key) && !runtimeScanDisabledReason(r));
  const selectedNonAdsScanRows = selectedScanRows.filter(
    (row) => row.protocol !== "ads"
  );
  const hasSelectedNonAdsScan = shouldShowScanSelected(
    selectedNonAdsScanRows.map((row) => row.protocol)
  );
  const adsSelected = can(ADS);
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
    selectedNonAdsScanRows.length === 0;

  // The schema can resolve after this pane mounts; seed ADS once known (only while nothing is
  // checked yet, so it never overrides a user's choice).
  const adsKeys = adsRows.map((r) => r.key).join(",");
  useEffect(() => {
    if (adsRows.length > 0) {
      setChecked((prev) =>
        prev.size === 0 ? new Set(adsRows.map((row) => row.key)) : prev
      );
    }
  }, [adsKeys]);

  useEffect(() => {
    if (scanning) {
      lastAdsProbePortPlans.current.clear();
      autoAdsProbeCandidates.current.clear();
    }
  }, [scanning]);

  useEffect(() => {
    const snapshot = adsScanSnapshot.current;
    if (
      scanning ||
      adsProbeRunning ||
      !sessionCurrent ||
      !snapshot ||
      adsCustomPortError
    ) {
      return;
    }
    const candidate = results.find(
      (result) =>
        result.protocol === "ads" &&
        adsServiceProbes[result.id] === undefined &&
        !autoAdsProbeCandidates.current.has(result.id) &&
        (!result.originRuntimeId ||
          origins.some((candidateOrigin) => candidateOrigin.id === result.originRuntimeId))
    );
    if (!candidate) {
      return;
    }
    autoAdsProbeCandidates.current.add(candidate.id);
    lastAdsProbePortPlans.current.set(candidate.id, adsPortPlanKey(snapshot.ports));
    onProbeAdsServices(candidate, snapshot.ports, snapshot.origin);
  }, [
    adsCustomPortError,
    adsProbeRunning,
    adsServiceProbes,
    origins,
    onProbeAdsServices,
    results,
    scanning,
    sessionCurrent,
  ]);

  const toggle = (key: string) =>
    setChecked((prev) => {
      const next = new Set(prev);
      next.has(key) ? next.delete(key) : next.add(key);
      return next;
    });

  const startScan = useCallback((rows: readonly Row[], mode: "ads" | "selected") => {
    const scanOrigin = discoveryOriginForMode(mode, hardwareOrigin);
    const snapshot = createAdsDiscoveryScanSnapshot(scanOrigin, adsDraft);
    adsScanSnapshot.current = snapshot;
    lastAdsProbePortPlans.current.clear();
    const items: DiscoverRequestItem[] = rows
      .filter((r) => !runtimeScanDisabledReason(r))
      .flatMap((r): readonly DiscoverRequestItem[] => {
        if (r.protocol === "ads") {
          return createAutomaticAdsDiscoveryItems(snapshot);
        }
        return [{
          protocol: r.protocol,
          cidr: r.input === "cidr" ? inputs[r.key]?.trim() || undefined : undefined,
          host: r.input === "host" ? inputs[r.key]?.trim() || undefined : undefined,
        }];
      });
    if (items.length > 0) {
      setScanMode(mode);
      onScan({
        origin: snapshot.origin,
        originEndpoint:
          scanOrigin === "this_host"
            ? undefined
            : selectedHardwareOrigin?.controlEndpoint,
        items,
      });
    }
  }, [
    adsDraft,
    hardwareOrigin,
    inputs,
    onScan,
    runtimeScanDisabledReason,
    selectedHardwareOrigin,
  ]);

  const scan = () => startScan(selectedNonAdsScanRows, "selected");
  const discoverAds = useCallback(() => {
    if (adsSelected) {
      startScan([ADS], "ads");
    }
  }, [adsSelected, startScan]);
  const findPhase =
    scanMode === "ads" && scanning
      ? "finding"
      : adsProbeRunning
        ? "probing"
        : "idle";
  const adsFindDisabledReason =
    adsHostError ??
    adsAmsNetIdError ??
    adsCustomPortError;
  useEffect(() => {
    if (
      !autoStartAds ||
      autoAdsStartConsumed.current ||
      discoveryBusy ||
      adsFindDisabledReason ||
      !adsSelected
    ) {
      return;
    }
    // Opening Discover is the user's scan action. Consume it before starting
    // so React re-renders cannot launch a second ADS request.
    autoAdsStartConsumed.current = true;
    discoverAds();
  }, [
    adsFindDisabledReason,
    autoStartAds,
    discoverAds,
    discoveryBusy,
    adsSelected,
  ]);

  useEffect(() => {
    if (!adsDraft.advanced || !adsRecoveryFocusRole) {
      return;
    }
    const frame = window.requestAnimationFrame(() => {
      const input = document.querySelector<HTMLInputElement>(
        `[data-role="${adsRecoveryFocusRole}"]`
      );
      if (!input) {
        return;
      }
      input.scrollIntoView({ block: "center" });
      input.focus();
      setAdsRecoveryFocusRole(undefined);
    });
    return () => window.cancelAnimationFrame(frame);
  }, [adsDraft.advanced, adsRecoveryFocusRole]);
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
  const openAdsIdentityRecovery = () => {
    const snapshot = adsScanSnapshot.current;
    if (!snapshot) {
      return;
    }
    setAdsRecoveryFocusRole(
      adsEmptyRecoveryFocusRole(snapshot, {
        hostError: adsHostError,
        amsNetIdError: adsAmsNetIdError,
        customPortError: adsCustomPortError,
      })
    );
    setAdsDraft((draft) => applyAdsEmptyRecovery(draft, snapshot));
  };

  const renderRow = (r: Row) => {
    const disabledReason = runtimeScanDisabledReason(r);
    const isAds = r.protocol === "ads";
    return (
      <div
        key={r.key}
        data-role={isAds ? "ads-discovery-section" : undefined}
        style={{ ...ROW, opacity: disabledReason ? 0.68 : 1 }}
      >
        {!isAds && (
          <input
            aria-label={`Include ${r.label}`}
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
        )}
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
              draft={adsDraft}
              hostError={adsHostError}
              amsNetIdError={adsAmsNetIdError}
              customPortError={adsCustomPortError}
              disabled={discoveryBusy}
              findPhase={findPhase}
              findDisabledReason={adsFindDisabledReason}
              hasRun={adsScanSnapshot.current !== undefined}
              onFind={discoverAds}
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
          <div className="trust-inspector__title">ADS discovery</div>
        </div>
        <button onClick={onClose} aria-label="Close" className="trust-button" style={ICON}>✕</button>
      </div>

      <div style={{ flex: 1, overflow: "auto" }}>
        <div className="trust-section">
          {error && !adsIdentityRecoveryError && (
            <div data-role="discovery-error">
              <div className="trust-field__message trust-field__message--error">
                {error}
              </div>
              {(errorDetails?.length ?? 0) > 0 && (
                <details data-role="discovery-error-technical" style={WARNING_DETAILS}>
                  <summary>Technical details</summary>
                  {errorDetails?.map((detail, index) => (
                    <div key={`${index}:${detail}`}>{detail}</div>
                  ))}
                </details>
              )}
            </div>
          )}
          {warning && (
            <div data-role="discovery-partial-warning">
              <div
                data-role="discovery-partial-warning-summary"
                className="trust-field__message"
                style={{ color: "var(--trust-warn)" }}
              >
                {warning}
              </div>
              {(warningDetails?.length ?? 0) > 0 && (
                <details data-role="discovery-warning-technical" style={WARNING_DETAILS}>
                  <summary>Technical details</summary>
                  {warningDetails?.map((detail, index) => (
                    <div key={`${index}:${detail}`}>{detail}</div>
                  ))}
                </details>
              )}
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

          {adsRows.map(renderRow)}

          {otherDiscoveryRows.length > 0 && (
            <button
              type="button"
              data-role="other-discovery-types-toggle"
              aria-expanded={showOtherDiscoveryTypes}
              style={TOGGLE}
              onClick={() => setShowOtherDiscoveryTypes((visible) => !visible)}
            >
              {showOtherDiscoveryTypes ? "▾" : "▸"} Other discovery types
            </button>
          )}
          {showOtherDiscoveryTypes && (
            <div data-role="other-discovery-types">
              {otherAutomatic.map(renderRow)}

              {otherKnownAddressRows.length > 0 && (
                <div style={SECTION}>
                  Known address or subnet for other protocols
                </div>
              )}
              {otherKnownAddressRows.map(renderRow)}

              {runtimeOnly.length > 0 && (
                <>
                  <div style={SECTION}>Runtime hardware scans ⚠</div>
                  <label style={{ ...LABEL, display: "block", margin: "8px 0" }}>
                    Runtime for hardware scan
                    <select
                      data-role="runtime-scan-origin"
                      disabled={discoveryBusy}
                      value={hardwareOrigin}
                      onChange={(event) => setHardwareOrigin(event.target.value)}
                      className="trust-input"
                      style={{ marginTop: 4 }}
                    >
                      {origins.map((candidateOrigin) => (
                        <option key={candidateOrigin.id} value={candidateOrigin.id}>
                          {candidateOrigin.label}
                        </option>
                      ))}
                    </select>
                  </label>
                </>
              )}
              {runtimeOnly.map(renderRow)}
            </div>
          )}
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
                role="status"
                style={EMPTY_RECOVERY}
              >
                <p className="trust-help" style={{ margin: 0 }}>
                  {adsEmptyIdentityCopy(adsScanSnapshot.current)}
                </p>
                {(errorDetails?.length ?? 0) > 0 && (
                  <details data-role="ads-empty-technical" style={WARNING_DETAILS}>
                    <summary>Technical details</summary>
                    {errorDetails?.map((detail, index) => (
                      <div key={`${index}:${detail}`}>{detail}</div>
                    ))}
                  </details>
                )}
                <button
                  type="button"
                  data-role="ads-empty-recovery"
                  className="trust-button"
                  onClick={openAdsIdentityRecovery}
                >
                  {adsScanSnapshot.current.targetAmsNetId
                    ? "Review ADS settings"
                    : adsScanSnapshot.current.host
                      ? "Enter AMS Net ID"
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
                {emptyResultCopy(
                  scanMode === "ads"
                    ? origins.find(
                        (candidateOrigin) =>
                          candidateOrigin.id === adsScanSnapshot.current?.origin
                      )
                    : selectedHardwareOrigin,
                  hasRuntimeOriginReady
                )}
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
                    disabledReason={
                      sessionDisabledReason ??
                      adsCustomPortError ??
                      (adsProbeRunning && !adsServiceProbes[c.id]?.probing
                        ? "Wait for the current ADS device check to finish."
                        : undefined)
                    }
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

      {showOtherDiscoveryTypes && hasSelectedNonAdsScan && (
        <div className="trust-section" style={{ display: "flex", gap: 8 }}>
          <button
            data-role="scan-selected"
            onClick={scan}
            disabled={scanDisabled}
            title={
              adsHostError ??
              adsAmsNetIdError ??
              adsCustomPortError ??
              (selectedNonAdsScanRows.length === 0
                ? "Select at least one available scan type."
                : undefined)
            }
            className="trust-button"
            style={SCAN_BUTTON}
          >
            {scanning && scanMode === "selected"
              ? `Scanning ${selectedNonAdsScanRows.length} selected type${selectedNonAdsScanRows.length === 1 ? "" : "s"}…`
              : `Scan ${selectedNonAdsScanRows.length} selected type${selectedNonAdsScanRows.length === 1 ? "" : "s"}`}
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
const WARNING_DETAILS: React.CSSProperties = { marginTop: 5, fontSize: 9.5, color: "var(--trust-text-muted)", lineHeight: 1.4 };
