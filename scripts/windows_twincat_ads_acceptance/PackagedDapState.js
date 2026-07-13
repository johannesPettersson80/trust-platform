"use strict";

const { selectedAdsSnapshot } = require("./PackagedAdsDapSnapshot");
const { requestIoStateEvent } = require("./PackagedDapIoState");

function requestAdsStateEvent(
  vscode,
  session,
  remoteSymbol,
  timeoutMs = 7_000
) {
  return new Promise((resolve, reject) => {
    let response;
    let responseReady = false;
    let settled = false;
    const events = [];
    let subscription;
    let timer;

    const finish = (error, value) => {
      if (settled) return;
      settled = true;
      if (timer) clearTimeout(timer);
      if (subscription) subscription.dispose();
      if (error) reject(error);
      else resolve(value);
    };
    const tryConverge = () => {
      if (!responseReady) return;
      const responseSnapshot = selectedAdsSnapshot(response, remoteSymbol);
      if (!responseSnapshot) return;
      for (const eventBody of events) {
        const eventSnapshot = selectedAdsSnapshot(eventBody, remoteSymbol);
        if (
          eventSnapshot &&
          JSON.stringify(responseSnapshot) === JSON.stringify(eventSnapshot)
        ) {
          finish(undefined, {
            response,
            eventBody,
            responseSnapshot,
            eventSnapshot,
            responseEventConverged: true,
            eventsObserved: events.length,
          });
          return;
        }
      }
    };

    subscription = vscode.debug.onDidReceiveDebugSessionCustomEvent((candidate) => {
      if (candidate.session.id === session.id && candidate.event === "stAdsState") {
        events.push(candidate.body);
        tryConverge();
      }
    });
    timer = setTimeout(
      () =>
        finish(
          new Error(
            `Timed out waiting for stAdsState response/event convergence for ${remoteSymbol}; observed ${events.length} event(s).`
          )
        ),
      timeoutMs
    );
    Promise.resolve()
      .then(() => session.customRequest("stAdsState"))
      .then((body) => {
        response = body;
        responseReady = true;
        tryConverge();
      })
      .catch((error) => finish(error));
  });
}

module.exports = { requestAdsStateEvent, requestIoStateEvent };
