import React, { useEffect, useMemo, useState } from "react";

import {
  didAnyAdsServiceRespond,
  groupAdsServiceProbeResults,
  resolveSelectedAdsServicePort,
  type AdsServiceProbeResult,
  type AdsServiceProbeViewState,
} from "../adsServiceProbeModel";
import { adsServiceProbeVisibleError } from "./adsErrorPresentation";
import type { DiscoverCandidate } from "../offlineComm";
import {
  adsDiscoveryFields,
  PLC_RUNTIME_PORTS,
  adsServicePresentation,
  type AdsDiscoveryDraft,
} from "./discoverPaneModel";

export function AdsDiscoveryControls({
  draft,
  hostError,
  amsNetIdError,
  customPortError,
  disabled,
  findPhase,
  findDisabledReason,
  hasRun,
  onFind,
  onChange,
}: {
  draft: AdsDiscoveryDraft;
  hostError?: string;
  amsNetIdError?: string;
  customPortError?: string;
  disabled: boolean;
  findPhase: "idle" | "finding" | "probing";
  findDisabledReason?: string;
  hasRun: boolean;
  onFind: () => void;
  onChange: (draft: AdsDiscoveryDraft) => void;
}) {
  const fields = adsDiscoveryFields(draft.advanced);
  const update = (patch: Partial<AdsDiscoveryDraft>) =>
    onChange({ ...draft, ...patch });

  return (
    <div style={CONTROLS}>
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

      {fields.includes("host") && (
        <label data-role="ads-known-host" style={FIELD_LABEL}>
          Known Host or IP <span style={OPTIONAL}>(optional recovery)</span>
          <input
            data-role="ads-host"
            value={draft.host}
            disabled={disabled}
            aria-invalid={Boolean(hostError)}
            aria-describedby={hostError ? "ads-host-error" : undefined}
            onChange={(event) => update({ host: event.target.value })}
            placeholder="Example: 192.168.50.42"
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

      {fields.includes("ams_net_id") && (
        <label style={FIELD_LABEL}>
          AMS Net ID <span style={OPTIONAL}>(optional recovery)</span>
          <input
            data-role="ads-ams-net-id"
            value={draft.amsNetId}
            disabled={disabled}
            aria-invalid={Boolean(amsNetIdError)}
            aria-describedby={amsNetIdError ? "ads-ams-net-id-error" : undefined}
            onChange={(event) => update({ amsNetId: event.target.value })}
            placeholder="Example: 5.23.91.12.1.1"
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
            placeholder="Example: 9000, 9001"
            className="trust-input"
          />
          <span className="trust-help" style={HELP}>
            Automatic checks include ADS 851–854, 301, and 501. Add other
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
          {(hostError || amsNetIdError || customPortError) ? (
            <span
              data-role="ads-advanced-attention"
              className="trust-field__message trust-field__message--error"
            >
              Advanced settings need attention: {hostError ?? amsNetIdError ?? customPortError}
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
          ) : (draft.host.trim() || draft.amsNetId.trim() || draft.customPorts.trim()) && (
            <span data-role="ads-advanced-summary" style={ADVANCED_SUMMARY}>
              Advanced: {[
                draft.host.trim() ? "known address" : "",
                draft.amsNetId.trim() ? "AMS Net ID" : "",
                draft.customPorts.trim()
                  ? `custom ports ${draft.customPorts.trim()}`
                  : "",
              ].filter(Boolean).join(" · ")}
            </span>
          )}
          <span className="trust-help" style={HELP}>
            Searches this computer and the local network, then shows responding
            ADS services (851–854, 301, and 501).
          </span>
        </>
      )}

      <button
        data-role="ads-discover"
        data-state={findPhase}
        type="button"
        onClick={onFind}
        disabled={disabled || Boolean(findDisabledReason)}
        title={findDisabledReason}
        className="trust-button trust-button--primary"
      >
        {findPhase === "finding"
          ? "Discovering ADS devices…"
          : findPhase === "probing"
            ? "Checking ADS services…"
            : hasRun
              ? "Scan ADS again"
              : "Discover ADS devices"}
      </button>
    </div>
  );
}

export function AdsDiscoveryComputerCard({
  candidate,
  probe,
  disabledReason,
  serviceResultsStale,
  onCheckServices,
  onBrowse,
}: {
  candidate: DiscoverCandidate;
  probe?: AdsServiceProbeViewState;
  disabledReason?: string;
  serviceResultsStale?: boolean;
  onCheckServices: () => void;
  onBrowse: (port: number) => void;
}) {
  const [selectedPort, setSelectedPort] = useState<number | undefined>();
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

  const host = textParam(candidate.params, "host", "ip");
  const netId = textParam(candidate.params, "ams_net_id", "target_net_id");
  const name = computerName(candidate, netId);
  const version = textParam(candidate.params, "tc_version");
  const manuallyDeclared =
    candidate.source === "manual" || candidate.confidence === "declared";
  const groupedResults = useMemo(
    () => groupAdsServiceProbeResults(probe?.results ?? []),
    [probe?.results]
  );
  const respondingResults = groupedResults.responding;
  const diagnosticResults = groupedResults.diagnostics;
  const routeMissing =
    resultsAreCurrent && respondingResults.length === 0
      ? diagnosticResults.find((result) => result.status === "route_missing")
      : undefined;
  const terminalFailure =
    resultsAreCurrent &&
    respondingResults.length === 0 &&
    diagnosticResults.some((result) => result.status === "check_failed");
  const usableCount = resultsAreCurrent
    ? (probe?.results.filter((result) => result.usable).length ?? 0)
    : 0;
  const manuallyEnteredServiceResponded =
    manuallyDeclared &&
    resultsAreCurrent &&
    didAnyAdsServiceRespond(probe?.results ?? []);
  const effectiveSelectedPort = resolveSelectedAdsServicePort(
    probe?.results ?? [],
    selectedPort,
    resultsAreCurrent
  );
  const effectiveSelected = probe?.results.find(
    (result) => result.port === effectiveSelectedPort && result.usable
  );
  const noServiceResponded = Boolean(
    resultsAreCurrent &&
      probe &&
      !probe.probing &&
      respondingResults.length === 0 &&
      (probe.completed || probe.error || probe.results.length > 0)
  );
  const discoveredIdentityOnly =
    textParam(candidate.params, "ads_service_status") === "identity_only";
  const observedIdentityOnly = Boolean(
    !manuallyDeclared &&
      respondingResults.length === 0 &&
      (discoveredIdentityOnly || noServiceResponded)
  );
  const browseReason =
    disabledReason ??
    (!resultsAreCurrent
      ? "ADS service settings changed. Check the updated services before browsing variables."
      : usableCount > 1 && !effectiveSelected
        ? "Choose an ADS service before browsing variables."
        : undefined);
  const cardState = !resultsAreCurrent
    ? "ports-changed"
    : routeMissing
    ? "route-missing"
    : probe?.probing
      ? "progress"
      : (probe?.error || terminalFailure) && respondingResults.length === 0
        ? "check-failed"
        : usableCount > 1
          ? "multiple-ports"
          : usableCount === 1
            ? "success"
            : respondingResults.length > 0
              ? "service-responded"
              : observedIdentityOnly
                ? "identity-only"
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
        <div data-role="ads-computer-name" style={COMPUTER_NAME} title={name}>
          {name}
        </div>
        <div style={COMPUTER_DETAIL}>
          ADS device
          {candidate.source === "ads_local_router"
            ? " · On the discovery computer"
            : host
              ? ` · ${host}`
              : ""}
        </div>
        <div
          data-role="ads-identity-status"
          data-status={
            manuallyEnteredServiceResponded
              ? "service-responded"
              : manuallyDeclared
                ? "declared"
                : observedIdentityOnly
                  ? "identity-only"
                  : "found"
          }
          style={
            (manuallyDeclared && !manuallyEnteredServiceResponded) ||
            observedIdentityOnly
              ? DECLARED_IDENTITY
              : FOUND_IDENTITY
          }
        >
          {manuallyEnteredServiceResponded
            ? "Address entered manually · ADS service responded"
            : manuallyDeclared
              ? "Address entered manually · waiting for an ADS response"
              : observedIdentityOnly
                ? "Identity found · ADS services not confirmed"
                : "Found"}
        </div>
      </div>

      <details style={DETAILS}>
        <summary>Technical details</summary>
        {host && <div>Host: {host}</div>}
        <div>AMS Net ID: {netId || "not reported"}</div>
        {version && <div>Device software version: {version}</div>}
        <div>Identity source: {identitySourceLabel(candidate.source)}</div>
        {probe?.error && <div>Service check: {probe.error}</div>}
        {diagnosticResults.map((result) => (
          <div
            key={`service-diagnostic:${result.port}`}
            data-role="ads-plc-runtime"
            data-result-visibility="technical"
            data-ads-port={result.port}
            data-service-kind={serviceKind(result.port)}
            data-status={result.status}
          >
            ADS {result.port}: {serviceStatusLabel(result)}
            {result.error?.message ? ` — ${result.error.message}` : ""}
          </div>
        ))}
      </details>

      {probe?.probing && (
        <div
          role="status"
          data-role="ads-probe-progress"
          data-state="progress"
          style={PROBE_STATUS}
        >
          {probe.currentPort
            ? `Checking ${adsServicePresentation(probe.currentPort).primary}…`
            : "Preparing ADS service checks…"}
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

      {!probe && (
        <div role="status" data-role="ads-probe-pending" style={PROBE_STATUS}>
          Waiting to check responding ADS services…
        </div>
      )}

      {noServiceResponded && !routeMissing && (
        <div
          data-role="ads-no-service-response"
          data-state={probe?.error || terminalFailure ? "check-failed" : "unavailable"}
          className="trust-field__message"
          style={PROBE_STATUS}
        >
          {probe?.error
            ? adsServiceProbeVisibleError(probe.error)
            : diagnosticResults.length > 0
              ? `The ADS device was found, but none of its ${diagnosticResults.length} checked services responded. Make sure it is running, then try again.`
              : "The ADS device was found, but its services could not be checked. Make sure it is running, then try again."}
        </div>
      )}

      {routeMissing && (
        <div
          data-role="ads-route-recovery"
          data-state="route-missing"
          className="trust-field__message"
          style={PROBE_STATUS}
        >
          This remote ADS device needs a route before its services can be checked.
        </div>
      )}

      {!probe?.probing && !routeMissing && (noServiceResponded || serviceResultsStale) && (
        <button
          type="button"
          data-role="ads-recheck-services"
          onClick={onCheckServices}
          disabled={Boolean(disabledReason)}
          title={disabledReason}
          className="trust-button"
        >
          {serviceResultsStale
            ? "Check updated ADS services"
            : probe?.error || terminalFailure
              ? "Retry ADS service check"
              : "Check ADS services again"}
        </button>
      )}

      {respondingResults.length > 0 && (
        <fieldset
          data-role="ads-service-results"
          style={{ display: "grid", gap: 4, border: 0, padding: 0, margin: 0 }}
        >
          <legend style={{ fontSize: 10, color: "var(--trust-text-muted)", padding: 0 }}>
            Responding ADS services for {name}
          </legend>
          {respondingResults.map((result) => {
            const presentation = adsServicePresentation(result.port);
            return (
              <div
                key={result.port}
                data-role="ads-plc-runtime"
                data-result-visibility="responding"
                data-ads-port={result.port}
                data-service-kind={serviceKind(result.port)}
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
                    aria-label={`Select ${presentation.primary}: ${presentation.secondary}`}
                  />
                ) : (
                  <span aria-hidden="true" style={STATUS_DOT}>•</span>
                )}
                <span style={{ flex: 1, minWidth: 0 }}>
                  <span style={RUNTIME_PRIMARY}>{presentation.primary}</span>
                  <span style={RUNTIME_SECONDARY}>
                    {presentation.secondary} · {serviceStatusLabel(result)}
                  </span>
                </span>
              </div>
            );
          })}
        </fieldset>
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
      ) : browseReason && usableCount > 0 ? (
        <div
          data-role="ads-browse-disabled-reason"
          data-state="disabled"
          className="trust-field__message"
          style={PROBE_STATUS}
        >
          {browseReason}
        </div>
      ) : null}

      {!routeMissing && usableCount > 0 && (
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

      {respondingResults.length > 0 && usableCount === 0 && (
        <div
          data-role="ads-no-browsable-service"
          data-state="responded"
          className="trust-field__message"
          style={PROBE_STATUS}
        >
          ADS responded, but no service reported browsable variables. Add another
          service under Advanced if needed.
        </div>
      )}

      {respondingResults.length > 0 && diagnosticResults.length > 0 && (
        <div
          data-role="ads-service-diagnostics-summary"
          data-count={diagnosticResults.length}
          className="trust-help"
          style={PROBE_STATUS}
        >
          {diagnosticResults.length} other ADS service{diagnosticResults.length === 1 ? " was" : "s were"}
          {" "}unavailable or could not be checked. See Technical details above.
        </div>
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
      return "Could not check this service";
    case "unavailable":
      return "Not running or unavailable";
  }
}

function serviceKind(port: number): "plc-runtime" | "ads-service" {
  return PLC_RUNTIME_PORTS.includes(
    port as (typeof PLC_RUNTIME_PORTS)[number]
  )
    ? "plc-runtime"
    : "ads-service";
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
  return withoutIdentity || "ADS device";
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
const FIELD_LABEL: React.CSSProperties = { display: "grid", gap: 4, fontSize: 10.5 };
const OPTIONAL: React.CSSProperties = { color: "var(--trust-text-muted)", fontWeight: 400 };
const ADVANCED_SUMMARY: React.CSSProperties = { fontSize: 10, color: "var(--trust-accent)" };
const ADVANCED_BUTTON: React.CSSProperties = { border: 0, padding: "3px 0", textAlign: "left", background: "transparent", color: "var(--trust-text-muted)", cursor: "pointer", fontSize: 10.5 };
const HELP: React.CSSProperties = { display: "block", lineHeight: 1.35 };
const CARD: React.CSSProperties = { display: "grid", gap: 7, padding: "9px", marginTop: 7, borderRadius: "var(--trust-radius-lg)", border: "1px solid var(--trust-border)", background: "var(--trust-surface)" };
const COMPUTER_NAME: React.CSSProperties = {
  fontSize: 12.5,
  fontWeight: 650,
  lineHeight: 1.3,
  overflowWrap: "anywhere",
};
const COMPUTER_DETAIL: React.CSSProperties = { fontSize: 10, color: "var(--trust-text-muted)", overflowWrap: "anywhere" };
const FOUND_IDENTITY: React.CSSProperties = { fontSize: 9.5, color: "var(--trust-ok)" };
const DECLARED_IDENTITY: React.CSSProperties = { fontSize: 9.5, color: "var(--trust-warn)" };
const DETAILS: React.CSSProperties = { fontSize: 9.5, color: "var(--trust-text-muted)", lineHeight: 1.4 };
const PROBE_STATUS: React.CSSProperties = { fontSize: 10.5, color: "var(--trust-text-muted)" };
const RUNTIME_ROW: React.CSSProperties = { display: "flex", gap: 7, alignItems: "center", padding: "5px 6px", borderRadius: "var(--trust-radius-sm)", background: "var(--trust-surface-raised)" };
const STATUS_DOT: React.CSSProperties = { width: 13, textAlign: "center", color: "var(--trust-text-subtle)" };
const RUNTIME_PRIMARY: React.CSSProperties = { display: "block", fontSize: 11.5, fontWeight: 600 };
const RUNTIME_SECONDARY: React.CSSProperties = { display: "block", fontSize: 10.5, lineHeight: 1.35, color: "var(--trust-text-muted)", whiteSpace: "normal", overflowWrap: "anywhere" };
