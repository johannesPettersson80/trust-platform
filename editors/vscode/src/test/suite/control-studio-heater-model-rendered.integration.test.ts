import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { withStudio } from "./control-studio-three-page.integration.test";
import { processReading, type SimulatorLoop } from "./control-studio-simulator-runtime";
import {
  assertKeptRecordingComplete,
  channel,
  chooseProjectExperiment,
  PROJECT_TEST_COMPLETED,
  projectTestAnswer,
  reviewProjectTest,
  startProjectTest,
  waitForProjectTestEnd,
  waitForRecordedChart,
} from "./control-studio-project-test";
import { pollValue, requiredEnvironment, sleep } from "./plant-rendered-harness";

/**
 * Lesson 15's water heater: the held 85 % step of lesson 13, and a model that the project's program integrates beside the plant, whose
 * temperature is recorded as a signal of its own. This suite once held a lesson 13 case as well; the journey "Control Studio process
 * dynamics" proves that lesson's recording more closely (the 300 held scans, the delay, the checkpoints and the kept recording).
 */
suite("Control Studio heater model actual rendered workflow", () => {
  test("control-water-heater-energy-balance executes its held heater case and retains independent evidence", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_HEATER_MODEL !== "1") this.skip();
    this.timeout(300_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const fixture = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual(fixture.id, "control-water-heater-energy-balance");
    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    try {
      await withStudio(async (inner, _reopen, capture) => {
        const loop = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
        const authority = loop.signals.additional.find((signal: any) => signal.address === loop.experiment.activation.address)?.id;
        assert.ok(authority, "the project's activation is one of the loop's recorded signals");
        const plant: SimulatorLoop = loop;
        await vscode.commands.executeCommand("workbench.action.closeSidebar");
        await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
        const started = await runtimeLifecycleService.startLocalSimulator();
        assert.strictEqual(started.ok, true, JSON.stringify(started));

        // The heater rests hot before the test: 35 °C with 75 %, read from the runtime's own scan.
        await pollValue("fresh hot state", () => processReading(plant),
          reading => reading !== undefined && Math.abs(reading.pv - 35) < 1e-6 && reading.mv === 75, 15_000);
        await chooseProjectExperiment(inner);
        assert.match(await reviewProjectTest(inner), /30\.1 s/, "the review shows the declared 30.1 s duration");
        await startProjectTest(inner);
        assert.strictEqual(await waitForProjectTestEnd(inner, 90_000), PROJECT_TEST_COMPLETED);
        // The engine kept the run: the page draws its recording, and the recording file in the project holds every scan the runtime recorded.
        const answer = await projectTestAnswer(inner);
        assert.strictEqual(answer.restoration, "restored", "the engine names the restoration its campaign confirmed");
        const samples = answer.recordedSamples;
        await waitForRecordedChart(inner, answer);
        await vscode.commands.executeCommand("notifications.clearAll");
        await sleep(300);
        await capture("energy-balance");
        assertKeptRecordingComplete(project, answer);
        fs.writeFileSync(path.join(captures, "heater-energy-balance.json"), JSON.stringify({ recordingId: answer.recordingId, samples }, null, 2));

        const active = samples.filter(sample => channel(sample, authority) === 1);
        assert.ok(active.length >= 280, "the recording includes the project's activation");
        const held = active.filter(sample => sample.appliedMv === 85);
        assert.ok(held.length >= 298 && held.length <= 302, `expected a 30 s held 85% segment, got ${held.length} scans`);
        const lastActive = active[active.length - 1];
        const firstZero = samples.findIndex(sample => sample.appliedMv === 0 && sample.scan > lastActive.scan);
        assert.ok(firstZero >= 0, "restoration must command zero after the held window");
        assert.ok(samples.slice(firstZero).every(sample => sample.appliedMv === 0), "terminal restoration must remain at zero without reheating");
        assert.ok(active[0].pv! >= 34.9 && active[0].pv! <= 35.1, "the held step starts from the hot operating point");
        assert.ok(lastActive.pv! > 36.8 && lastActive.pv! < 37.1, "the held step ends near 37 °C");
        const model = active.map(sample => channel(sample, "model"));
        assert.ok(active.some((sample, index) => Math.abs(model[index] - sample.pv!) > 1e-4), "the Euler model must be recorded separately from the measured temperature");
        assert.ok(active.every((sample, index) => Math.abs(model[index] - sample.pv!) < 0.08), "the Euler model and the measured temperature must agree within the declared teaching tolerance");
      });
    } finally {
      await runtimeLifecycleService.stopRuntime();
    }
  });
});
