import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import type { LoopRecording } from "../../controlStudio/loopMonitor";
import { ExperimentRecordingOwner, stopExperimentRecording } from "../../controlStudio/experimentRecording";
import { renderControlStudioHtml, type ControlStudioView } from "../../controlStudio/panelTemplate";
import { refreshTestExperiment, type TestExperimentLifecycle } from "../../controlStudio/testExperimentLifecycle";
import { applyStudioTargetLabel } from "../../controlStudio/webview/studioTargetLabel";
import { buildCanvasGraph } from "../../networkCanvas/graphData";
import { buildNetworkCanvasModel } from "../../networkCanvas/model";
import { RuntimeControlError } from "../../runtimeControlClient";
import {
  announceSimulationSpeedRead,
  latestSimulatorSpeedRead,
  onDidReadSimulationSpeed,
  targetLabelUpdates,
  type SimulationSpeedRead,
} from "../../simulationSpeed";
import { SIMULATOR_RUNTIME_ID } from "../../trustHomeModel";

const extensionRoot = path.resolve(__dirname, "..", "..", "..");

function readSource(...segments: string[]): string {
  return fs.readFileSync(path.join(extensionRoot, "src", ...segments), "utf8");
}

function testLoop(): ControlLoopDefinition {
  return { id: "loop:test", projectId: "project", runtimeId: SIMULATOR_RUNTIME_ID, revision: "1" } as unknown as ControlLoopDefinition;
}

/** A Test lifecycle whose identity the runtime does not hold ("absent"), with the start landing or settled as the caller says. */
function absentTestLifecycle(landing: () => boolean, log: string[]): TestExperimentLifecycle {
  return {
    experimentId: () => "exp-1",
    loop: () => testLoop(),
    recovery: () => undefined,
    recordingActive: () => false,
    recoverOwner: async () => { log.push("recover"); },
    recovered: () => { log.push("recovered"); },
    status: () => Promise.reject(new RuntimeControlError("no such experiment", "experiment_not_found")),
    classify: () => "invalid",
    terminal: async () => undefined,
    terminalObserved: () => undefined,
    clear: async () => { log.push("clear"); },
    post: async phase => { log.push(`post:${phase}`); },
    schedule: () => { log.push("schedule"); },
    startLanding: () => landing(),
  };
}

suite("Control Studio final wiring", () => {
  test("a status read announces the runtime's speed and keeps the simulator's latest read", () => {
    const seen: SimulationSpeedRead[] = [];
    const subscription = onDidReadSimulationSpeed(read => { seen.push(read); });
    try {
      const read: SimulationSpeedRead = { runtimeId: SIMULATOR_RUNTIME_ID, simulated: true, configured: 10, achieved: 7 };
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

  test("the label message is posted only when its text changes, and only for the pinned runtime", () => {
    const posted: string[] = [];
    const update = targetLabelUpdates(SIMULATOR_RUNTIME_ID, "Simulation ×10", label => { posted.push(label); });
    update({ runtimeId: "other-runtime", simulated: true, configured: 2, achieved: 1 });
    update({ runtimeId: SIMULATOR_RUNTIME_ID, simulated: true, configured: 10 });
    update({ runtimeId: SIMULATOR_RUNTIME_ID, simulated: true, configured: 10, achieved: 7 });
    update({ runtimeId: SIMULATOR_RUNTIME_ID, simulated: true, configured: 10, achieved: 7 });
    update({ runtimeId: SIMULATOR_RUNTIME_ID, simulated: false, configured: 1 });
    assert.deepStrictEqual(posted, ["Simulation ×7 of ×10", "Physical"]);
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
    announceSimulationSpeedRead({ runtimeId: SIMULATOR_RUNTIME_ID, simulated: true, configured: 10, achieved: 7 });
    const running = buildCanvasGraph(buildNetworkCanvasModel({
      stage: "runtime_live",
      runtime: { running: true, runtimeState: "running", runtimeMode: "simulate" },
    }), undefined);
    const runningNode = running.hosts[0].runtimes[0] as unknown as { simulationTimeScale?: number; simulationAchievedTimeScale?: number };
    assert.strictEqual(runningNode.simulationTimeScale, 10, "the configured speed reaches the Mode row");
    assert.strictEqual(runningNode.simulationAchievedTimeScale, 7, "the achieved speed reaches the Mode row");

    const stopped = buildCanvasGraph(buildNetworkCanvasModel({
      stage: "runtime_live",
      runtime: { running: false, runtimeState: "stopped", runtimeMode: "simulate" },
    }), undefined);
    const stoppedNode = stopped.hosts[0].runtimes[0] as unknown as { simulationTimeScale?: number; simulationAchievedTimeScale?: number };
    assert.strictEqual(stoppedNode.simulationTimeScale, undefined, "a stopped simulator shows no speed");
    assert.strictEqual(stoppedNode.simulationAchievedTimeScale, undefined, "a stopped simulator shows no achieved speed");
  });

  test("a Test recording is kept in the study before its runtime buffer is released", async () => {
    const loop = testLoop();
    const events: string[] = [];
    const recording = { rawSamples: [] } as unknown as LoopRecording;
    const owner = new ExperimentRecordingOwner(async requestedLoop => ({
      loop: requestedLoop,
      stop: async () => { events.push("stop"); return recording; },
      release: async () => { events.push("release"); },
    }));
    await owner.start(loop);
    const kept = await stopExperimentRecording(owner, loop, async stored => {
      events.push(stored === recording ? "retain" : "retain-other");
    });
    assert.strictEqual(kept, recording);
    assert.deepStrictEqual(events, ["stop", "retain", "release"], "stop, then keep in the study, then release");
    assert.strictEqual(owner.recording, false);
  });

  test("a failed keep leaves the runtime buffer for the next Stop", async () => {
    const loop = testLoop();
    const released: string[] = [];
    const owner = new ExperimentRecordingOwner(async requestedLoop => ({
      loop: requestedLoop,
      stop: async () => ({ rawSamples: [] } as unknown as LoopRecording),
      release: async () => { released.push("release"); },
    }));
    await owner.start(loop);
    await assert.rejects(() => stopExperimentRecording(owner, loop, async () => { throw new Error("study is full"); }), /study is full/);
    assert.deepStrictEqual(released, [], "nothing is released before the study keeps the samples");
    assert.strictEqual(owner.recording, true, "the owner stays, so the next Stop retries the keep");
  });

  test("an absent identity whose start is still landing keeps its journal", async () => {
    const landing: string[] = [];
    await refreshTestExperiment(absentTestLifecycle(() => true, landing));
    assert.deepStrictEqual(landing, [], "an unanswered start is not treated as never started: nothing is cleared or posted");

    const settled: string[] = [];
    await refreshTestExperiment(absentTestLifecycle(() => false, settled));
    assert.deepStrictEqual(settled, ["clear", "post:failed"], "once the start has failed or been answered, absent means it never started");
  });

  test("the pending-start guard and the study retention are wired on the Test page", () => {
    const panelSource = readSource("controlStudio", "panel.ts");
    assert.ok(panelSource.includes("startLanding: experimentId => landingTestStart === experimentId"),
      "the Test lifecycle asks whether the start is still landing");
    assert.ok(panelSource.includes("landingTestStart = plan.id;") && panelSource.includes(".finally(() => { landingTestStart = undefined; })"),
      "the identity is marked landing until the start settles");
    assert.ok(panelSource.includes("return stopExperimentRecording(experimentRecorder, loop, async kept => {"),
      "the Test stop hands the study's retention to the recording owner");
    assert.ok(!panelSource.includes("relayTuningSession") && !panelSource.includes("RelayTuningSession"),
      "the no-op relay session and its resets are gone");
    assert.ok(!readSource("controlStudio", "relayTuning.ts").includes("RelayTuningSession"), "the relay session class is removed");
    assert.ok(!readSource("controlStudio", "experimentRecording.ts").includes("Manual Record"),
      "the stale Manual Record comment is removed");
  });
});
