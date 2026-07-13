"use strict";

const { requestAdsStateEvent } = require("./PackagedDapState");

async function readFreshAdsSnapshots({
  vscode,
  session,
  remoteSymbol,
  restartStartedAtMs,
  sleep,
}) {
  const observations = { qualityStates: new Set(), convergenceFailures: 0 };
  const common = { vscode, session, remoteSymbol, sleep, observations };
  const accepted = await waitForCandidate(
    common,
    Date.now() + 45_000,
    (candidate) => isFreshGood(candidate, restartStartedAtMs)
  );
  const later = accepted
    ? await waitForCandidate(
        common,
        Date.now() + 20_000,
        (candidate) =>
          isFreshGood(candidate, restartStartedAtMs) &&
          candidate.body.scan > accepted.body.scan &&
          sameImportedEntry(candidate.entry, accepted.entry)
      )
    : undefined;
  return {
    accepted,
    later,
    qualityStates: [...observations.qualityStates],
    convergenceFailures: observations.convergenceFailures,
  };
}

async function waitForCandidate(context, deadline, predicate) {
  while (Date.now() < deadline) {
    let roundTrip;
    try {
      roundTrip = await requestAdsStateEvent(
        context.vscode,
        context.session,
        context.remoteSymbol
      );
    } catch (_error) {
      context.observations.convergenceFailures += 1;
      await context.sleep(200);
      continue;
    }
    const candidate = convergedCandidate(roundTrip, context.remoteSymbol);
    for (const entry of [candidate?.entry, candidate?.eventEntry]) {
      if (entry?.quality?.state) {
        context.observations.qualityStates.add(entry.quality.state);
      }
    }
    if (candidate && predicate(candidate)) return candidate;
    await context.sleep(200);
  }
  return undefined;
}

function convergedCandidate(roundTrip, remoteSymbol) {
  const responseBody = roundTrip.response;
  const eventBody = roundTrip.eventBody;
  const responseEntries = Array.isArray(responseBody?.entries)
    ? responseBody.entries
    : [];
  const eventEntries = Array.isArray(eventBody?.entries) ? eventBody.entries : [];
  const entry = responseEntries.find(
    (candidate) => candidate?.remoteSymbol === remoteSymbol
  );
  const eventEntry = eventEntries.find(
    (candidate) => candidate?.remoteSymbol === remoteSymbol
  );
  if (
    roundTrip.responseEventConverged !== true ||
    responseBody?.schemaVersion !== 1 ||
    eventBody?.schemaVersion !== 1 ||
    !entry ||
    !eventEntry
  ) {
    return undefined;
  }
  return {
    body: responseBody,
    entry,
    eventBody,
    eventEntry,
    eventsObserved: roundTrip.eventsObserved,
  };
}

function isFreshGood(candidate, restartStartedAtMs) {
  const updatedAt = candidate.entry.quality?.lastUpdateMs;
  return (
    Number.isSafeInteger(candidate.body.scan) &&
    candidate.body.scan > 0 &&
    candidate.entry.quality?.state === "good" &&
    candidate.eventEntry.quality?.state === "good" &&
    Number.isSafeInteger(updatedAt) &&
    updatedAt >= restartStartedAtMs &&
    updatedAt <= Date.now() + 5_000
  );
}

function sameImportedEntry(candidate, accepted) {
  return ["connection", "name", "remoteSymbol", "valueType", "access"].every(
    (key) => candidate[key] === accepted[key]
  );
}

module.exports = { readFreshAdsSnapshots };
