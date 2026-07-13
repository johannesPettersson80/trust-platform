"use strict";

const { waitFor } = require("./AcceptanceWait");
const {
  isSimulatorVisualState,
  waitForSettledSimulatorCanvas,
} = require("./PackagedSimulatorCanvasState");
const {
  isBlockedStartingAction,
  isStartingAction,
} = require("./PackagedSimulatorVisualProof");

function createSimulatorLifecycle({
  vscode,
  proof,
  check,
  activeTabLabel,
  clickAction,
  sidebarSnapshot,
  pageText,
  canvasSnapshot,
  clearActiveSession,
  activeSession,
}) {
  async function beginStartAttempt(name, devicesOpen) {
    const activeBefore = activeTabLabel();
    clearActiveSession();
    proof.journey.start_attempts += 1;
    const click = await clickAction("start");
    check(`${name}-click-accepted`, click.clicked, click);
    const sidebarStarting = waitFor(
      sidebarSnapshot,
      (value) => isStartingAction(value.action),
      `${name} sidebar Starting`,
      15_000
    );
    const statusStarting = waitFor(
      pageText,
      (value) => /truST:\s*Simulator starting/i.test(value),
      `${name} status bar Starting`,
      15_000
    );
    const canvasStarting = devicesOpen
      ? waitFor(
          canvasSnapshot,
          (value) =>
            isSimulatorVisualState(value.simulator, "starting", "Starting…"),
          `${name} canvas Starting`,
          15_000
        )
      : Promise.resolve(undefined);
    const sidebar = await sidebarStarting;
    const duplicate = await clickAction("busy");
    const duplicateBlocked = isBlockedStartingAction(duplicate);
    const [statusText, canvas] = await Promise.all([
      statusStarting,
      canvasStarting,
    ]);
    proof.journey.starting_states_observed += 1;
    if (duplicateBlocked) proof.journey.blocked_duplicate_start_clicks += 1;
    const state = {
      sidebar_action: sidebar.action,
      canvas_simulator: canvas?.simulator,
      status_bar_starting: /truST:\s*Simulator starting/i.test(statusText),
      duplicate_click: duplicate,
      active_tab_before: activeBefore,
      active_tab_during: activeTabLabel(),
    };
    check(
      `${name}-starting-is-one-disabled-attempt`,
      isStartingAction(sidebar.action) &&
        state.status_bar_starting &&
        duplicateBlocked &&
        (!devicesOpen ||
          isSimulatorVisualState(
            canvas?.simulator,
            "starting",
            "Starting…"
          )),
      state
    );
    return state;
  }

  async function waitForRunning(name, devicesOpen, expectedSessionCount) {
    const sidebar = await waitFor(
      sidebarSnapshot,
      (value) => value.action.state === "stop" && value.action.text === "Stop",
      `${name} sidebar Running`,
      60_000
    );
    const statusText = await waitFor(
      pageText,
      (value) => /truST:\s*Simulator running/i.test(value),
      `${name} status bar Running`,
      15_000
    );
    let canvas;
    if (devicesOpen) {
      canvas = await waitForSettledSimulatorCanvas(canvasSnapshot, {
        health: "connected",
        label: "Running",
        description: `${name} settled canvas Running`,
      });
    }
    const session = await waitFor(
      async () =>
        activeSession() ||
        (vscode.debug.activeDebugSession?.type === "structured-text"
          ? vscode.debug.activeDebugSession
          : undefined),
      Boolean,
      `${name} Structured Text debug session`,
      15_000
    );
    check(
      `${name}-debug-session-count`,
      proof.journey.debug_sessions_started === expectedSessionCount,
      { debug_sessions_started: proof.journey.debug_sessions_started }
    );
    return {
      evidence: {
        sidebar_action: sidebar.action,
        sidebar_target: sidebar.target,
        canvas_simulator: canvas?.simulator,
        canvas_text: canvas?.canvasText,
        status_bar_running: /truST:\s*Simulator running/i.test(statusText),
      },
      session,
    };
  }

  async function stopAndWait(name, expectedTerminationCount) {
    const click = await clickAction("stop");
    check(`${name}-click-accepted`, click.clicked, click);
    const sidebar = await waitFor(
      sidebarSnapshot,
      (value) => value.action.state === "start" && value.action.text === "Start",
      `${name} sidebar Stopped`,
      35_000
    );
    const canvas = await waitForSettledSimulatorCanvas(canvasSnapshot, {
      health: "stopped",
      label: "Stopped",
      description: `${name} settled canvas Stopped`,
    });
    const statusText = await waitFor(
      pageText,
      (value) => /truST:\s*Simulator stopped/i.test(value),
      `${name} status bar Stopped`,
      15_000
    );
    await waitFor(
      async () => proof.journey.debug_sessions_terminated,
      (count) => count === expectedTerminationCount,
      `${name} terminated Structured Text session`,
      15_000
    );
    return {
      sidebar_action: sidebar.action,
      sidebar_target: sidebar.target,
      sidebar_text: sidebar.bodyText,
      canvas_simulator: canvas.simulator,
      canvas_text: canvas.canvasText,
      status_bar_stopped: /truST:\s*Simulator stopped/i.test(statusText),
    };
  }

  return { beginStartAttempt, stopAndWait, waitForRunning };
}

module.exports = { createSimulatorLifecycle };
