import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { withStudio } from "./control-studio-three-page.integration.test";
import { engineAnswerRecorderScript } from "./control-studio-rendered-harness";
import { pollValue, requiredEnvironment } from "./plant-rendered-harness";
import { processReading, type SimulatorLoop } from "./control-studio-simulator-runtime";
import { runtimeLifecycleService } from "../../runtimeLifecycle";

/**
 * The samples of the kept recording in the last identification answer the page holds: `IdentifyResult.recordedSamples`, which a campaign that
 * finishes keeps (identify_action.rs, keep_campaign). Identify's start is the engine's automatic flow (B8, runRoute.ts), so the answer is the
 * run's own: the identification it made is `RunResult.identification`. An identification the engine answered by itself carries them at the top.
 */
const LAST_IDENTIFY_SAMPLES = "const holders=(d.defaultView.__engineAnswers||[]).filter(e=>e.kind==='control-studio.engine.result').map(e=>e.method==='run'?e.result&&e.result.identification:e.method==='identify'?e.result:undefined).filter(identification=>identification&&Array.isArray(identification.recordedSamples));const last=holders[holders.length-1];return JSON.stringify(last?last.recordedSamples:[]);";
/** The restoration that the same identification answer names (`IdentifyResult.restoration`, C12): the one its campaign's experiment confirmed. */
const LAST_IDENTIFY_RESTORATION = "const holders=(d.defaultView.__engineAnswers||[]).filter(e=>e.kind==='control-studio.engine.result').map(e=>e.method==='run'?e.result&&e.result.identification:e.method==='identify'?e.result:undefined).filter(identification=>identification&&Array.isArray(identification.recordedSamples));const last=holders[holders.length-1];return String(last?last.restoration:undefined);";
/** The engine's answers the page holds, as the recorder projects them. */
const ENGINE_ANSWERS = "return JSON.stringify((d.defaultView.__engineAnswers||[]).map(e=>({kind:e.kind,method:e.method,status:e.status,flowStage:e.flowStage,reason:e.reason})));";

suite("Control Studio connected live identification", () => {
  test("continues plotting current runtime samples after Reset", async function () {
    if (process.env.TRUST_IDENTIFY_LIVE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async (inner, _reopen, capture) => {
      const started = await runtimeLifecycleService.startLocalSimulator(); assert.ok(started.ok);
      await pollValue("live readings", () => inner("return d.querySelector('[data-control-studio-chart]').dataset.pointCount;"), value => Number(value) >= 8, 15000);
      await inner("d.querySelector('[data-studio-reset-chart]').click();Array.from(d.querySelectorAll('.studio-reset-confirm button')).find(b=>b.textContent==='Reset study').click();return 'reset';");
      await new Promise(resolve => setTimeout(resolve, 3000));
      const after = JSON.parse(await inner("return JSON.stringify({points:Number(d.querySelector('[data-control-studio-chart]').dataset.pointCount),range:JSON.parse(d.querySelector('[data-control-studio-chart]').dataset.timeRange),model:d.querySelector('[data-studio-model-result]').checkVisibility()});"));
      // A Reset starts no run, so the page's own state is the evidence here: no runtime recording is taken.
      fs.writeFileSync(path.join(path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT")), "reset-live-state.json"), JSON.stringify({after}));
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
      // The engine route reports the run's progress both as `control-studio.engine.*` messages (`__engineAnswers`) and as flow events on
      // the page (threePageHostState.ts, dispatchEngineerEvent): the evidence keeps both.
      await inner(engineAnswerRecorderScript());
      await new Promise(resolve => setTimeout(resolve, 200));
      await inner("d.querySelector('[data-engineer-find]').click();return 'identify';");
      await new Promise(resolve => setTimeout(resolve, 10000));
      const state = JSON.parse(await inner("return JSON.stringify({range:JSON.parse(d.querySelector('[data-control-studio-chart]').dataset.timeRange),busy:d.querySelector('[data-studio-three-page]').dataset.busy,events:d.defaultView.__liveEvents.map(p=>({status:p.status,reason:p.reason,flowStage:p.flowStage})),engine:(d.defaultView.__engineAnswers||[]).map(e=>({status:e.status,flowStage:e.flowStage,reason:e.reason}))});"));
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
        // An aborted identification answers no result, so it keeps no recorded samples: the engine's answers to the abort are its evidence.
        fs.writeFileSync(path.join(path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT")), "identify-abort-answers.json"), await inner(ENGINE_ANSWERS));
      }
    }, {loop: loop => {loop.experiment.durationSeconds=60;}});
  });
  test("closing a live experiment leaves it running in the runtime and reopening follows it until the plant is restored", async function () {
    if (process.env.TRUST_IDENTIFY_LIVE !== "1") this.skip();
    this.timeout(180000);
    await withStudio(async (inner, reopen, capture) => {
      const loop: SimulatorLoop = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
      assert.ok((await runtimeLifecycleService.startLocalSimulator()).ok);
      // The output the plant receives at rest, read from the runtime's own scan before Identify starts: the runtime is read only while no
      // engine operation runs (control-studio-simulator-runtime.ts), and the live readings below wait for the page's monitor again.
      const atRest = (await pollValue("output at rest", () => processReading(loop), reading => reading !== undefined, 15000))!;
      // The read above replaced the runtime's one inspection watch with the journey's two signals (runtime protocol annex, E3 rule 1). The
      // run's first read of the loop's sample, which names the mode that Automatic chooses its method from, would then answer that no
      // coherent scan is held yet, and Automatic refuses a loop whose mode is unavailable (B3.3). The page's monitor takes the watch
      // back with its next readings, so Identify starts once readings have arrived that came after the journey's own: a chart that holds eight
      // points since the simulator started says nothing about that. Each reading adds a point to each of the chart's two or three series.
      const readAt = Number(await inner("return d.querySelector('[data-control-studio-chart]').dataset.pointCount;"));
      await pollValue("live readings after the journey's own read", () => inner("return d.querySelector('[data-control-studio-chart]').dataset.pointCount;"),
        value => Number(value) >= readAt + 6, 15000);
      const clicked = Date.now();
      await inner("d.querySelector('[data-engineer-find]').click();return 'identify';");
      await new Promise(resolve => setTimeout(resolve, 10000));
      // The experiment runs when the panel closes. A run the engine refused or ended by itself shows here, with the sentence the page holds,
      // instead of as a reopened page that has nothing to follow.
      assert.strictEqual(await inner("return d.querySelector('[data-studio-three-page]').dataset.busy;"), "true",
        `Identify runs when the panel closes: ${await inner("return d.querySelector('[data-studio-message=model]')?.textContent ?? '';")}`);
      const closedAfterSeconds = (Date.now() - clicked) / 1000;
      await reopen();
      await new Promise(resolve => setTimeout(resolve, 3000));
      await capture("live-reopened");
      // The experiment belongs to the runtime: closing the panel neither aborts nor restores it, and the reopened page follows it
      // to its end, restores what the identification changed and shows its result.
      await pollValue("reattached identification shows as running", () => inner("return d.querySelector('[data-studio-three-page]').dataset.busy;"), value => value === "true", 20000);
      await pollValue("reattached identification ends", () => inner("return d.querySelector('[data-studio-three-page]').dataset.busy;"), value => value === "false", 120000);
      await new Promise(resolve => setTimeout(resolve, 1500));
      // The followed run is the operation the engine kept running, so its finished answer carries the samples of its kept recording.
      const samples: { elapsedSeconds: number; appliedMv: number | null }[] = JSON.parse(await pollValue("the followed run's recorded samples",
        () => inner(LAST_IDENTIFY_SAMPLES), value => JSON.parse(value).length > 0, 15000));
      const elevated = samples.filter(sample => sample.appliedMv! > samples[0].appliedMv! + 5);
      assert.ok(elevated.length, "the identification applied its step");
      assert.ok(elevated[elevated.length - 1].elapsedSeconds - samples[0].elapsedSeconds > closedAfterSeconds + 1,
        "the step stayed applied after the panel closed, as the runtime owns the experiment");
      // The kept recording ends at the execution that confirms the restored Manual command (specification 35, B3.4 step 5 and A7), while the
      // program's Control.PID moves its output back at its own rate limit (RateMinimum -25 %/s, 2.5 % per 100 ms execution): the restored
      // output is therefore read from the runtime once the run has ended, as the journey once read it from a recording of its own.
      assert.strictEqual(await inner(LAST_IDENTIFY_RESTORATION), "restored", "the engine names the restoration its campaign confirmed");
      const restored = await pollValue("restored output", () => processReading(loop), reading => reading?.mv === atRest.mv, 10000)
        .catch(() => processReading(loop));
      assert.strictEqual(restored?.mv, atRest.mv, "the runtime restored the connected output when the experiment ended");
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-model-result]')?.hidden === false);"), "true",
        "the reopened page shows the result of the experiment it followed");
      await capture("live-close-reopen");
      const state = await inner("return JSON.stringify({root:d.querySelector('[data-studio-three-page]').dataset,chart:d.querySelector('[data-control-studio-chart]').dataset,label:d.querySelector('[data-control-studio-chart]').getAttribute('aria-label')});");
      fs.writeFileSync(path.join(path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT")), "live-close-state.json"), state);
    }, {loop: loop => {loop.experiment.durationSeconds=60;}});
  });

});
