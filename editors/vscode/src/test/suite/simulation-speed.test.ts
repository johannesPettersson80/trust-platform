import * as assert from "assert";
import type { ControlStudioView } from "../../controlStudio/panelTemplate";
import { renderThreePageStudioHtml } from "../../controlStudio/threePageTemplate";
import {
  runtimeTargetFromStatus,
  simulationSpeedBehind,
  simulationSpeedText,
  simulationTargetLabel,
} from "../../controlStudio/runtimeTarget";
import { selectedStatusView } from "../../runtimeControls";
import {
  applyLiveSimulationSpeed,
  savedSimulationSpeed,
  SIMULATION_SPEEDS,
  speedChoices,
  withSimulationTimeScale,
} from "../../simulationSpeed";
import {
  selectedRuntime,
  type RemoteRuntime,
  type RuntimeModelSnapshot,
  type SelectedRuntime,
} from "../../trustHomeModel";

const SIMULATOR_ENDPOINT = "tcp://127.0.0.1:9";
const REMOTE_ENDPOINT = "tcp://10.0.0.5:7000";

function simulatorSnapshot(runtimeState: "running" | "stopped"): RuntimeModelSnapshot {
  return {
    runtimeMode: "simulate",
    runtimeState,
    endpoint: SIMULATOR_ENDPOINT,
    endpointConfigured: true,
    endpointReachable: true,
    starting: false,
  };
}

/** The target the status bar would select: the simulator, or a remote runtime with the given id. */
function selectTarget(snapshot: RuntimeModelSnapshot, selectedId: string, remotes: RemoteRuntime[] = []): SelectedRuntime {
  return selectedRuntime({ snapshot, remotes, managed: [], selectedId });
}

const HTML_ENTITIES: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" };

function escapeHtml(value: string): string {
  return value.replace(/[&<>"]/g, (character) => HTML_ENTITIES[character] ?? character);
}

suite("Simulation speed", () => {
  test("a missing simulation.toml becomes just the [simulation] table", () => {
    assert.strictEqual(withSimulationTimeScale(undefined, 10), "[simulation]\ntime_scale = 10\n");
    assert.strictEqual(withSimulationTimeScale("  \n", 5), "[simulation]\ntime_scale = 5\n");
  });

  test("a file without a [simulation] table gets one before the first table, after the leading comments", () => {
    const before = [
      "# Water heater plant",
      "# Edited by the project",
      "",
      "[[plants]]",
      'id = "tank"',
      'kind = "gravity_tank"',
      "",
      "[[couplings]]",
      'from = "pump"',
      'to = "tank"',
      "",
    ].join("\n");
    assert.strictEqual(
      withSimulationTimeScale(before, 10),
      [
        "# Water heater plant",
        "# Edited by the project",
        "",
        "[simulation]",
        "time_scale = 10",
        "",
        "[[plants]]",
        'id = "tank"',
        'kind = "gravity_tank"',
        "",
        "[[couplings]]",
        'from = "pump"',
        'to = "tank"',
        "",
      ].join("\n")
    );
  });

  test("an existing time_scale changes only its value and keeps its comment, enabled and [[plants]]", () => {
    const text = [
      "[simulation]",
      "enabled = true",
      "time_scale = 8   # faster than real time",
      "",
      "[[plants]]",
      'id = "tank"',
      "",
    ].join("\n");
    assert.strictEqual(
      withSimulationTimeScale(text, 20),
      [
        "[simulation]",
        "enabled = true",
        "time_scale = 20   # faster than real time",
        "",
        "[[plants]]",
        'id = "tank"',
        "",
      ].join("\n")
    );
  });

  test("a [simulation] table without time_scale gets the key directly after its header", () => {
    const text = "[simulation]\nenabled = true\n\n[[plants]]\nid = \"tank\"\n";
    assert.strictEqual(
      withSimulationTimeScale(text, 5),
      "[simulation]\ntime_scale = 5\nenabled = true\n\n[[plants]]\nid = \"tank\"\n"
    );
  });

  test("real time writes time_scale = 1 and keeps the table", () => {
    assert.strictEqual(withSimulationTimeScale("[simulation]\ntime_scale = 10\n", 1), "[simulation]\ntime_scale = 1\n");
  });

  test("CRLF line endings are kept", () => {
    assert.strictEqual(
      withSimulationTimeScale("[simulation]\r\ntime_scale = 2\r\n[[plants]]\r\nid = 1\r\n", 10),
      "[simulation]\r\ntime_scale = 10\r\n[[plants]]\r\nid = 1\r\n"
    );
  });

  test("the saved speed is read from [simulation] time_scale, with real time as the fallback", () => {
    assert.strictEqual(savedSimulationSpeed("[simulation]\ntime_scale = 20\n"), 20);
    assert.strictEqual(savedSimulationSpeed("[[plants]]\nid = \"tank\"\n"), 1);
    assert.strictEqual(savedSimulationSpeed("[simulation]\ntime_scale = 500\n"), 1);
    assert.strictEqual(savedSimulationSpeed(undefined), 1);
  });

  test("the quick pick offers the seven speeds and marks the current one", () => {
    assert.deepStrictEqual([...SIMULATION_SPEEDS], [1, 2, 5, 10, 20, 50, 100]);
    const choices = speedChoices(10);
    assert.deepStrictEqual(choices.map((choice) => choice.label), ["1×", "2×", "5×", "10×", "20×", "50×", "100×"]);
    assert.deepStrictEqual(
      choices.filter((choice) => choice.description === "current").map((choice) => choice.speed),
      [10]
    );
    assert.strictEqual(choices.find((choice) => choice.speed === 1)?.detail, "Real time");
  });

  test("config.set reaches the runtime only when a running simulator is the selected target", async () => {
    const sent: Array<{ endpoint: string; requestType: string; params: unknown }> = [];
    const send = async (endpoint: string, _authToken: string | undefined, requestType: string, params?: unknown) => {
      sent.push({ endpoint, requestType, params });
      return {};
    };
    const running = selectTarget(simulatorSnapshot("running"), "");
    assert.strictEqual(
      await applyLiveSimulationSpeed({ selected: running, endpoint: SIMULATOR_ENDPOINT, authToken: "token", speed: 10 }, send),
      "applied"
    );
    assert.deepStrictEqual(sent, [
      { endpoint: SIMULATOR_ENDPOINT, requestType: "config.set", params: { "simulation.time_scale": 10 } },
    ]);

    const stopped = selectTarget(simulatorSnapshot("stopped"), "");
    const remote = selectTarget(simulatorSnapshot("running"), REMOTE_ENDPOINT, [{ id: REMOTE_ENDPOINT, label: "10.0.0.5" }]);
    assert.strictEqual(
      await applyLiveSimulationSpeed({ selected: stopped, endpoint: SIMULATOR_ENDPOINT, authToken: undefined, speed: 10 }, send),
      "saved-only"
    );
    assert.strictEqual(
      await applyLiveSimulationSpeed({ selected: remote, endpoint: REMOTE_ENDPOINT, authToken: undefined, speed: 10 }, send),
      "saved-only"
    );
    assert.strictEqual(
      await applyLiveSimulationSpeed({ selected: running, endpoint: "", authToken: undefined, speed: 10 }, send),
      "saved-only"
    );
    assert.strictEqual(sent.length, 1, "no request may reach a runtime that is not a running selected simulator");
  });

  test("a runtime refusal reaches the caller with the runtime's message", async () => {
    const refuse = async () => {
      throw new Error("config_value_error: the runtime is not running a simulation");
    };
    await assert.rejects(
      applyLiveSimulationSpeed(
        { selected: selectTarget(simulatorSnapshot("running"), ""), endpoint: SIMULATOR_ENDPOINT, authToken: undefined, speed: 2 },
        refuse
      ),
      /the runtime is not running a simulation/
    );
  });

  test("the status bar names the simulator's speed while it runs", () => {
    const running = selectTarget(simulatorSnapshot("running"), "");
    const fast = selectedStatusView(running, { configured: 10 });
    assert.strictEqual(fast.text, "$(circle-filled) truST: Simulator ×10 running");
    assert.strictEqual(fast.simulatorRunning, true);
    assert.strictEqual(fast.speedBehind, false);

    const behind = selectedStatusView(running, { configured: 10, achieved: 7 });
    assert.strictEqual(behind.text, "$(circle-filled) truST: Simulator ×7 of ×10 running");
    assert.strictEqual(behind.speedBehind, true, "more than 5 % below must use the warning colour");

    const within = selectedStatusView(running, { configured: 10, achieved: 9.5 });
    assert.strictEqual(within.text, "$(circle-filled) truST: Simulator ×10 running", "exactly 5 % below is not behind");
    assert.strictEqual(within.speedBehind, false);

    assert.strictEqual(selectedStatusView(running, { configured: 1 }).text, "$(circle-filled) truST: Simulator running");
    assert.strictEqual(selectedStatusView(running).text, "$(circle-filled) truST: Simulator running");
    const realTimeBehind = selectedStatusView(running, { configured: 1, achieved: 0.9 });
    assert.strictEqual(realTimeBehind.text, "$(circle-filled) truST: Simulator running", "no achieved speed at 1x");
    assert.strictEqual(realTimeBehind.speedBehind, false, "no warning colour at 1x");

    const stopped = selectedStatusView(selectTarget(simulatorSnapshot("stopped"), ""), { configured: 10 });
    assert.strictEqual(stopped.text, "$(circle-outline) truST: Simulator stopped");
    assert.strictEqual(stopped.simulatorRunning, false);
  });

  test("one speed rule: real time shows no achieved speed; behind only above 1x and below 0.95 of N", () => {
    assert.strictEqual(simulationSpeedText({ configured: 1 }), "");
    assert.strictEqual(simulationSpeedText({ configured: 1, achieved: 0.9 }), "", "never ×0.9 of ×1");
    assert.strictEqual(simulationSpeedBehind({ configured: 1, achieved: 0.5 }), false, "no warning at 1x");
    assert.strictEqual(simulationSpeedText({ configured: 10 }), "×10");
    assert.strictEqual(simulationSpeedText({ configured: 10, achieved: 7 }), "×7 of ×10");
    assert.strictEqual(simulationSpeedText({ configured: 10, achieved: 9.5 }), "×10", "exactly 0.95 times N is not behind");
    assert.strictEqual(simulationSpeedText({ configured: 10, achieved: 9.45 }), "×9.4 of ×10", "shown values round down");
    assert.strictEqual(simulationSpeedText({ configured: 2, achieved: 1.8 }), "×1.8 of ×2");
    assert.strictEqual(simulationSpeedText({ configured: 2, achieved: 1.9 }), "×2");
    assert.strictEqual(simulationSpeedText({ configured: 100, achieved: 94.9 }), "×94.9 of ×100");
    assert.strictEqual(simulationSpeedBehind({ configured: 20, achieved: 18.9 }), true);
    assert.strictEqual(simulationSpeedBehind({ configured: 20, achieved: 19 }), false);
  });

  test("the Control Studio target names simulation and speed, or physical", () => {
    const simulated = (scale: number, achieved?: number) => runtimeTargetFromStatus({
      simulation_enabled: true,
      simulation_only_io: true,
      simulation_time_scale: scale,
      ...(achieved === undefined ? {} : { simulation_achieved_time_scale: achieved }),
    });
    assert.strictEqual(simulationTargetLabel(simulated(1)), "Simulation");
    assert.strictEqual(simulationTargetLabel(simulated(1, 0.9)), "Simulation", "no achieved speed at 1x");
    assert.strictEqual(simulationTargetLabel(simulated(10, 10)), "Simulation ×10");
    assert.strictEqual(simulationTargetLabel(simulated(10, 7)), "Simulation ×7 of ×10");
    assert.strictEqual(simulationTargetLabel(runtimeTargetFromStatus({ simulation_enabled: false })), "Physical");
  });

  test("the Control Studio page shows its target label only when one is given", () => {
    const view = {
      cspSource: "vscode-resource:",
      nonce: "nonce",
      chartScript: "chart.js",
      workflowScript: "workflow.js",
      project: "project",
      session: "session",
      revision: "revision",
      loop: "loop",
      loopRevision: "loop-revision",
      processUnit: "",
      outputUnit: "",
    } as unknown as ControlStudioView;
    const labelled = renderThreePageStudioHtml(view, escapeHtml, "Simulation ×7 of ×10");
    assert.ok(labelled.includes('data-studio-target'), "the target label element must be rendered");
    assert.ok(labelled.includes("Simulation ×7 of ×10"), "the label text must be rendered");
    const unlabelled = renderThreePageStudioHtml(view, escapeHtml);
    assert.ok(/data-studio-target[^>]*hidden/.test(unlabelled), "without a label the element is hidden, ready for a later label");
  });
});
