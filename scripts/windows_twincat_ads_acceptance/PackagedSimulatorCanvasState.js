"use strict";

const { waitFor } = require("./AcceptanceWait");

function isSimulatorVisualState(simulator, health, label) {
  return simulator?.health === health && simulator.statusText === label;
}

function isSettledSimulatorCanvas(snapshot, health, label) {
  const text = String(snapshot?.canvasText || "");
  return isSimulatorVisualState(snapshot?.simulator, health, label) &&
    /Simulated I\/O/i.test(text) &&
    !/Loading your devices|No devices yet/i.test(text);
}

function waitForSettledSimulatorCanvas(
  canvasSnapshot,
  { health, label, description, timeoutMs = 20_000 }
) {
  return waitFor(
    canvasSnapshot,
    (value) => isSettledSimulatorCanvas(value, health, label),
    description,
    timeoutMs
  );
}

module.exports = {
  isSettledSimulatorCanvas,
  isSimulatorVisualState,
  waitForSettledSimulatorCanvas,
};
