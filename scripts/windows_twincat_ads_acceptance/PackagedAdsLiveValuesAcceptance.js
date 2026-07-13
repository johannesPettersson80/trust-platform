"use strict";

const {
  readFreshAdsSnapshots,
} = require("./PackagedAdsLiveValuesDapProof");
const {
  readMatchingRenderedAdsValue,
} = require("./PackagedAdsLiveValuesRenderProof");

async function provePackagedAdsLiveValues({
  vscode,
  proof,
  importedVariable,
  beginStartAttempt,
  waitForRunning,
  attachByKind,
  evalInDoc,
  waitFor,
  sleep,
  check,
}) {
  const restartStartedAtMs = Date.now();
  await beginStartAttempt("ads-import-restart", true);
  const running = await waitForRunning("ads-import-restart", true, 3);
  check(
    "ads-import-restart-is-one-new-session",
    proof.journey.debug_sessions_started === 3 &&
      proof.journey.debug_sessions_terminated === 2,
    {
      debug_sessions_started: proof.journey.debug_sessions_started,
      debug_sessions_terminated: proof.journey.debug_sessions_terminated,
    }
  );

  const dapProof = await readFreshAdsSnapshots({
    vscode,
    session: running.session,
    remoteSymbol: importedVariable.remote_symbol,
    restartStartedAtMs,
    sleep,
  });
  const accepted = dapProof.accepted;
  const later = dapProof.later;
  const responseEventConverged = Boolean(accepted);
  const acceptedAtMs = Date.now();
  const qualityRecentAfterRestart =
    Number.isSafeInteger(accepted?.entry?.quality?.lastUpdateMs) &&
    accepted.entry.quality.lastUpdateMs >= restartStartedAtMs &&
    accepted.entry.quality.lastUpdateMs <= acceptedAtMs + 5_000;
  const laterScanStrictlyHigher =
    Number.isSafeInteger(later?.body?.scan) &&
    later.body.scan > accepted?.body?.scan;
  check(
    "dap-ads-state-proves-imported-live-read",
    Boolean(accepted) &&
      responseEventConverged &&
      Boolean(later) &&
      Number.isSafeInteger(accepted.body.scan) &&
      accepted.body.scan > 0 &&
      qualityRecentAfterRestart &&
      laterScanStrictlyHigher &&
      accepted.entry.access === "read" &&
      accepted.eventEntry.access === "read" &&
      typeof accepted.entry.value === "string" &&
      typeof accepted.entry.valueType === "string" &&
      accepted.entry.valueType.length > 0,
    {
      schema_version: accepted?.body?.schemaVersion,
      scan: accepted?.body?.scan,
      response_event_converged: responseEventConverged,
      response_imported_entry_found: Boolean(accepted?.entry),
      event_imported_entry_found: Boolean(accepted?.eventEntry),
      converged_events_observed: accepted?.eventsObserved,
      convergence_failures: dapProof.convergenceFailures,
      restart_started_at_ms: restartStartedAtMs,
      accepted_scan_positive: Number.isSafeInteger(accepted?.body?.scan) && accepted.body.scan > 0,
      accepted_quality_last_update_ms: accepted?.entry?.quality?.lastUpdateMs,
      accepted_quality_recent_after_restart: qualityRecentAfterRestart,
      later_response_event_converged: Boolean(later),
      later_scan: later?.body?.scan,
      later_scan_strictly_higher: laterScanStrictlyHigher,
      later_same_imported_entry: Boolean(later),
      later_imported_entry_still_good: later?.entry?.quality?.state === "good",
      later_quality_last_update_ms: later?.entry?.quality?.lastUpdateMs,
      access: accepted?.entry?.access,
      quality_states_observed: dapProof.qualityStates,
      value_present: typeof accepted?.entry?.value === "string",
      value_type_present: Boolean(accepted?.entry?.valueType),
    }
  );

  await vscode.commands.executeCommand("trust-lsp.debug.openIoPanel");
  const liveValuesSession = await attachByKind("live-values", 15_000);
  const snapshot = () =>
    evalInDoc(
      liveValuesSession,
      "#sections",
      `var rows=[...d.querySelectorAll('.ads-row')].map(function(row){var subtitles=[...row.querySelectorAll('.source-subtitle')].map(function(item){return (item.textContent||'').trim();});return {name:(row.querySelector('.name>div')?.textContent||'').trim(),remote_symbol:subtitles[0]||'',connection_access:subtitles[1]||'',value:(row.querySelector('.value')?.textContent||'').trim(),value_type:(row.querySelector('.type-cell')?.textContent||'').trim(),quality:(row.querySelector('.state-badge')?.textContent||'').trim(),button_count:row.querySelectorAll('button').length};});var body=(d.body.textContent||'').replace(/\\s+/g,' ').trim();return {rows:rows,connected_variables:/Connected variables/i.test(body),ads_section:/\\bADS\\b/i.test(body),active_target:(d.querySelector('[aria-label="Active Live Values target"]')?.textContent||'').replace(/\\s+/g,' ').trim()};`
    );
  const renderedMatch = await readMatchingRenderedAdsValue({
    vscode,
    session: running.session,
    remoteSymbol: importedVariable.remote_symbol,
    readRendered: snapshot,
    sleep,
  });
  const { rendered, row, entry: renderedDapEntry } = renderedMatch;
  check(
    "live-values-renders-imported-ads-variable-read-only",
    rendered.connected_variables &&
      rendered.ads_section &&
      row?.quality === "Good" &&
      row?.value === renderedDapEntry.value &&
      row?.value_type === renderedDapEntry.valueType &&
      /Read-only/i.test(row?.connection_access || "") &&
      row?.button_count === 0,
    {
      connected_variables: rendered.connected_variables,
      ads_section: rendered.ads_section,
      imported_entry_found: Boolean(row),
      quality: row?.quality,
      value_matches_dap: row?.value === renderedDapEntry.value,
      type_matches_dap: row?.value_type === renderedDapEntry.valueType,
      rendered_dap_scan: renderedMatch.scan,
      rendered_dap_comparisons: renderedMatch.comparisons,
      rendered_dap_snapshots: renderedMatch.dapSnapshots,
      read_only: /Read-only/i.test(row?.connection_access || ""),
      row_button_count: row?.button_count,
    }
  );
  await waitFor(
    async () => ({
      opened: proof.journey.live_values_ever_opened,
      focused: proof.journey.live_values_ever_focused,
    }),
    (value) => value.opened && value.focused,
    "explicit Live Values tab observation",
    5_000
  );
  check(
    "live-values-opens-only-when-explicitly-requested",
    proof.journey.live_values_ever_opened &&
      proof.journey.live_values_ever_focused,
    {
      opened: proof.journey.live_values_ever_opened,
      focused: proof.journey.live_values_ever_focused,
    }
  );
  proof.journey.ads_ui.live_values = {
    schema_version: accepted.body.schemaVersion,
    response_event_converged: responseEventConverged,
    response_imported_entry_found: true,
    event_imported_entry_found: true,
    converged_events_observed: accepted.eventsObserved,
    convergence_failures: dapProof.convergenceFailures,
    restart_started_at_ms: restartStartedAtMs,
    accepted_scan: accepted.body.scan,
    accepted_scan_positive: true,
    accepted_quality_last_update_ms: accepted.entry.quality.lastUpdateMs,
    accepted_quality_recent_after_restart: qualityRecentAfterRestart,
    later_response_event_converged: true,
    later_scan: later.body.scan,
    later_scan_strictly_higher: laterScanStrictlyHigher,
    later_same_imported_entry: true,
    later_imported_entry_still_good: later.entry.quality.state === "good",
    later_quality_last_update_ms: later.entry.quality.lastUpdateMs,
    later_value_changed: later.entry.value !== accepted.entry.value,
    rendered_response_event_converged: true,
    rendered_dap_scan: renderedMatch.scan,
    rendered_dap_events_observed: renderedMatch.eventsObserved,
    rendered_dap_comparisons: renderedMatch.comparisons,
    rendered_dap_snapshots: renderedMatch.dapSnapshots,
    rendered_dap_convergence_failures: renderedMatch.convergenceFailures,
    access: accepted.entry.access,
    quality: accepted.entry.quality.state,
    rendered: true,
    rendered_read_only: /Read-only/i.test(row.connection_access),
    rendered_without_actions: row.button_count === 0,
    value_matches_dap: row.value === renderedDapEntry.value,
    type_matches_dap: row.value_type === renderedDapEntry.valueType,
  };
  proof.journey.ads_ui.status = "pass";
}

module.exports = { provePackagedAdsLiveValues };
