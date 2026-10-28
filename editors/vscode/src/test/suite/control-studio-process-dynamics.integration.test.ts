import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { runtimeLifecycleService } from "../../runtimeLifecycle";
import type { RecordedChartSample } from "../../controlStudio/generated/OpenResult";
import { withStudio } from "./control-studio-three-page.integration.test";
import { processReading, type SimulatorLoop } from "./control-studio-simulator-runtime";
import {
  applyRenderedTheme,
  assertKeptRecordingComplete,
  assertStudyReopened,
  channel,
  chooseProjectExperiment,
  PROJECT_TEST_COMPLETED,
  projectTestAnswer,
  reviewProjectTest,
  startProjectTest,
  waitForProjectTestEnd,
  waitForProjectTestRunning,
  waitForRecordedChart,
} from "./control-studio-project-test";
import { pollValue, requiredEnvironment, sleep } from "./plant-rendered-harness";

/** The sample nearest to `seconds` on the recording's own time base. */
function nearest(samples: readonly RecordedChartSample[], seconds: number): RecordedChartSample {
  assert.ok(samples.length > 0, "the held-input recording must contain samples");
  return samples.reduce((best, sample) =>
    Math.abs(sample.elapsedSeconds - seconds) < Math.abs(best.elapsedSeconds - seconds) ? sample : best);
}

/** Lesson 13's project, as the journey opens it: the open-loop observation whose program holds 85 % for 30 s after its activation. */
function assertLesson13(project: string): void {
  const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
  assert.strictEqual(loop.id, "control-lesson-13");
  assert.strictEqual(loop.topology, "open-loop");
  assert.strictEqual(loop.controller.type, "Main", "this is the project-owned open-loop process observation");
  assert.strictEqual(loop.signals.processVariable.address, "%IL0");
  assert.strictEqual(loop.signals.manipulatedVariable.address, "%QL0");
  assert.strictEqual(loop.experiment.durationSeconds, 30.1);
}

suite("Control Studio process dynamics", () => {
  test("runs, keeps and reopens the published thirty-second held-input observation", async function () {
    if (process.env.TRUST_PROCESS_DYNAMICS !== "1" || process.env.TRUST_PROCESS_DYNAMICS_UI_ONLY === "1") this.skip();
    this.timeout(300_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await applyRenderedTheme();
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    assertLesson13(project);
    try {
      await withStudio(async (inner, reopen, capture) => {
        const loop: SimulatorLoop = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
        const shoot = async (name: string) => {
          await vscode.commands.executeCommand("notifications.clearAll");
          await sleep(300);
          await capture(name);
        };
        await vscode.commands.executeCommand("workbench.action.closeSidebar");
        await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
        assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);

        // The heater rests hot before the test: 35 °C with 75 % (lesson 13, step 1), read from the runtime's own scan.
        await pollValue("prepared hot process", () => processReading(loop),
          reading => reading !== undefined && Math.abs(reading.pv - 35) < 1e-6 && reading.mv === 75, 15_000);
        await chooseProjectExperiment(inner);
        const duration = await reviewProjectTest(inner);
        assert.match(duration, /30\.1 s/, "the review shows the declared 30.1 s duration");
        await shoot("01-reviewed-held-input");

        await startProjectTest(inner);
        await waitForProjectTestRunning(inner);
        // The runtime is not read while the test runs: the engine's sampler and recording share the runtime's one inspection watch, and
        // the held input is in the kept recording below.
        await shoot("02-held-input-running");

        assert.strictEqual(await waitForProjectTestEnd(inner, 60_000), PROJECT_TEST_COMPLETED);
        // The runtime restored the test's output, and the page shows the recording the engine kept.
        await pollValue("restored output", () => processReading(loop), reading => reading?.mv === 0, 10_000);
        const answer = await projectTestAnswer(inner);
        assert.strictEqual(answer.restoration, "restored", "the engine names the restoration its campaign confirmed");
        const samples = answer.recordedSamples;
        await waitForRecordedChart(inner, answer);
        assert.match(await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label');"), /°C/, "the recorded measurement is drawn in its unit");
        assert.match(await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label');"), /%/, "the recorded output is drawn in its unit");
        await shoot("03-complete-held-input");

        // The engine kept the run: its answer carries the samples, and its recording file in the project is complete.
        const file = assertKeptRecordingComplete(project, answer);
        fs.writeFileSync(path.join(captures, "process-dynamics-answer.json"),
          JSON.stringify({ recordingId: answer.recordingId, restoration: answer.restoration, modelType: answer.modelType, fitRefusal: answer.fitRefusal,
            acquisition: file.run.acquisition, evidence: file.evidence, samples }, null, 2));

        const active = samples.filter(sample => channel(sample, "experiment-authority") === 1);
        assert.ok(active.length > 0, "the native record must include project activation");
        const onset = active[0].elapsedSeconds;
        const held = active.filter(sample => sample.elapsedSeconds - onset < 30 - 1e-9);
        assert.strictEqual(held.length, 300, "the project must retain all 300 native 100 ms scans of its 30 s hold");
        assert.ok(held.every(sample => sample.appliedMv === 85), "the project must hold the declared 85 percent input");
        // The engine plans the declared 30.1 s (specification 35, B3.4 step 4 and C12). The runtime counts a duration to the execution at which
        // the restoration takes effect, so the activation is in force for the 300 scans of the 30 s hold and is restored in the sample after
        // them, the restoration sample, in which the program has ended its hold (0 %) and the activation is off.
        const restoration = samples.find(sample => Math.abs(sample.elapsedSeconds - onset - 30) < 1e-9);
        assert.ok(restoration && channel(restoration, "experiment-authority") === 0 && restoration.appliedMv === 0,
          "the recording ends with its one restoration sample at 30.0 s: the program has ended its 30 s hold and the host has restored the activation");
        assert.ok(active.slice(0, 9).every(sample => Math.abs(sample.pv! - 35) < 1e-8), "the declared one-second process delay must be visible");
        assert.ok(Math.abs(nearest(active, onset + 6).pv! - 36.264) < 0.04, "the six-second native process checkpoint must be retained");
        assert.ok(Math.abs(nearest(active, onset + 26).pv! - 36.986) < 0.01, "the twenty-six-second native process checkpoint must be retained");
        const restored = samples.filter(sample => channel(sample, "experiment-authority") === 0);
        assert.ok(restored.some(sample => sample.appliedMv === 0), "the recording must include terminal zero output");

        // Reopening shows the study the engine kept: the identified model when the recording supported one, and none when it did not.
        await reopen();
        await assertStudyReopened(inner, answer);
        await shoot("04-reopened-observation");
      });
    } finally {
      await runtimeLifecycleService.stopRuntime();
    }
  });

  test("keeps the retained recording of a short project test on the chart while live values keep arriving", async function () {
    if (process.env.TRUST_PROCESS_DYNAMICS !== "1" || process.env.TRUST_PROCESS_DYNAMICS_UI_ONLY !== "1") this.skip();
    this.timeout(180_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await applyRenderedTheme();
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    assertLesson13(project);
    try {
      // The short display scenario: the same lesson, with the declared duration shortened to 1.1 s.
      await withStudio(async (inner, _reopen, capture) => {
        const loop: SimulatorLoop = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
        const shoot = async (name: string) => {
          await vscode.commands.executeCommand("notifications.clearAll");
          await sleep(300);
          await capture(name);
        };
        await vscode.commands.executeCommand("workbench.action.closeSidebar");
        await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
        assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
        await pollValue("prepared hot process", () => processReading(loop),
          reading => reading !== undefined && Math.abs(reading.pv - 35) < 1e-6 && reading.mv === 75, 15_000);
        // The chart shows the live signals before the test: the measurement and the applied output.
        await pollValue("live readings", () => inner("return d.querySelector('[data-control-studio-chart]').dataset.pointCount;"), value => Number(value) >= 8, 15_000);
        await shoot("01-live-before-test");

        await chooseProjectExperiment(inner);
        assert.match(await reviewProjectTest(inner), /1\.1 s/, "the review shows the shortened duration");
        await startProjectTest(inner);
        assert.strictEqual(await waitForProjectTestEnd(inner, 60_000), PROJECT_TEST_COMPLETED);
        const answer = await projectTestAnswer(inner);
        assert.ok(answer.recordedSamples.length >= 10, "the short display scenario must retain actual native samples");
        await waitForRecordedChart(inner, answer);
        await shoot("02-retained-short-observation");

        // Live values keep arriving while the process cools, and they neither extend nor replace the retained recording.
        const chart = "return JSON.stringify({points:d.querySelector('[data-control-studio-chart]').dataset.pointCount,range:d.querySelector('[data-control-studio-chart]').dataset.timeRange,series:d.querySelector('[data-control-studio-chart]').dataset.seriesCount});";
        await inner("d.defaultView.__monitorSnapshots=0;d.defaultView.addEventListener('message',event=>{if(event.data&&event.data.kind==='loop.monitor.snapshot')d.defaultView.__monitorSnapshots++;});return 'counting';");
        const before = await inner(chart);
        await sleep(5000);
        assert.ok(Number(await inner("return String(d.defaultView.__monitorSnapshots);")) > 0, "live samples continue to arrive");
        assert.strictEqual(await inner(chart), before, "live cooling must not extend or replace the retained recording");
        await pollValue("restored output", () => processReading(loop), reading => reading?.mv === 0, 10_000);
        fs.writeFileSync(path.join(captures, "process-dynamics-short-chart.json"), before);
        await shoot("03-retained-while-live");
      }, { loop: loop => { loop.experiment.durationSeconds = 1.1; } });
    } finally {
      await runtimeLifecycleService.stopRuntime();
    }
  });
});
