"use strict";

const { REQUIRED_ADS_PORTS } = require("./PackagedAdsCustomPorts");
const {
  clickInnerRescan,
  enterCustomPorts,
  expandAdvanced,
} = require("./PackagedAdsCustomPortDom");

const SERVICE_STATUSES = new Set([
  "available",
  "unsupported",
  "empty",
  "route_missing",
  "check_failed",
  "unavailable",
]);

async function runPackagedAdsCustomPortAcceptance({
  evaluate,
  discoverySnapshot,
  sleep,
  check,
  state,
  target,
  expectedCustomPorts,
}) {
  const expectedPorts = [...REQUIRED_ADS_PORTS, ...expectedCustomPorts];
  const csv = expectedCustomPorts.join(",");
  const advanced = await expandAdvanced(evaluate);
  const input = await enterCustomPorts(evaluate, csv, sleep);
  const stale = await waitForState(
    discoverySnapshot,
    (snapshot) => {
      const card = targetCard(snapshot, target);
      return (
        snapshot.advanced_expanded &&
        snapshot.custom_ports_value === csv &&
        !snapshot.custom_ports_invalid &&
        card?.results_stale === true &&
        hasExactPortRows(card, REQUIRED_ADS_PORTS)
      );
    },
    sleep,
    10_000
  );
  const staleCard = targetCard(stale, target);
  const rescan = await clickInnerRescan(evaluate);
  const phases = new Set();
  const deadline = Date.now() + 90_000;
  let finalSnapshot;
  let finalCard;
  while (Date.now() < deadline) {
    finalSnapshot = await discoverySnapshot();
    if (finalSnapshot.discover_state) phases.add(finalSnapshot.discover_state);
    finalCard = targetCard(finalSnapshot, target);
    if (
      finalSnapshot.discover_text === "Scan ADS again" &&
      finalCard &&
      !finalCard.results_stale &&
      hasExactPortRows(finalCard, expectedPorts)
    ) {
      break;
    }
    await sleep(80);
  }
  const rows = finalCard?.services || [];
  const explicitRows =
    hasExactPortRows(finalCard, expectedPorts) &&
    rows.every(
      (row) =>
        SERVICE_STATUSES.has(row.status) &&
        (row.visibility === "responding" || row.visibility === "technical")
    );
  const evidence = {
    requested_custom_ports: expectedCustomPorts,
    custom_ports_input: input.value,
    input_event_dispatched: input.input_event_dispatched,
    change_event_dispatched: input.change_event_dispatched,
    advanced_expanded: advanced.clicked && stale.advanced_expanded,
    stale_results_observed: staleCard?.results_stale === true,
    stale_default_result_ports: sortedPorts(staleCard?.services || []),
    inner_rescan_click_count: rescan.clicked ? 1 : 0,
    rescan_phases_observed: [...phases],
    expected_result_ports: expectedPorts,
    result_rows: rows.map(({ port, status, visibility }) => ({ port, status, visibility })),
    exact_result_row_count: rows.length,
    same_target_reused:
      finalCard?.ams_net_id === target.ams_net_id &&
      normalizeHost(finalCard?.host) === normalizeHost(target.host),
    default_results_rechecked:
      staleCard?.results_stale === true &&
      REQUIRED_ADS_PORTS.every((port) => rowCount(rows, port) === 1),
    custom_results_present: expectedCustomPorts.every(
      (port) => rowCount(rows, port) === 1
    ),
    stale_results_cleared: finalCard?.results_stale === false,
    explicit_result_rows: explicitRows,
  };
  state.custom_port_recovery = evidence;
  check(
    "packaged-ads-ui-advanced-custom-port-results",
    expectedCustomPorts.length >= 1 &&
      expectedCustomPorts.length <= 4 &&
      evidence.input_event_dispatched &&
      evidence.change_event_dispatched &&
      evidence.advanced_expanded &&
      evidence.stale_results_observed &&
      evidence.inner_rescan_click_count === 1 &&
      evidence.rescan_phases_observed.includes("probing") &&
      evidence.same_target_reused &&
      evidence.default_results_rechecked &&
      evidence.custom_results_present &&
      evidence.stale_results_cleared &&
      evidence.explicit_result_rows,
    evidence
  );
}

async function waitForState(read, accept, sleep, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let value;
  while (Date.now() < deadline) {
    value = await read();
    if (accept(value)) return value;
    await sleep(50);
  }
  throw new Error("Timed out waiting for Advanced ADS service results to become stale.");
}

function targetCard(snapshot, target) {
  const cards = (snapshot?.cards || []).filter(
    (card) =>
      card.ams_net_id === target.ams_net_id &&
      normalizeHost(card.host) === normalizeHost(target.host)
  );
  return cards.length === 1 ? cards[0] : undefined;
}

function hasExactPortRows(card, ports) {
  const rows = card?.services || [];
  return rows.length === ports.length && ports.every((port) => rowCount(rows, port) === 1);
}

function rowCount(rows, port) {
  return rows.filter((row) => row.port === port).length;
}

function sortedPorts(rows) {
  return rows.map((row) => row.port).sort((left, right) => left - right);
}

function normalizeHost(value) {
  return String(value || "").trim().toLowerCase();
}

module.exports = { runPackagedAdsCustomPortAcceptance };
