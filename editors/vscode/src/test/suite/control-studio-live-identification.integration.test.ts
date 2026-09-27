import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { withStudio } from "./control-studio-three-page.integration.test";
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
      await inner("d.querySelector('[data-studio-reset-chart]').click();return 'reset';");
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
      await new Promise(resolve => setTimeout(resolve, 200));
      const loop = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
      const observed = await RuntimeRecordingSession.start(loop);
      await inner("d.querySelector('[data-engineer-find]').click();return 'identify';");
      await new Promise(resolve => setTimeout(resolve, 10000));
      const state = JSON.parse(await inner("return JSON.stringify({range:JSON.parse(d.querySelector('[data-control-studio-chart]').dataset.timeRange),busy:d.querySelector('[data-studio-three-page]').dataset.busy,events:d.defaultView.__liveEvents.map(p=>({status:p.status,reason:p.reason,experiment:p.identificationExperiment}))});"));
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
  test("closing a live experiment restores the plant before reopening", async function () {
    if (process.env.TRUST_IDENTIFY_LIVE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async (inner, reopen, capture) => {
      assert.ok((await runtimeLifecycleService.startLocalSimulator()).ok);
      await pollValue("live readings", () => inner("return d.querySelector('[data-control-studio-chart]').dataset.pointCount;"), value => Number(value) >= 8, 15000);
      const loop = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
      const observed = await RuntimeRecordingSession.start(loop);
      await inner("d.querySelector('[data-engineer-find]').click();return 'identify';");
      await new Promise(resolve => setTimeout(resolve, 10000));
      await reopen();
      await new Promise(resolve => setTimeout(resolve, 1500));
      const recording = await observed.stop();
      const samples = recording.rawSamples!;
      assert.ok(samples.some(sample => sample.appliedMv! > samples[0].appliedMv! + 5));
      assert.strictEqual(samples[samples.length-1].appliedMv, samples[0].appliedMv, "Closing restores the connected output");
      assert.strictEqual(samples[samples.length-1].controllerMode, samples[0].controllerMode);
      assert.strictEqual(await inner("return d.querySelector('[data-studio-three-page]').dataset.busy;"), "false");
      await capture("live-close-reopen");
      const state = await inner("return JSON.stringify({root:d.querySelector('[data-studio-three-page]').dataset,chart:d.querySelector('[data-control-studio-chart]').dataset,label:d.querySelector('[data-control-studio-chart]').getAttribute('aria-label')});");
      fs.writeFileSync(path.join(path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT")), "live-close-state.json"), state);
      await pollValue("reopened live readings", () => inner("return d.querySelector('[data-control-studio-chart]').dataset.pointCount;"), value => Number(value) >= 4, 10000);
    }, {loop: loop => {loop.experiment.durationSeconds=60;}});
  });

});
