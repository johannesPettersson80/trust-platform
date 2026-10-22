import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { withStudio } from "./control-studio-three-page.integration.test";
import { processReading, type SimulatorLoop } from "./control-studio-simulator-runtime";
import {
  applyRenderedTheme,
  assertKeptRecordingComplete,
  assertStudyReopened,
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

/** The temperature a lesson publishes for a window of the run, in seconds from the project's activation, and how closely the recording must keep it. */
interface PublishedWindow {
  readonly startSeconds: number;
  readonly endSeconds: number;
  readonly temperature: number;
}

/**
 * The 120.1 s warming and load experiment of a water heater lesson, as the lesson's own project test runs it (README, Experiment in
 * Control Studio): 60 s of warming and holding, then 60 s with 30 kW more heat loss. The lesson publishes the temperatures of the
 * windows 50 to 60 s and 110 to 120 s; the journey checks the recording the engine kept against them.
 */
async function recordsPublishedExperiment(windows: readonly PublishedWindow[], tolerance: number): Promise<void> {
  await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
  await applyRenderedTheme();
  const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
  const fixture = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
  assert.strictEqual(fixture.controller.type, "Control.PID");
  assert.strictEqual(fixture.signals.processVariable.address, "%IL0");
  assert.strictEqual(fixture.experiment.durationSeconds, 120.1);
  const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
  try {
    await withStudio(async (inner, reopen, capture) => {
      const loop = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
      const shoot = async (name: string) => {
        await vscode.commands.executeCommand("notifications.clearAll");
        await sleep(300);
        await capture(name);
      };
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const started = await runtimeLifecycleService.startLocalSimulator();
      assert.strictEqual(started.ok, true, JSON.stringify(started));

      // The water rests at 20 °C with the heater command at 0 % before the test (README, step 1), read from the runtime's own scan.
      const plant: SimulatorLoop = loop;
      await pollValue("fresh heater measurement", () => processReading(plant),
        value => value !== undefined && Math.abs(value.pv - 20) < 1e-6 && value.mv === 0, 15_000);
      await shoot("01-idle");
      await chooseProjectExperiment(inner);
      assert.match(await reviewProjectTest(inner), /120\.1 s/, "the review shows the declared 120.1 s duration");
      await shoot("02-review");

      await startProjectTest(inner);
      await waitForProjectTestRunning(inner);
      // The runtime is not read while the test runs (the engine's sampler and recording share its one inspection watch): the warming is in
      // the kept recording below.
      await shoot("03-running");
      assert.strictEqual(await waitForProjectTestEnd(inner, 240_000), PROJECT_TEST_COMPLETED);
      await shoot("04-completed");

      // The engine kept the experiment: the page draws the recording, and the recording file in the project holds every scan.
      const answer = await projectTestAnswer(inner);
      assert.strictEqual(answer.restoration, "restored", "the engine names the restoration its campaign confirmed");
      await waitForRecordedChart(inner, answer);
      const file = assertKeptRecordingComplete(project, answer);
      const authority = loop.signals.additional.find((signal: any) => signal.address === loop.experiment.activation.address)?.id;
      assert.ok(authority, "the project's activation is one of the loop's recorded signals");
      const active = answer.recordedSamples.filter(sample => sample.additional?.some(signal => signal.id === authority && signal.value === 1));
      assert.ok(active.length >= 1190, "retain the actual 120-second experiment");
      const inWindow = (start: number, end: number) => active.filter(sample =>
        sample.elapsedSeconds - active[0].elapsedSeconds >= start && sample.elapsedSeconds - active[0].elapsedSeconds < end);
      for (const { startSeconds, endSeconds, temperature } of windows) {
        const window = inWindow(startSeconds, endSeconds);
        assert.ok(window.length >= 99, `retain the ${startSeconds}–${endSeconds}s assessment window at the 0.1s task interval`);
        assert.ok(window.every(sample => Math.abs(sample.pv! - temperature) <= tolerance),
          `the actual temperatures of ${startSeconds}–${endSeconds}s satisfy the published comparison of ${temperature} °C`);
      }
      fs.writeFileSync(path.join(captures, "heater-answer.json"), JSON.stringify({ recordingId: answer.recordingId, restoration: answer.restoration,
        modelType: answer.modelType, fitRefusal: answer.fitRefusal, acquisition: file.run.acquisition, evidence: file.evidence, samples: answer.recordedSamples }, null, 2));

      // The next test can be reviewed again; the study the engine kept comes back with the reopened panel.
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-find]').disabled);"), "false", "the next project test can be reviewed");
      await reopen();
      await assertStudyReopened(inner, answer);
      await shoot("05-reopened");
    });
  } finally {
    await runtimeLifecycleService.stopRuntime();
  }
}

suite("Control Studio actual PID heater lesson", () => {
  // Lesson 4 (P control): the water settles below the setpoint, and further below after the extra heat loss (README, Evidence).
  test("records the published proportional warming and load experiment through ordinary I/O", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_HEATER_PID_LESSON !== "1") this.skip();
    this.timeout(480_000);
    await recordsPublishedExperiment([
      { startSeconds: 50, endSeconds: 60, temperature: 24.285714 },
      { startSeconds: 110, endSeconds: 120, temperature: 22.142857 },
    ], 0.05);
  });
  // Lesson 5 (PI control): the integral term removes the shortfall, so both windows hold the 35 °C setpoint (README, requirement).
  test("records the published proportional-integral warming and load experiment through ordinary I/O", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_HEATER_PID_LESSON !== "1") this.skip();
    this.timeout(480_000);
    await recordsPublishedExperiment([
      { startSeconds: 50, endSeconds: 60, temperature: 35 },
      { startSeconds: 110, endSeconds: 120, temperature: 35 },
    ], 0.2);
  });
});
