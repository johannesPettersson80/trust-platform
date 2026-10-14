import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { withStudio } from "./control-studio-three-page.integration.test";
import { engineAnswerRecorderScript } from "./control-studio-rendered-harness";
import { pollValue, requiredEnvironment } from "./plant-rendered-harness";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { RuntimeRecordingSession } from "../../controlStudio/runtimeRecording";

suite("Control Studio connected live identification", () => {
  test("continues plotting current runtime samples after Reset", async function () {
    if (process.env.TRUST_IDENTIFY_LIVE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async (inner, _reopen, capture) => {
      const started = await runtimeLifecycleService.startLocalSimulator(); assert.ok(started.ok);
      await pollValue("live readings", () => inner("return d.querySelector('[data-control-studio-chart]').dataset.pointCount;"), value => Number(value) >= 8, 15000);
      const loop = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
      const recording = await RuntimeRecordingSession.start(loop);
      await inner("d.querySelector('[data-studio-reset-chart]').click();Array.from(d.querySelectorAll('.studio-reset-confirm button')).find(b=>b.textContent==='Reset study').click();return 'reset';");
      await new Promise(resolve => setTimeout(resolve, 3000));
      const after = JSON.parse(await inner("return JSON.stringify({points:Number(d.querySelector('[data-control-studio-chart]').dataset.pointCount),range:JSON.parse(d.querySelector('[data-control-studio-chart]').dataset.timeRange),model:d.querySelector('[data-studio-model-result]').checkVisibility()});"));
      const raw = await recording.stop();
      fs.writeFileSync(path.join(path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT")), "reset-live-samples.json"), JSON.stringify({after,recording:raw}));
      await capture("reset-live");
      assert.ok(after.points >= 8, "Reset must clear old history and immediately resume live signals, not freeze an empty recording");
      assert.strictEqual(after.model, false);
      await new Promise(resolve => setTimeout(resolve, 1500));
      assert.ok(Number(await inner("return d.querySelector('[data-control-studio-chart]').dataset.pointCount;")) > after.points, "Live samples continue arriving");
    });
  });
  test("shows the connected plant evolving during Identify instead of returning a completed isolated trial", async function () {
    if (process.env.TRUST_IDENTIFY_LIVE !== "1") this.skip();
    this.timeout(120000);
    await withStudio(async (inner, _reopen, capture) => {
      const started = await runtimeLifecycleService.startLocalSimulator(); assert.ok(started.ok);
      await pollValue("live readings", () => inner("return d.querySelector('[data-control-studio-chart]').dataset.pointCount;"), value => Number(value) >= 8, 15000);
      await inner("d.defaultView.__liveEvents=[];d.defaultView.addEventListener('trust-control-studio-engineer',e=>d.defaultView.__liveEvents.push(e.detail));const m=d.querySelector('[data-engineer-acquisition-method]');m.value='mv_step';m.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'step';");
      // The engine route reports the run's progress as `control-studio.engine.*` messages, not as page events: the evidence keeps both.
      await inner(engineAnswerRecorderScript());
      await new Promise(resolve => setTimeout(resolve, 200));
      const loop = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
      const observed = await RuntimeRecordingSession.start(loop);
      await inner("d.querySelector('[data-engineer-find]').click();return 'identify';");
      await new Promise(resolve => setTimeout(resolve, 10000));
      const state = JSON.parse(await inner("return JSON.stringify({range:JSON.parse(d.querySelector('[data-control-studio-chart]').dataset.timeRange),busy:d.querySelector('[data-studio-three-page]').dataset.busy,events:d.defaultView.__liveEvents.map(p=>({status:p.status,reason:p.reason,experiment:p.identificationExperiment})),engine:(d.defaultView.__engineAnswers||[]).map(e=>({status:e.status,flowStage:e.flowStage,reason:e.reason}))});"));
      fs.writeFileSync(path.join(path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT")), "identify-live-progress.json"), JSON.stringify(state));
      await capture("identify-live-progress");
      try {
        assert.strictEqual(state.busy, "true", "Identify remains active while its live experiment is running");
        assert.ok(state.range[1] >= 8, "The chart grows while the experiment is running");
        assert.ok(state.range[1] < 15, "The visible chart advances with the running plant, not a whole 30/60-second batch result");
      } finally {
        await inner("if(d.querySelector('[data-studio-three-page]').dataset.busy==='true')d.querySelector('[data-engineer-find]').click();return 'abort';");
        await pollValue("abort complete", () => inner("return d.querySelector('[data-studio-three-page]').dataset.busy;"), value => value === "false", 15000);
        await new Promise(resolve => setTimeout(resolve, 1200));
        const record = await observed.stop();
        fs.writeFileSync(path.join(path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT")), "identify-connected-observation.json"), JSON.stringify(record));
        const samples = record.rawSamples!;
        assert.ok(samples.some(sample => sample.appliedMv! > samples[0].appliedMv! + 5), "The connected runtime's applied input changes");
        assert.ok(samples.some(sample => sample.pv! > samples[0].pv! + 1), "Its native simulated plant responds during the run");
        assert.strictEqual(samples[samples.length-1].appliedMv, samples[0].appliedMv, "Abort restores the original connected output");
        assert.strictEqual(samples[samples.length-1].controllerMode, samples[0].controllerMode, "Abort restores the original controller mode");
      }
    }, {loop: loop => {loop.experiment.durationSeconds=60;}});
  });
  test("closing a live experiment leaves it running in the runtime and reopening follows it until the plant is restored", async function () {
    if (process.env.TRUST_IDENTIFY_LIVE !== "1") this.skip();
    this.timeout(180000);
    await withStudio(async (inner, reopen, capture) => {
      assert.ok((await runtimeLifecycleService.startLocalSimulator()).ok);
      await pollValue("live readings", () => inner("return d.querySelector('[data-control-studio-chart]').dataset.pointCount;"), value => Number(value) >= 8, 15000);
      const loop = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
      const observed = await RuntimeRecordingSession.start(loop);
      const clicked = Date.now();
      await inner("d.querySelector('[data-engineer-find]').click();return 'identify';");
      await new Promise(resolve => setTimeout(resolve, 10000));
      const closedAfterSeconds = (Date.now() - clicked) / 1000;
      await reopen();
      await new Promise(resolve => setTimeout(resolve, 3000));
      await capture("live-reopened");
      // The experiment belongs to the runtime: closing the panel neither aborts nor restores it, and the reopened page follows it
      // to its end, restores what the identification changed and shows its result.
      await pollValue("reattached identification shows as running", () => inner("return d.querySelector('[data-studio-three-page]').dataset.busy;"), value => value === "true", 20000);
      await pollValue("reattached identification ends", () => inner("return d.querySelector('[data-studio-three-page]').dataset.busy;"), value => value === "false", 120000);
      await new Promise(resolve => setTimeout(resolve, 1500));
      const recording = await observed.stop();
      const samples = recording.rawSamples!;
      const elevated = samples.filter(sample => sample.appliedMv! > samples[0].appliedMv! + 5);
      assert.ok(elevated.length, "the identification applied its step");
      assert.ok(elevated[elevated.length - 1].elapsedSeconds - samples[0].elapsedSeconds > closedAfterSeconds + 1,
        "the step stayed applied after the panel closed, as the runtime owns the experiment");
      assert.strictEqual(samples[samples.length-1].appliedMv, samples[0].appliedMv, "the runtime restored the connected output when the experiment ended");
      assert.strictEqual(samples[samples.length-1].controllerMode, samples[0].controllerMode);
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-model-result]')?.hidden === false);"), "true",
        "the reopened page shows the result of the experiment it followed");
      await capture("live-close-reopen");
      const state = await inner("return JSON.stringify({root:d.querySelector('[data-studio-three-page]').dataset,chart:d.querySelector('[data-control-studio-chart]').dataset,label:d.querySelector('[data-control-studio-chart]').getAttribute('aria-label')});");
      fs.writeFileSync(path.join(path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT")), "live-close-state.json"), state);
    }, {loop: loop => {loop.experiment.durationSeconds=60;}});
  });

});
