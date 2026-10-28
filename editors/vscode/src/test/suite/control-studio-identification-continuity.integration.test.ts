import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { engineAnswerRecorderScript } from "./control-studio-rendered-harness";
import { withStudio } from "./control-studio-three-page.integration.test";
import { pollValue, requiredEnvironment } from "./plant-rendered-harness";

// The fixture loop (src/test/fixtures/control-studio-continuity, a Control.PID law without a parked feature) is served by the engine
// (crates/trust-control/src/loop_definition.rs `rust_engine_support`), so the journey reads the engine's answers that the page records
// (control-studio-rendered-harness.ts `engineAnswerRecorderScript`): the engine route sends no status events.

type View = (body: string) => Promise<string>;
/** One point of the applied plant input on the identification plot's time base; a missing value is a gap, not a point (B10.3). */
interface InputPoint { timeSeconds: number; value: number }
/** One recorded engine answer as this journey reads it: its status, and the identification a Run answered, never the whole plot. */
interface Answer {
  kind: string;
  method?: string;
  status: string;
  reason?: string;
  stage?: string;
  /** The identification a Run answered (IdentifyResult, B3.8): its model type and the recording the model was fitted from. */
  modelType?: string;
  recordingId?: string;
  /** The TypeScript route's estimation runs, which the engine does not answer (gap G4). */
  runs?: any[];
  /** The plot's first applied plant input (IdentifyResult `plot`, role `plant-input`). */
  input?: InputPoint[];
}

// A 20 s experiment covers the 4 s process several times over, so eight identifications fit the budget.
const setup = (loop: any) => { loop.experiment = { ...loop.experiment, kind: "relay", amplitude: 1, durationSeconds: 20, relayDirection: "positive" }; };
const output = (name: string, value: unknown) => fs.writeFileSync(path.join(path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT")), `${name}.json`), JSON.stringify(value));
/** The engine answers the page recorded, from the `from`-th on. */
const answersFrom = (from: number): string =>
  `return JSON.stringify((d.defaultView.__engineAnswers||[]).slice(${from}).map(e=>{const r=e.result||{};const i=e.method==='run'?r.identification:e.method==='identify'?r:undefined;const plot=i&&i.plot;const input=plot?plot.traces.find(t=>t.role==='plant-input'):undefined;return {kind:e.kind,method:e.method,status:e.status,reason:e.reason,stage:e.flowStage,modelType:i&&i.modelType,recordingId:i&&i.recordingId,runs:i&&i.runs,input:input?plot.timeSeconds.map((timeSeconds,k)=>({timeSeconds,value:input.values[k]})).filter(p=>p.value!==null):undefined};}));`;
const answerCount = async (inner: View): Promise<number> => Number(await inner("return String((d.defaultView.__engineAnswers||[]).length);"));
/** A run that ended: its answer (completed, or paused for an input), or a failure. The run's last update comes just before its answer. */
const ended = (e: Answer): boolean => (e.kind === "control-studio.engine.result" && ["completed", "paused"].includes(e.status)) || e.status === "failed";
/** The times at which an applied input changes: each sample that differs from the one before it (B10.3: a missing value is no sample). */
const changeTimes = (points: readonly { timeSeconds: number; value: number }[]): number[] =>
  points.slice(1).filter((point, index) => Math.abs(point.value - points[index].value) > 1e-9).map((point) => point.timeSeconds);
/** The samples of an excitation: those from its first change to just before its last change, which returns the input to its baseline. */
const excitation = (points: readonly { timeSeconds: number; value: number }[]): number[] => {
  const times = changeTimes(points);
  if (times.length < 2) return [];
  return points.filter((point) => point.timeSeconds >= times[0] && point.timeSeconds < times[times.length - 1]).map((point) => point.value);
};
/**
 * The clock intervals an excitation spans: the time from its first change to its last, in its shortest interval between two changes.
 * A PRBS holds each of its bits for one clock interval, so this counts its bits, as the TypeScript route's trajectory length did.
 */
const clockIntervals = (points: readonly { timeSeconds: number; value: number }[]): number => {
  const times = changeTimes(points);
  if (times.length < 2) return 0;
  const clock = Math.min(...times.slice(1).map((time, index) => time - times[index]));
  return Math.round((times[times.length - 1] - times[0]) / clock);
};

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

async function listen(inner: View) {
  await inner(engineAnswerRecorderScript());
}
async function identify(inner: View, method: string, capture: (name: string) => Promise<void>, previousIds: Set<string>, gaps: string[], prefix = "continuity") {
  const from = await answerCount(inner);
  await inner(`const m=d.querySelector('[data-engineer-acquisition-method]');m.value=${JSON.stringify(method)};m.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'method';`);
  // The engine records the selection as a study input (engineInputs.ts): its answer is `edited` when the change left a validation
  // behind, and `inputs` when there was none to leave.
  await pollValue("saved selection", () => inner(answersFrom(from)), raw => (JSON.parse(raw) as Answer[]).some(e => e.method === "inputs" && ["edited", "inputs"].includes(e.status)), 10000);
  assert.strictEqual(await inner(`return String((d.defaultView.__engineAnswers||[]).slice(${from}).some(e=>e.status==='running'));`), "false", "Method selection alone does not excite the plant");
  await inner("d.querySelector('[data-engineer-find]').click();return 'identify';");
  const events = JSON.parse(await pollValue(`${method} complete`, () => inner(answersFrom(from)), raw => (JSON.parse(raw) as Answer[]).some(ended), 90000)) as Answer[];
  output(`${prefix}-${method}-${previousIds.size}`, events);
  await capture(`${prefix}-${method}-${previousIds.size}`);
  assert.ok(events.some(e => e.status === "completed"), JSON.stringify(events.map(e => ({status:e.status,reason:e.reason}))));
  const result = [...events].reverse().find(e => typeof e.modelType === "string");
  // One record: the model is fitted from the one recording the engine's answer names.
  assert.ok(result && typeof result.recordingId === "string" && result.recordingId.length > 0, JSON.stringify(events.map(e => ({status:e.status,reason:e.reason}))));
  assert.ok(!previousIds.has(result.recordingId), `${method} must execute new ST recordings instead of reusing the previous method's experiment`);
  previousIds.add(result.recordingId);
  // Gap G4, re-expressed on the engine's fields: a qualified record is a completed identification (B3.9), which the answer carries as its
  // model type, fitted from the one recording it names (checked above).
  await reportedGap(gaps, `G4 ${method} record`, () => assert.ok(result.modelType && result.recordingId));
  // The applied input the engine plots (IdentifyResult `plot`, B3.8): the experiment's command, read from the plot (gap G4, re-expressed).
  const points = result.input ?? [];
  if (["mv_step", "automatic"].includes(method)) {
    // The held part runs from the step until the input is restored at the end of the experiment.
    const stepStart = points.find(p => p.value !== points[0].value)?.timeSeconds;
    const end = points.length > 0 ? points[points.length - 1].timeSeconds : Number.NaN;
    const plateau = stepStart === undefined ? [] : points.filter(p => p.timeSeconds > stepStart + 1 && p.timeSeconds < end - 1).map(p => p.value);
    assert.ok(stepStart !== undefined && plateau.length > 10, "the displayed step input leaves its baseline and holds");
    assert.ok(Math.max(...plateau) - Math.min(...plateau) < 1e-6, "Actual displayed step input has a held plateau");
    // One held step, whose end restoration the runtime makes (native end restoration): by the end of the record the input has left the
    // plateau back toward its baseline. The loop's rate limit can make the return take longer than the record (B3.4 step 5).
    await reportedGap(gaps, `G4 ${method} experiment command`, () => {
      const baseline = points[0].value, held = plateau[0], last = points[points.length - 1].value;
      assert.ok(Math.abs(last - baseline) < Math.abs(held - baseline) - 1e-9, "One held step, with native end restoration");
    });
  }
  // The relay feedback: between leaving its baseline and returning to it, the input switches between two levels only.
  if (method === "relay") await reportedGap(gaps, "G4 relay experiment command", () => assert.strictEqual(new Set(excitation(points).map(value => Math.round(value * 1e6))).size, 2, "The relay switches between two levels"));
  // The excitation command of more than fifteen points: a PRBS spans more than fifteen clock intervals, a multisine more than fifteen samples.
  if (method === "prbs" || method === "multisine") await reportedGap(gaps, `G4 ${method} experiment command`, () => assert.ok(clockIntervals(points) > 15, "The excitation spans more than fifteen clock intervals"));
  const seriesCount = await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;");
  // Gap G2: the chart draws the applied input beside the measurement and the prediction (engineIdentify.ts `showPlot`, its experiment).
  await reportedGap(gaps, `G2 ${method} applied input on the plot`, () => assert.strictEqual(seriesCount, "3"));
  return result;
}

suite("Control Studio identification continuity", () => {
  test("runs each selected method in the same study and reacquires after reopening", async function () {
    if (process.env.TRUST_IDENTIFY_CONTINUOUS !== "1") this.skip();
    this.timeout(900000);
    await withStudio(async (inner, reopen, capture) => {
      const gaps: string[] = [];
      const ids = new Set<string>(); await listen(inner);
      // The engine route identifies on the connected runtime and refuses without one (specification 35, A2.7; connected_run.rs
      // `runtime_required`); the TypeScript route started the local simulator itself (runtimeSearchConnection.ts).
      const started = await runtimeLifecycleService.startLocalSimulator();
      assert.ok(started.ok, JSON.stringify(started));
      for (const method of ["multisine", "mv_step", "mv_pulse", "prbs", "relay", "setpoint_step", "automatic"]) {
        await identify(inner, method, capture, ids, gaps);
      }
      await reopen(); await listen(inner);
      await identify(inner, "mv_step", capture, ids, gaps);
      assert.deepStrictEqual(gaps, [], REPORTED_GAPS);
    }, { loop: setup });
  });
});
