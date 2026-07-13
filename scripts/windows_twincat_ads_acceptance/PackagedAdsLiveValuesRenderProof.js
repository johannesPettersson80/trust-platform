"use strict";

const { requestAdsStateEvent } = require("./PackagedDapState");

async function readMatchingRenderedAdsValue({
  vscode,
  session,
  remoteSymbol,
  readRendered,
  sleep,
  timeoutMs = 20_000,
}) {
  const deadline = Date.now() + timeoutMs;
  let convergenceFailures = 0;
  let comparisons = 0;
  let dapSnapshots = 0;
  while (Date.now() < deadline) {
    let roundTrip;
    try {
      roundTrip = await requestAdsStateEvent(vscode, session, remoteSymbol);
    } catch (_error) {
      convergenceFailures += 1;
      await sleep(120);
      continue;
    }
    const entries = Array.isArray(roundTrip.response?.entries)
      ? roundTrip.response.entries
      : [];
    const selected = entries.filter(
      (candidate) => candidate?.remoteSymbol === remoteSymbol
    );
    const entry = selected.length === 1 ? selected[0] : undefined;
    dapSnapshots += 1;
    const dapSampleValid =
      roundTrip.responseEventConverged === true &&
      Number.isSafeInteger(roundTrip.response?.scan) &&
      roundTrip.response.scan > 0 &&
      entry?.quality?.state === "good" &&
      typeof entry.value === "string";
    const renderDeadline = Math.min(deadline, Date.now() + 750);
    while (dapSampleValid && Date.now() < renderDeadline) {
      const rendered = await readRendered();
      const rows = rendered.rows.filter((row) => row.remote_symbol === remoteSymbol);
      const row = rows.length === 1 ? rows[0] : undefined;
      comparisons += 1;
      if (
        row?.quality === "Good" &&
        row.value === entry.value &&
        row.value_type === entry.valueType
      ) {
        return {
          rendered,
          row,
          entry,
          scan: roundTrip.response.scan,
          eventsObserved: roundTrip.eventsObserved,
          comparisons,
          dapSnapshots,
          convergenceFailures,
        };
      }
      await sleep(50);
    }
    await sleep(120);
  }
  throw new Error(
    `Timed out matching rendered ADS value to ${dapSnapshots} DAP snapshot(s) after ${comparisons} DOM comparison(s) and ${convergenceFailures} convergence failure(s).`
  );
}

module.exports = { readMatchingRenderedAdsValue };
