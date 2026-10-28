import * as assert from "assert";
import type { ControlStudioView } from "../../controlStudio/panelTemplate";
import { renderThreePageStudioHtml } from "../../controlStudio/threePageTemplate";
import { selectedStatusView } from "../../runtimeControls";
import { readRuntimeSpeed, sendLiveSpeed, speedChoices } from "../../simulationSpeed";
import type { SelectedRuntime } from "../../trustHomeModel";
import { fakeRuntime, runtimeCommandsReady } from "./fakeControlRuntime";

const SIMULATOR_ENDPOINT = "tcp://127.0.0.1:9";

/** The Simulator as the language server's selection names it (`trust-lsp.runTargets`, action `selection`). */
function simulator(status: "running" | "stopped"): SelectedRuntime {
  const running = status === "running";
  return {
    id: "simulator",
    label: "Simulator",
    kind: "simulator",
    status,
    statusLabel: running ? "Running" : "Stopped",
    primary: running
      ? { action: "stop", label: "Stop", enabled: true, gateable: false }
      : { action: "start", label: "Start", enabled: true, gateable: true },
  };
}

const HTML_ENTITIES: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" };

function escapeHtml(value: string): string {
  return value.replace(/[&<>"]/g, (character) => HTML_ENTITIES[character] ?? character);
}

suite("Simulation speed", () => {
  // The simulation.toml edit, the saved speed, the offered speeds and the rule of where a saved speed goes are the language
  // server's (trust-lsp.simulationSpeed; crates/trust-lsp/src/handlers/commands/project/simulation_speed_tests.rs). These
  // tests prove the extension's part: the chooser's labels and forwarding the server's request to the runtime.
  test("the quick pick labels the speeds the server offers and marks the current one", () => {
    const choices = speedChoices(10, [1, 2, 5, 10, 20, 50, 100]);
    assert.deepStrictEqual(choices.map((choice) => choice.label), ["1×", "2×", "5×", "10×", "20×", "50×", "100×"]);
    assert.deepStrictEqual(
      choices.filter((choice) => choice.description === "current").map((choice) => choice.speed),
      [10]
    );
    assert.strictEqual(choices.find((choice) => choice.speed === 1)?.detail, "Real time");
  });

  test("the request the server names after a save reaches the running simulator unchanged", async () => {
    const sent: Array<{ endpoint: string; authToken: string | undefined; requestType: string; params: unknown }> = [];
    const send = async (endpoint: string, authToken: string | undefined, requestType: string, params?: unknown) => {
      sent.push({ endpoint, authToken, requestType, params });
      return {};
    };
    await sendLiveSpeed(
      { request: "config.set", params: { "simulation.time_scale": 10 } },
      SIMULATOR_ENDPOINT,
      "token",
      send
    );
    assert.deepStrictEqual(sent, [
      {
        endpoint: SIMULATOR_ENDPOINT,
        authToken: "token",
        requestType: "config.set",
        params: { "simulation.time_scale": 10 },
      },
    ]);
  });

  test("a runtime refusal reaches the caller with the runtime's message", async () => {
    const refuse = async () => {
      throw new Error("config_value_error: the runtime is not running a simulation");
    };
    await assert.rejects(
      sendLiveSpeed({ request: "config.set", params: { "simulation.time_scale": 2 } }, SIMULATOR_ENDPOINT, undefined, refuse),
      /the runtime is not running a simulation/
    );
  });

  test("the status bar names the simulator's speed in the words the runtime gives while it runs", () => {
    // The speed words and the behind flag are the runtime's (speed_tests.rs in the runtime pins the rule); the bar shows them.
    const running = simulator("running");
    const fast = selectedStatusView(running, { speedText: "×10", speedBehind: false });
    assert.strictEqual(fast.text, "$(circle-filled) truST: Simulator ×10 running");
    assert.strictEqual(fast.simulatorRunning, true);
    assert.strictEqual(fast.speedBehind, false);

    const behind = selectedStatusView(running, { speedText: "×7 of ×10", speedBehind: true });
    assert.strictEqual(behind.text, "$(circle-filled) truST: Simulator ×7 of ×10 running");
    assert.strictEqual(behind.speedBehind, true, "the runtime says the speed is behind: the warning colour");

    assert.strictEqual(selectedStatusView(running, { speedText: "", speedBehind: false }).text, "$(circle-filled) truST: Simulator running",
      "the runtime gives no words at 1x");
    assert.strictEqual(selectedStatusView(running).text, "$(circle-filled) truST: Simulator running");

    const stopped = selectedStatusView(simulator("stopped"), { speedText: "×10", speedBehind: false });
    assert.strictEqual(stopped.text, "$(circle-outline) truST: Simulator stopped");
    assert.strictEqual(stopped.simulatorRunning, false);
  });

  test("a simulator held at a breakpoint is paused in the status bar, not running", async function () {
    // Acceptance finding F4: the bar said "Simulator ×10 running" while the call stack said "PAUSED ON BREAKPOINT". The
    // runtime's status says `state: "paused"` during a debugger hold (status_handlers.rs); the language server reads it
    // (`trust-lsp.runTargets`, action `speed`) and the bar shows that state.
    this.timeout(60000);
    await runtimeCommandsReady();
    let report: Record<string, unknown> = {
      state: "paused", simulated: true, simulation_enabled: true, simulation_time_scale: 10,
      simulation_speed_text: "×10", simulation_speed_behind: true,
    };
    const runtime = await fakeRuntime(() => ({ ok: true, result: report }));
    try {
      const held = await readRuntimeSpeed(runtime.endpoint, undefined);
      assert.strictEqual(held?.paused, true);
      const running = simulator("running");
      const paused = selectedStatusView(running, held);
      assert.strictEqual(paused.text, "$(debug-pause) truST: Simulator ×10 paused");
      assert.strictEqual(paused.speedBehind, false, "a held scan is not behind its speed");
      assert.strictEqual(paused.simulatorRunning, true, "the bar keeps reading, so Continue shows running again");
      report = { ...report, state: "running", simulation_speed_behind: false };
      const resumed = await readRuntimeSpeed(runtime.endpoint, undefined);
      assert.strictEqual(selectedStatusView(running, resumed).text, "$(circle-filled) truST: Simulator ×10 running");
    } finally {
      await runtime.close();
    }
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
