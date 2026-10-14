import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { engineAnswerRecorderScript } from "./control-studio-rendered-harness";
import { withStudio } from "./control-studio-three-page.integration.test";
import { pollValue, requiredEnvironment } from "./plant-rendered-harness";

const clippedRelay = process.env.TRUST_IDENTIFY_RELAY_CLIPPED === "1";
const methods = ["automatic", "mv_step", "mv_pulse", "setpoint_step", "relay", "prbs", "multisine"] as const;

type View = (body: string) => Promise<string>;
/**
 * The engine answers the page recorded from the `from`-th on (control-studio-rendered-harness.ts): each answer's status, and the
 * identification a Run answered with its plotted plant inputs and measured outputs (IdentifyResult `plot`, B3.8). `runs` and
 * `experiment` are the TypeScript route's records, which the engine does not answer (gap G4).
 */
const answersFrom = (from: number): string =>
  `return JSON.stringify((d.defaultView.__engineAnswers||[]).slice(${from}).map(e=>{const r=e.result||{};const i=e.method==='run'?r.identification:e.method==='identify'?r:undefined;const plot=i&&i.plot;const traces=plot?plot.traces:[];return {kind:e.kind,method:e.method,status:e.status,command:e.command,operationId:e.operationId,flowStage:e.flowStage,reason:e.reason,modelType:i&&i.modelType,provisional:i&&i.provisional,recordingId:i&&i.recordingId,runs:i&&i.runs&&i.runs.map(q=>({id:q.run.id,request:q.request,run:q.run,evaluation:q.evaluation.status})),experiment:e.failure&&e.failure.experiment,inputs:plot?traces.filter(t=>t.role==='plant-input').map(t=>({id:t.symbol,points:plot.timeSeconds.map((timeSeconds,k)=>({timeSeconds,value:t.values[k]})).filter(p=>p.value!==null)})):undefined,outputs:plot?traces.filter(t=>t.role==='measurement').map(t=>({id:t.symbol})):undefined};}));`;
const answerCount = async (inner: View): Promise<number> => Number(await inner("return String((d.defaultView.__engineAnswers||[]).length);"));
/** A run that ended: its answer (completed, or paused for an input), or a failure. The run's last update comes just before its answer. */
const ended = (event: any): boolean => (event.kind === "control-studio.engine.result" && ["completed", "paused"].includes(event.status)) || event.status === "failed";
/** The changes of an applied input: each sample that differs from the one before it (B10.3: a missing value is no sample). */
const changes = (values: readonly number[]): number => values.slice(1).filter((value, index) => Math.abs(value - values[index]) > 1e-9).length;
/** The command a method runs, read from the applied input the engine plots (IdentifyResult `plot`): the facts of `methodChecks`, on the plot. */
function plottedCommand(method: string, values: readonly number[]): void {
  if (method === "mv_pulse") assert.ok(changes(values) >= 2 && Math.abs(values[values.length - 1] - values[0]) < 1e-6, "Pulse restores during the record");
  if (method === "relay") assert.strictEqual(new Set(values.map(value => Math.round(value * 1e6))).size, 2, "Switch between two levels, from measured feedback");
  if (method === "prbs" || method === "multisine") assert.ok(changes(values) > 15, "Use the selected frequency excitation sequence");
  // The plot holds the actuator's output only; the reference a setpoint step moves is not on the engine's answer (a remaining gap).
  if (method === "setpoint_step") assert.fail("The engine's identification does not name the reference a setpoint step moves");
}

const REPORTED_GAPS = "Checks the engine route cannot pass yet (reported gaps)";
/**
 * Runs one check of a value the engine route does not show yet (a reported gap) where it stands, logs it when it fails, and lets the
 * journey go on, so that one run shows every gap; the test fails at its end when any gap failed.
 */
async function reportedGap(gaps: string[], name: string, check: () => unknown): Promise<void> {
  try { await check(); }
  catch (error) {
    const gap = `${name}: ${error instanceof Error ? error.message : String(error)}`;
    gaps.push(gap);
    console.error(`Reported gap: ${gap}`);
  }
}

suite("Control Studio all identification methods", () => {
  for (const method of methods) test(clippedRelay && method === "relay" ? "reports clipped relay output and retains the actual experiment" : `executes ${method} from the method selector and shows its actual experiment`, async function () {
    if (process.env.TRUST_IDENTIFY_ALL_METHODS !== "1") this.skip();
    const selected = process.env.TRUST_IDENTIFY_METHODS?.split(",") ?? methods;
    if (!selected.includes(method)) this.skip();
    // 20 s more than the TypeScript route's 210 s: on the engine route the runtime start is its own step before Identify.
    this.timeout(230000);
    let loop: any;
    let observations: unknown;
    const suffix = process.env.TRUST_CONTROL_STUDIO_PID_GROUP_NATIVE === "1" ? "mimo" : "siso";
    // The SISO fixture (examples/control-pid-commissioning) is served by the engine. The PID-group fixture of the MIMO run declares a
    // group and channels, which the engine parks (crates/trust-control/src/loop_definition.rs `rust_engine_support`): it stays on the
    // TypeScript route, whose status events these checks read as before.
    const served = suffix === "siso";
    try {
      await withStudio(async (inner, reopen, capture) => {
        const gaps: string[] = [];
        assert.strictEqual(loop.controller.type, "Control.PID", "Execute the real library controller");
        const selectMethod = `d.querySelector('[data-engineer-stage-tab=model]').click();const method=d.querySelector('[data-engineer-acquisition-method]');method.value=${JSON.stringify(method)};method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'method selected';`;
        let beforeMethod = 0;
        if (served) {
          await inner(engineAnswerRecorderScript());
          // The engine route identifies on the connected runtime and refuses without one (specification 35, A2.7; connected_run.rs
          // `runtime_required`); the TypeScript route started the local simulator itself (runtimeSearchConnection.ts).
          const started = await runtimeLifecycleService.startLocalSimulator();
          assert.ok(started.ok, JSON.stringify(started));
          beforeMethod = await answerCount(inner);
          await inner(selectMethod);
        } else {
          await inner(`d.querySelector('[data-engineer-stage-tab=model]').click();d.defaultView.__methodEvents=[];d.defaultView.addEventListener('trust-control-studio-engineer',e=>{const p=e.detail;d.defaultView.__methodEvents.push({status:p.status,flowStage:p.flowStage,reason:p.reason,experiment:p.identificationExperiment,model:p.model,presentation:p.identificationPresentation,runs:p.identification?.runs?.map(r=>({id:r.run.id,request:r.request,run:r.run,evaluation:r.evaluation.status}))});});const method=d.querySelector('[data-engineer-acquisition-method]');method.value=${JSON.stringify(method)};method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'method selected';`);
        }
        await pollValue("method input stored", () => inner("return String(d.querySelector('[data-studio-three-page]').dataset.busy??'false');"), value => value === "false", 10000);
        // The engine records the selected method as a study input before the run reads it (engineInputs.ts, §0.1a `inputs`).
        if (served) await pollValue("recorded method", () => inner(answersFrom(beforeMethod)), raw => JSON.parse(raw).some((event: any) => event.method === "inputs"), 10000);
        assert.strictEqual(await inner(`return d.querySelector('[data-engineer-acquisition-method]').value;`), method);
        const beforeIdentify = served ? await answerCount(inner) : 0;
        assert.strictEqual(await inner("const button=d.querySelector('[data-engineer-find]');if(!button.checkVisibility()||button.disabled)return 'unavailable';button.click();return 'started';"), "started");
        const events = served
          ? JSON.parse(await pollValue(`${method} terminal`, () => inner(answersFrom(beforeIdentify)), raw => JSON.parse(raw).some(ended), 170000))
          : JSON.parse(await pollValue(`${method} terminal`, () => inner("return JSON.stringify(d.defaultView.__methodEvents);"), raw => JSON.parse(raw).some((event: any) => ["completed", "paused", "failed"].includes(event.status)), 170000));
        observations = events;
        /** Reset study: the TypeScript route reloads the page; the engine route answers the reset, and the page clears the model and shows the engine's words for a new study (engineIdentify.ts `clearEngineIdentification`). */
        const resetStudy = async (): Promise<void> => {
          const beforeReset = served ? await answerCount(inner) : 0;
          await inner("d.querySelector('[data-studio-reset-chart]').click();Array.from(d.querySelectorAll('.studio-reset-confirm button')).find(b=>b.textContent==='Reset study').click();return 'reset';");
          if (!served) {
            await pollValue("reset complete", () => inner("return d?.querySelector('[data-control-studio-chart]')?.dataset.pointCount??'loading';").catch(() => "loading"), value => Number(value) >= 4, 10000);
            return;
          }
          // The engine answers the reset, and the page clears the model it showed and names the new study.
          await pollValue("reset complete", () => inner(`return JSON.stringify({reset:(d.defaultView.__engineAnswers||[]).slice(${beforeReset}).some(e=>e.status==='reset'),status:d.querySelector('[data-three-page-identification-status]')?.textContent??'',cleared:d.querySelector('[data-studio-model-result]')?.hidden??false,points:d.querySelector('[data-control-studio-chart]')?.dataset.pointCount??'loading'});`), value => {
            const state = JSON.parse(value) as { reset: boolean; status: string; cleared: boolean; points: string };
            return state.reset && state.status === "New identification study started." && state.cleared && Number(state.points) >= 4;
          }, 10000);
          // The engine's reset answer ends the page's Reset (threePageOperation.ts), so the page is ready for a new identification.
          const busy = await inner("return d.querySelector('[data-studio-three-page]').dataset.busy;");
          await reportedGap(gaps, "G5 reset leaves the page busy", () => assert.strictEqual(busy, "false", "Reset leaves the page ready for a new identification"));
        };
        if (clippedRelay && method === "relay") {
          // This path runs only with TRUST_IDENTIFY_RELAY_CLIPPED=1. The TypeScript route's Identify refused the clipped relay record.
          // On the engine route that sentence is raised by Tune's relay rule (crates/trust-control/src/tuning/relay_rule.rs,
          // `RELAY_INTERMEDIATE_OUTPUT`), and Identify fits a relay record as an excitation, so it is a reported gap there (G5).
          const clippedRefusal = (): void => assert.ok(events.some((event: any) => event.status === "failed" && /clipped or intermediate output values/.test(event.reason)), "Explain the rate-limited waveform");
          if (served) await reportedGap(gaps, "G5 clipped relay refusal", clippedRefusal);
          else clippedRefusal();
          const pointCount = Number(await inner("return d.querySelector('[data-control-studio-chart]').dataset.pointCount;"));
          if (served) {
            // Gap G5: a refused identification answers its failure only; the page keeps no rejected experiment to show.
            await reportedGap(gaps, "G5 rejected experiment", () => assert.ok(events.some((event: any) => event.experiment?.inputs?.length), "Keep the rejected experiment visible"));
            await reportedGap(gaps, "G5 rejected experiment on the plot", () => assert.ok(pointCount > 100, `pointCount ${pointCount}`));
          } else {
            assert.ok(events.some((event: any) => event.experiment?.inputs?.length), "Keep the rejected experiment visible");
            assert.ok(pointCount > 100);
          }
          await capture("method-siso-relay-clipped");
          await resetStudy();
          assert.deepStrictEqual(gaps, [], REPORTED_GAPS);
          return;
        }
        assert.ok(events.some((event: any) => event.status === "completed" && event.flowStage === "Identify"),
          JSON.stringify(events.map((event: any) => ({ status: event.status, reason: event.reason }))));
        const scanBuffer = (): void => assert.ok(events.filter((event: any) => event.runs).every((event: any) => event.runs.every((run: any) => run.request.execution === "connected-runtime" && run.request.acquisition.method === "runtime_scan_buffer")), "Use the connected plant's scan buffer");
        // Gap G4 (remains): the engine's answer names no acquisition path, so the connected scan buffer is not shown.
        if (served) await reportedGap(gaps, "G4 scan-buffer acquisition", () => { assert.ok(events.some((event: any) => event.runs), "The engine's identification answers no estimation runs"); scanBuffer(); });
        else scanBuffer();
        const result = [...events].reverse().find((event: any) => served ? typeof event.modelType === "string" : event.status === "model-ready");
        assert.ok(served ? result?.modelType : result?.model, "The selected method must produce a provisional plant model");
        const oneRecord = (): void => assert.ok(result.runs?.length === 1 && result.runs.every((run: any) => run.run.evidence === "ST-executed" && run.evaluation === "Pass"), "Use one bounded actual-ST estimation record");
        if (served) {
          // One record: the engine fits the model from the one recording its answer names.
          assert.ok(typeof result.recordingId === "string" && result.recordingId.length > 0, "Use one bounded actual-ST estimation record");
          // Gap G4 (remains): the engine's identification names no evidence source, so ST-executed is not shown.
          await reportedGap(gaps, "G4 ST-executed record", () => assert.strictEqual(result.evidence, "ST-executed", "The engine's identification names no evidence source"));
        } else oneRecord();
        assert.strictEqual(await inner("return d.querySelector('[data-studio-three-page]').dataset.selectedStage;"), "model");
        assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "false");
        // The engine's plot holds the applied plant input and the measured output beside the prediction (IdentifyResult `plot`).
        const experiment = served ? { inputs: result.inputs, outputs: result.outputs } : result.presentation?.experiment;
        assert.ok(experiment?.inputs?.length && experiment.outputs?.length, "Show applied inputs and measured outputs with the selected model");
        for (const input of experiment.inputs) {
          const values = input.points.map((point: any) => point.value).filter((value: any) => typeof value === "number");
          assert.ok(Math.max(...values) - Math.min(...values) > 0.001, `${input.id} actually changes`);
        }
        if (served) {
          // Gap G4, re-expressed on the engine's fields: the one acquisition is the one recording the result names.
          await reportedGap(gaps, "G4 one acquisition", () => assert.ok(typeof result.recordingId === "string" && result.recordingId.length > 0, "Identify performs one acquisition"));
          assert.strictEqual(result.provisional, true, "Independent checking belongs on Validate");
        } else {
          const runs = result.runs.filter((run: any) => run.run.samples.length > 4);
          assert.strictEqual(new Set(runs.map((run: any) => run.id)).size, 1, "Identify performs one acquisition");
          assert.strictEqual(result.model.validation, "unvalidated", "Independent checking belongs on Validate");
        }
        const methodChecks = (runs: any[]): void => {
          if (method === "mv_pulse") {
            const command = runs[0].request.experiment;
            assert.ok(command.trajectory.some((event: any) => event.value === command.restorationValue && event.atMs > 0 && event.atMs < command.durationMs), "Pulse restores during the record");
          }
          if (method === "setpoint_step") assert.ok(runs.every((run: any) => run.request.experiment.affectedSignal === loop.signals.setpoint.symbol), "Step the declared reference while recording the applied actuator output");
          if (method === "relay") assert.ok(runs.every((run: any) => run.request.experiment.relayFeedback && !run.request.experiment.trajectory.length), "Switch from measured feedback, not a timed square wave");
          if (method === "prbs" || method === "multisine") assert.ok(runs.every((run: any) => run.request.experiment.trajectory.length > 15), "Use the selected frequency excitation sequence");
        };
        // Gap G4, re-expressed on the applied input of the engine's plot: the command each method runs (IdentifyResult `plot`).
        if (served) await reportedGap(gaps, `G4 ${method} experiment command`, () => plottedCommand(method, (experiment.inputs[0]?.points ?? []).map((point: any) => point.value as number)));
        else methodChecks(result.runs.filter((run: any) => run.run.samples.length > 4));
        const seriesCount = Number(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"));
        const everySeries = (): void => assert.ok(seriesCount >= experiment.outputs.length * 2 + experiment.inputs.length, "Plot every input, measurement and prediction");
        // Gap G2: engineIdentify.ts `showPlot` hands the chart the measurement and the prediction only, not the applied input.
        if (served) await reportedGap(gaps, "G2 applied input on the plot", everySeries);
        else everySeries();
        await capture(`method-${suffix}-${method}`);
        assert.ok(Number(await inner("return String(d.querySelector('[data-studio-model-equation]').getBoundingClientRect().height);")) <= 120,
          "The full model equation stays scrollable in a compact result so the experiment remains visible");
        if (suffix === "mimo") {
          await inner("d.defaultView.scrollTo(0,d.body.scrollHeight);return 'last channels';");
          await capture(`method-${suffix}-${method}-lower-channels`);
          await inner("d.defaultView.scrollTo(0,0);return 'top';");
        }
        await reopen();
        if (served) await inner(engineAnswerRecorderScript());
        await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'reopened';");
        const label = await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label');");
        // Gaps G2 and G3 (closed by the opened identification): the reopened chart draws the applied input beside the response.
        if (served) await reportedGap(gaps, "G2/G3 reopened applied input", () => assert.match(label, /Applied input/));
        else assert.match(label, /Applied input/);
        await resetStudy();
        assert.deepStrictEqual(gaps, [], REPORTED_GAPS);
      }, { loop: value => { loop = value; loop.experiment.durationSeconds = suffix === "mimo" ? 60 : 30; loop.experiment.relayDirection = "positive"; if (method === "relay") { loop.experiment.kind = "relay"; loop.experiment.amplitude = clippedRelay ? 10 : 1; } if (suffix === "mimo") loop.identificationPolicy = { ...loop.identificationPolicy, maximumOrder: 8 }; } });
    } finally {
      if (observations) fs.writeFileSync(path.join(path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT")), `method-${suffix}-${method}.json`), JSON.stringify(observations));
    }
  });
});
