import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { renderControlStudioHtml, type ControlStudioView } from "../../controlStudio/panelTemplate";
import { applyStudioTargetLabel } from "../../controlStudio/webview/studioTargetLabel";
import type { CanvasFleet } from "../../networkCanvas/fleetModel";
import { canvasGraph } from "../../networkCanvas/graphData";
import {
  announceSimulationSpeedRead,
  latestSimulatorSpeedRead,
  onDidReadSimulationSpeed,
  type SimulationSpeedRead,
} from "../../simulationSpeed";
import { SIMULATOR_RUNTIME_ID } from "../../trustHomeModel";

const extensionRoot = path.resolve(__dirname, "..", "..", "..");

function readSource(...segments: string[]): string {
  return fs.readFileSync(path.join(extensionRoot, "src", ...segments), "utf8");
}

suite("Control Studio final wiring", () => {
  test("a status read announces the runtime's speed and keeps the simulator's latest read", () => {
    const seen: SimulationSpeedRead[] = [];
    const subscription = onDidReadSimulationSpeed(read => { seen.push(read); });
    try {
      const read: SimulationSpeedRead = { runtimeId: SIMULATOR_RUNTIME_ID, simulated: true, configured: 10, achieved: 7,
        speedText: "×7 of ×10", speedBehind: true };
      announceSimulationSpeedRead(read);
      assert.deepStrictEqual(seen, [read], "each announced read reaches the subscriber once");
      assert.deepStrictEqual(latestSimulatorSpeedRead(), read, "the canvas reads the simulator's latest status");
    } finally {
      subscription.dispose();
    }
  });

  test("the speed event comes from the status-bar read and adds no timer of its own", () => {
    const runtimeControls = readSource("runtimeControls.ts");
    assert.ok(runtimeControls.includes("announceSimulationSpeedRead({ runtimeId: selected.id, ...speed })"),
      "the status-bar read announces every status it gets");
    assert.strictEqual(runtimeControls.split("setInterval(").length - 1, 1, "the status bar keeps its one existing speed timer");
    assert.ok(runtimeControls.includes("SPEED_REFRESH_INTERVAL_MS"), "the timer keeps the existing 1.5 s cadence");
    assert.ok(!readSource("simulationSpeed.ts").includes("setInterval("), "the speed module adds no timer");
  });

  test("the label message uses the envelope and scope fields the other host messages use", () => {
    const panelSource = readSource("controlStudio", "panel.ts");
    assert.ok(panelSource.includes('kind: "control-studio.target-label"'), "the label has its own message kind");
    assert.ok(panelSource.includes("expectedLoopRevision: loop.revision") && panelSource.includes("editorSessionId, loopId: loop.id"),
      "the label carries the loop scope the page checks");
  });

  test("the Control Studio page renders the label it is given and updates only that element", () => {
    const view: ControlStudioView = {
      cspSource: "vscode-resource:", nonce: "nonce", chartScript: "chart.js", workflowScript: "workflow.js",
      project: "project", session: "session", revision: "1", loop: "loop", loopRevision: "1", processUnit: "", outputUnit: "",
      targetLabel: "Simulation ×10",
    };
    const html = renderControlStudioHtml(view);
    assert.ok(html.includes("data-studio-target") && html.includes("Simulation ×10"), "the template renders the initial label");
    const unlabelled = renderControlStudioHtml({ ...view, targetLabel: undefined });
    assert.ok(/data-studio-target[^>]*hidden/.test(unlabelled),
      "without a label the element still exists, hidden, so a later label has a place");

    const element = { textContent: "Simulation ×10" };
    const root = { querySelector: (selector: string) => (selector === "[data-studio-target]" ? element : null) };
    assert.strictEqual(applyStudioTargetLabel(root, { label: "Simulation ×7 of ×10" }), true);
    assert.strictEqual(element.textContent, "Simulation ×7 of ×10", "the page sets the label text");
    assert.strictEqual(applyStudioTargetLabel({ querySelector: () => null }, { label: "Physical" }), false,
      "a page without the label element has nothing to update");
    assert.strictEqual(applyStudioTargetLabel(root, { label: 7 }), false, "a label must be text");
    assert.strictEqual(element.textContent, "Simulation ×7 of ×10", "a refused label leaves the element as it was");

    const hostState = readSource("controlStudio", "webview", "threePageHostState.ts");
    assert.ok(hostState.includes('message.kind === "control-studio.target-label"') && hostState.includes("applyStudioTargetLabel(root, message)"),
      "the page's host listener routes the label message to its element");
  });

  test("the simulator node carries the speed while it runs and none once it stops", () => {
    announceSimulationSpeedRead({ runtimeId: SIMULATOR_RUNTIME_ID, simulated: true, configured: 10, achieved: 7,
      speedText: "×7 of ×10", speedBehind: true });
    const running = canvasGraph(simulatorFleet("connected"));
    const runningNode = running.hosts[0].runtimes[0] as unknown as { simulationTimeScale?: number; simulationAchievedTimeScale?: number;
      simulationSpeedText?: string };
    assert.strictEqual(runningNode.simulationTimeScale, 10, "the configured speed reaches the Mode row");
    assert.strictEqual(runningNode.simulationAchievedTimeScale, 7, "the achieved speed reaches the Mode row");
    assert.strictEqual(runningNode.simulationSpeedText, "×7 of ×10", "the speed words the runtime gives reach the Mode row");

    const stopped = canvasGraph(simulatorFleet("stopped"));
    const stoppedNode = stopped.hosts[0].runtimes[0] as unknown as { simulationTimeScale?: number; simulationAchievedTimeScale?: number;
      simulationSpeedText?: string };
    assert.strictEqual(stoppedNode.simulationTimeScale, undefined, "a stopped simulator shows no speed");
    assert.strictEqual(stoppedNode.simulationAchievedTimeScale, undefined, "a stopped simulator shows no achieved speed");
    assert.strictEqual(stoppedNode.simulationSpeedText, undefined, "a stopped simulator shows no speed words");
  });
});

/** The canvas's answer with the drawn simulator alone, `health` as the server words its lifecycle. */
function simulatorFleet(health: string): CanvasFleet {
  return {
    hosts: [
      {
        id: "host:this-computer",
        hostname: "This computer",
        label: "local computer",
        health: "connected",
        runtimeCount: 1,
        endpointCount: 0,
        containers: [],
        runtimes: [
          {
            id: "runtime:local",
            name: "Simulator",
            mode: "simulate",
            health,
            detail: "",
            endpointCount: 0,
            endpoints: [],
            managed: false,
            attached: health === "connected",
            runTarget: true,
            identify: { enabled: false },
            controls: [],
            discovery: { ready: health === "connected" },
          },
        ],
      },
    ],
    links: [],
    external: [],
    externalCount: 0,
    sharedCount: 0,
    summary: "1 host · 1 runtime · 0 endpoints",
    faults: [],
  };
}
