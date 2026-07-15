import React, { useEffect, useMemo, useState } from "react";

import {
  resolveSelectedAdsServicePort,
  shouldShowAdsServiceCheckConfirmation,
  type AdsServiceProbeResult,
  type AdsServiceProbeViewState,
} from "../adsServiceProbeModel";
import type { DiscoverCandidate } from "../offlineComm";
import {
  ADS_DISCOVERY_LOCATIONS,
  AUTOMATIC_TWINCAT_SERVICE_PORTS,
  adsDiscoveryFields,
  PLC_RUNTIME_PORTS,
  twinCatServicePresentation,
  type AdsDiscoveryDraft,
  type AdsDiscoveryLocationId,
} from "./discoverPaneModel";

export function AdsDiscoveryControls({
  checked,
  draft,
  hostError,
  amsNetIdError,
  customPortError,
  disabled,
  findPhase,
  findDisabledReason,
  onFind,
  onChange,
}: {
  checked: boolean;
  draft: AdsDiscoveryDraft;
  hostError?: string;
  amsNetIdError?: string;
  customPortError?: string;
  disabled: boolean;
  findPhase: "idle" | "finding" | "probing";
  findDisabledReason?: string;
  onFind: () => void;
  onChange: (draft: AdsDiscoveryDraft) => void;
}) {
  if (!checked) {
    return null;
  }
  const fields = adsDiscoveryFields(draft.location, draft.advanced);
  const update = (patch: Partial<AdsDiscoveryDraft>) =>
    onChange({ ...draft, ...patch });

  return (
    <div style={CONTROLS}>
      <fieldset data-role="ads-location" style={FIELDSET}>
        <legend style={LEGEND}>Where is TwinCAT?</legend>
        {ADS_DISCOVERY_LOCATIONS.map((location) => (
          <label key={location.id} style={RADIO_ROW}>
            <input
              data-role="ads-location-option"
              type="radio"
              name="ads-target-location"
              value={location.id}
              checked={draft.location === location.id}
              disabled={disabled}
              onChange={() =>
                update({ location: location.id as AdsDiscoveryLocationId })
              }
            />
            <span>{location.label}</span>
            {location.recommended && <span style={RECOMMENDED}>Recommended</span>}
          </label>
        ))}
      </fieldset>

      {fields.includes("host") && (
        <label style={FIELD_LABEL}>
          Host or IP
          <input
            data-role="ads-host"
            value={draft.host}
            disabled={disabled}
            aria-invalid={Boolean(hostError)}
            aria-describedby={hostError ? "ads-host-error" : undefined}
            onChange={(event) => update({ host: event.target.value })}
            placeholder="192.168.77.11"
            className="trust-input"
          />
          {hostError && (
            <span
              id="ads-host-error"
              data-role="ads-validation-error"
              data-state="validation"
              className="trust-field__message trust-field__message--error"
            >
              {hostError}
            </span>
          )}
        </label>
      )}

      <button
        data-role="ads-advanced-toggle"
        type="button"
        aria-expanded={draft.advanced}
        disabled={disabled}
        onClick={() => update({ advanced: !draft.advanced })}
        style={ADVANCED_BUTTON}
      >
        {draft.advanced ? "▾" : "▸"} Advanced
      </button>

      {fields.includes("ams_net_id") && (
        <label style={FIELD_LABEL}>
          AMS Net ID <span style={OPTIONAL}>(optional manual fallback)</span>
          <input
            data-role="ads-ams-net-id"
            value={draft.amsNetId}
            disabled={disabled}
            aria-invalid={Boolean(amsNetIdError)}
            aria-describedby={amsNetIdError ? "ads-ams-net-id-error" : undefined}
            onChange={(event) => update({ amsNetId: event.target.value })}
            placeholder="5.23.91.12.1.1"
            className="trust-input"
          />
          {amsNetIdError && (
            <span
              id="ads-ams-net-id-error"
              data-role="ads-validation-error"
              data-state="validation"
              className="trust-field__message trust-field__message--error"
            >
              {amsNetIdError}
            </span>
          )}
        </label>
      )}

      {fields.includes("ads_port") && (
        <label style={FIELD_LABEL}>
          Additional ADS service ports <span style={OPTIONAL}>(optional)</span>
          <input
            data-role="ads-custom-ports"
            value={draft.customPorts}
            disabled={disabled}
            aria-invalid={Boolean(customPortError)}
            aria-describedby={customPortError ? "ads-custom-ports-error" : undefined}
            onChange={(event) => update({ customPorts: event.target.value })}
            placeholder="9000, 9001"
            className="trust-input"
          />
          <span className="trust-help" style={HELP}>
            The confirmed service check includes PLC runtimes ADS 851–854,
            Additional task 1 (ADS 301), and NC SAF service (ADS 501). Add other
            logical services here. These are not TCP or UDP socket ports. Add up
            to four additional ports; at most ten services are checked in total.
          </span>
          {customPortError && (
            <span
              id="ads-custom-ports-error"
              data-role="ads-validation-error"
              data-state="validation"
              className="trust-field__message trust-field__message--error"
            >
              {customPortError}
            </span>
          )}
        </label>
      )}

      {!draft.advanced && (
        <>
          {(amsNetIdError || customPortError) ? (
            <span
              data-role="ads-advanced-attention"
              className="trust-field__message trust-field__message--error"
            >
              Advanced settings need attention: {amsNetIdError ?? customPortError}
              <button
                type="button"
                disabled={disabled}
                onClick={() => update({ advanced: true })}
                className="trust-button"
                style={{ marginLeft: 6 }}
              >
                Expand
              </button>
            </span>
          ) : (draft.amsNetId.trim() || draft.customPorts.trim()) && (
            <span data-role="ads-advanced-summary" style={ADVANCED_SUMMARY}>
              Advanced settings applied
            </span>
          )}
          <span className="trust-help" style={HELP}>
            After finding the TwinCAT computer, confirm that other software on
            the discovery computer is not reading TwinCAT, then check ADS {AUTOMATIC_TWINCAT_SERVICE_PORTS.join(", ")}.
          </span>
        </>
      )}

      <button
        data-role="ads-find-twincat"
        data-state={findPhase}
        type="button"
        onClick={onFind}
        disabled={disabled || Boolean(findDisabledReason)}
        title={findDisabledReason}
        className="trust-button trust-button--primary"
      >
        {findPhase === "finding"
          ? "Finding TwinCAT…"
          : findPhase === "probing"
            ? "Checking TwinCAT services…"
            : "Find TwinCAT"}
      </button>
    </div>
  );
}

export function AdsDiscoveryComputerCard({
  candidate,
  probe,
  servicePorts,
  discoveryOriginLabel,
  disabledReason,
  serviceResultsStale,
  onCheckServices,
  onBrowse,
}: {
  candidate: DiscoverCandidate;
  probe?: AdsServiceProbeViewState;
  servicePorts: readonly number[];
  discoveryOriginLabel: string;
  disabledReason?: string;
  serviceResultsStale?: boolean;
  onCheckServices: () => void;
  onBrowse: (port: number) => void;
}) {
  const [selectedPort, setSelectedPort] = useState<number | undefined>();
  const [connectionSafetyConfirmed, setConnectionSafetyConfirmed] =
    useState(false);
  const [recheckRequested, setRecheckRequested] = useState(false);
  const servicePortPlanKey = servicePorts.join(",");
  const resultsAreCurrent = !serviceResultsStale;
  const usablePorts = useMemo(
    () =>
      new Set(
        resultsAreCurrent
          ? (probe?.results ?? [])
              .filter((result) => result.usable)
              .map((result) => result.port)
          : []
      ),
    [probe?.results, resultsAreCurrent]
  );

  useEffect(() => {
    if (selectedPort !== undefined && !usablePorts.has(selectedPort)) {
      setSelectedPort(undefined);
    }
  }, [selectedPort, usablePorts]);

  useEffect(() => {
    setConnectionSafetyConfirmed(false);
  }, [servicePortPlanKey]);

  const host = textParam(candidate.params, "host", "ip");
  const netId = textParam(candidate.params, "ams_net_id", "target_net_id");
  const name = computerName(candidate, netId);
  const version = textParam(candidate.params, "tc_version");
  const manuallyDeclared =
    candidate.source === "manual" || candidate.confidence === "declared";
  const checkServices = () => {
    setConnectionSafetyConfirmed(false);
    setRecheckRequested(false);
    onCheckServices();
  };
  const routeMissing = resultsAreCurrent
    ? probe?.results.find((result) => result.status === "route_missing")
    : undefined;
  const terminalFailure = resultsAreCurrent
    ? probe?.results.some((result) => result.status === "check_failed")
    : false;
  const usableCount = resultsAreCurrent
    ? (probe?.results.filter((result) => result.usable).length ?? 0)
    : 0;
  const needsServiceCheckConfirmation =
    shouldShowAdsServiceCheckConfirmation(
      probe,
      Boolean(serviceResultsStale),
      recheckRequested
    );
  const effectiveSelectedPort = resolveSelectedAdsServicePort(
    probe?.results ?? [],
    selectedPort,
    resultsAreCurrent
  );
  const effectiveSelected = probe?.results.find(
    (result) => result.port === effectiveSelectedPort && result.usable
  );
  const browseReason =
    disabledReason ??
    (!resultsAreCurrent
      ? "ADS service settings changed. Check the updated services before browsing variables."
      : routeMissing
      ? "Set up the route, then check and browse variables."
      : !effectiveSelected && probe && !probe.probing
        ? browseDisabledReason(probe.results)
        : undefined);
  const cardState = !resultsAreCurrent
    ? "ports-changed"
    : routeMissing
    ? "route-missing"
    : probe?.probing
      ? "progress"
      : probe?.error || terminalFailure
        ? "check-failed"
        : usableCount > 1
          ? "multiple-ports"
          : usableCount === 1
            ? "success"
            : manuallyDeclared
              ? "manual-declared"
              : "found";

  return (
    <div
      data-role="ads-computer"
      data-state={cardState}
      data-identity={manuallyDeclared ? "declared" : "observed"}
      style={CARD}
    >
      <div style={{ minWidth: 0 }}>
        <div style={COMPUTER_NAME}>{name}</div>
        <div style={COMPUTER_DETAIL}>
          TwinCAT computer
          {candidate.source === "ads_local_router"
            ? " · On the discovery computer"
            : host
              ? ` · ${host}`
              : ""}
        </div>
        <div
          data-role="ads-identity-status"
          data-status={manuallyDeclared ? "declared" : "found"}
          style={manuallyDeclared ? DECLARED_IDENTITY : FOUND_IDENTITY}
        >
          {manuallyDeclared
            ? "Entered manually — identity not verified yet"
            : "Found"}
        </div>
      </div>

      <details style={DETAILS}>
        <summary>Technical details</summary>
        {host && <div>Host: {host}</div>}
        <div>AMS Net ID: {netId || "not reported"}</div>
        {version && <div>TwinCAT version: {version}</div>}
        <div>Identity source: {identitySourceLabel(candidate.source)}</div>
      </details>

      {probe?.probing && (
        <div
          role="status"
          data-role="ads-probe-progress"
          data-state="progress"
          style={PROBE_STATUS}
        >
          {probe.currentPort
            ? `Checking ${twinCatServicePresentation(probe.currentPort).primary} (ADS ${probe.currentPort})…`
            : "Preparing TwinCAT service checks…"}
        </div>
      )}
      {probe?.error && (
        <div
          data-role="ads-probe-error"
          data-state="check-failed"
          className="trust-field__message trust-field__message--error"
        >
          TwinCAT was found, but its services could not be checked: {probe.error}
        </div>
      )}

      {!resultsAreCurrent && (
        <div
          data-role="ads-results-stale"
          data-state="ports-changed"
          className="trust-field__message"
          style={PROBE_STATUS}
        >
          These results use the previous ADS service list. Check the updated
          services before selecting or browsing one.
        </div>
      )}

      {needsServiceCheckConfirmation && (
        <div
          data-role="ads-probe-safety"
          data-state="confirmation-required"
          style={PROBE_SAFETY}
        >
          <div>
            Checking opens a temporary ADS connection from {discoveryOriginLabel}.
            Before checking, stop any truST runtime or other software there that
            is currently reading TwinCAT. Leave TwinCAT and the PLC running.
          </div>
          <label style={SAFETY_CONFIRMATION}>
            <input
              data-role="ads-probe-safety-confirmation"
              type="checkbox"
              checked={connectionSafetyConfirmed}
              disabled={Boolean(disabledReason) || probe?.probing}
              onChange={(event) =>
                setConnectionSafetyConfirmed(event.target.checked)
              }
            />
            <span>
              I stopped other software on {discoveryOriginLabel} that is reading
              TwinCAT
            </span>
          </label>
          <button
            type="button"
            data-role="ads-check-services"
            data-state={serviceResultsStale ? "ports-changed" : "ready"}
            onClick={checkServices}
            disabled={
              !connectionSafetyConfirmed ||
              Boolean(disabledReason) ||
              probe?.probing
            }
            title={
              disabledReason ??
              (!connectionSafetyConfirmed
                ? "Confirm that other software is not reading TwinCAT first."
                : undefined)
            }
            className="trust-button trust-button--primary"
          >
            {probe?.error || terminalFailure
              ? "Retry service checks"
              : serviceResultsStale
                ? "Check updated ADS services"
                : probe?.completed
                  ? "Check services again"
                : `Check ${servicePorts.length} ADS services`}
          </button>
        </div>
      )}

      {probe?.completed && !needsServiceCheckConfirmation && (
        <button
          type="button"
          data-role="ads-recheck-services"
          onClick={() => {
            setConnectionSafetyConfirmed(false);
            setRecheckRequested(true);
          }}
          disabled={Boolean(disabledReason)}
          title={disabledReason}
          className="trust-button"
        >
          Check services again
        </button>
      )}

      {(probe?.results ?? []).map((result) => {
        const presentation = twinCatServicePresentation(result.port);
        return (
          <label
            key={result.port}
            data-role="ads-plc-runtime"
            data-ads-port={result.port}
            data-service-kind={
              PLC_RUNTIME_PORTS.includes(
                result.port as (typeof PLC_RUNTIME_PORTS)[number]
              )
                ? "plc-runtime"
                : "twincat-service"
            }
            data-status={result.status}
            style={RUNTIME_ROW}
          >
            {result.usable ? (
              <input
                type="radio"
                name={`ads-runtime-${candidate.id}`}
                value={result.port}
                checked={effectiveSelectedPort === result.port}
                disabled={!resultsAreCurrent || Boolean(disabledReason)}
                onChange={() => setSelectedPort(result.port)}
                aria-label={`Select ${presentation.primary} (ADS ${result.port})`}
              />
            ) : (
              <span aria-hidden="true" style={STATUS_DOT}>•</span>
            )}
            <span style={{ flex: 1, minWidth: 0 }}>
              <span style={RUNTIME_PRIMARY}>{presentation.primary}</span>
              <span style={RUNTIME_SECONDARY}>
                ({presentation.secondary}) · {serviceStatusLabel(result)}
              </span>
            </span>
          </label>
        );
      })}

      {!probe?.probing && probe && probe.results.length === 0 && !probe.error && (
        <div style={PROBE_STATUS}>No TwinCAT service probe results were returned.</div>
      )}

      {routeMissing && (
        <button
          type="button"
          data-role="ads-route-setup"
          onClick={() => onBrowse(routeMissing.port)}
          disabled={Boolean(disabledReason)}
          title={disabledReason}
          className={disabledReason ? "trust-button" : "trust-button trust-button--primary"}
        >
          Set up route
        </button>
      )}

      {browseReason && usableCount > 1 && !disabledReason ? (
        <div
          data-role="ads-runtime-choice-required"
          data-state="multiple-ports"
          className="trust-field__message"
          style={PROBE_STATUS}
        >
          {browseReason}
        </div>
      ) : browseReason ? (
        <div
          data-role="ads-browse-disabled-reason"
          data-state="disabled"
          className="trust-field__message"
          style={PROBE_STATUS}
        >
          {browseReason}
        </div>
      ) : null}

      {!routeMissing && (
        <button
          type="button"
          data-role="ads-browse-variables"
          onClick={() => effectiveSelected && onBrowse(effectiveSelected.port)}
          disabled={!effectiveSelected || Boolean(disabledReason)}
          title={browseReason}
          className={effectiveSelected && !disabledReason ? "trust-button trust-button--primary" : "trust-button"}
        >
          Browse variables
        </button>
      )}
    </div>
  );
}

function serviceStatusLabel(result: AdsServiceProbeResult): string {
  switch (result.status) {
    case "available":
      return `Available — ${result.symbolCount} variable${result.symbolCount === 1 ? "" : "s"}`;
    case "unsupported":
      return "Available, but Symbol Upload is not supported";
    case "empty":
      return "Available, but no variables were reported";
    case "route_missing":
      return "Route setup required";
    case "check_failed":
      return `Check failed — ${result.error?.message ?? "unknown error"}`;
    case "unavailable":
      return "Not running or unavailable";
  }
}

function browseDisabledReason(results: readonly AdsServiceProbeResult[]): string {
  const usable = results.filter((result) => result.usable).length;
  if (usable > 1) {
    return "Choose a TwinCAT service before browsing variables.";
  }
  return "No TwinCAT service with browsable variables is available yet.";
}

function textParam(
  params: Record<string, unknown>,
  ...keys: string[]
): string {
  for (const key of keys) {
    const value = params[key];
    if (typeof value === "string" && value.trim()) {
      return value.trim();
    }
  }
  return "";
}

function computerName(candidate: DiscoverCandidate, netId: string): string {
  const reported = textParam(candidate.params, "name");
  if (reported) {
    return reported;
  }
  const label = candidate.label.trim();
  const withoutIdentity = netId
    ? label.replace(` · ${netId}`, "").replace(`TwinCAT ${netId}`, "").trim()
    : label;
  return withoutIdentity || "TwinCAT computer";
}

function identitySourceLabel(source: string): string {
  switch (source) {
    case "ads_local_router":
      return "Local AMS router";
    case "ads_identify":
      return "Directed UDP Identify";
    case "ads_broadcast":
      return "Network UDP discovery";
    case "manual":
      return "Entered AMS identity";
    default:
      return source.replace(/_/g, " ");
  }
}

const CONTROLS: React.CSSProperties = { margin: "6px 0 2px 24px", display: "grid", gap: 7 };
const FIELDSET: React.CSSProperties = { border: 0, margin: 0, padding: 0, minWidth: 0 };
const LEGEND: React.CSSProperties = { fontSize: 11, fontWeight: 650, marginBottom: 4 };
const RADIO_ROW: React.CSSProperties = { display: "flex", gap: 6, alignItems: "center", fontSize: 11, marginTop: 4 };
const RECOMMENDED: React.CSSProperties = { color: "var(--trust-accent)", fontSize: 9.5 };
const FIELD_LABEL: React.CSSProperties = { display: "grid", gap: 4, fontSize: 10.5 };
const OPTIONAL: React.CSSProperties = { color: "var(--trust-text-muted)", fontWeight: 400 };
const ADVANCED_SUMMARY: React.CSSProperties = { fontSize: 10, color: "var(--trust-accent)" };
const ADVANCED_BUTTON: React.CSSProperties = { border: 0, padding: "3px 0", textAlign: "left", background: "transparent", color: "var(--trust-text-muted)", cursor: "pointer", fontSize: 10.5 };
const HELP: React.CSSProperties = { display: "block", lineHeight: 1.35 };
const CARD: React.CSSProperties = { display: "grid", gap: 7, padding: "9px", marginTop: 7, borderRadius: "var(--trust-radius-lg)", border: "1px solid var(--trust-border)", background: "var(--trust-surface)" };
const COMPUTER_NAME: React.CSSProperties = { fontSize: 12.5, fontWeight: 650, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" };
const COMPUTER_DETAIL: React.CSSProperties = { fontSize: 10, color: "var(--trust-text-muted)", overflowWrap: "anywhere" };
const FOUND_IDENTITY: React.CSSProperties = { fontSize: 9.5, color: "var(--trust-success, #43a047)" };
const DECLARED_IDENTITY: React.CSSProperties = { fontSize: 9.5, color: "var(--trust-warn)" };
const DETAILS: React.CSSProperties = { fontSize: 9.5, color: "var(--trust-text-muted)", lineHeight: 1.4 };
const PROBE_STATUS: React.CSSProperties = { fontSize: 10.5, color: "var(--trust-text-muted)" };
const PROBE_SAFETY: React.CSSProperties = { display: "grid", gap: 7, padding: "8px", borderRadius: "var(--trust-radius-sm)", border: "1px solid var(--trust-warn)", fontSize: 10.5, color: "var(--trust-text-muted)" };
const SAFETY_CONFIRMATION: React.CSSProperties = { display: "flex", gap: 7, alignItems: "flex-start", color: "var(--trust-text)" };
const RUNTIME_ROW: React.CSSProperties = { display: "flex", gap: 7, alignItems: "center", padding: "5px 6px", borderRadius: "var(--trust-radius-sm)", background: "var(--trust-surface-raised)" };
const STATUS_DOT: React.CSSProperties = { width: 13, textAlign: "center", color: "var(--trust-text-subtle)" };
const RUNTIME_PRIMARY: React.CSSProperties = { display: "block", fontSize: 11.5, fontWeight: 600 };
const RUNTIME_SECONDARY: React.CSSProperties = { display: "block", fontSize: 10.5, lineHeight: 1.35, color: "var(--trust-text-muted)", whiteSpace: "normal", overflowWrap: "anywhere" };
