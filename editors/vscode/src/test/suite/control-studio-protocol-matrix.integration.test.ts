import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { ConnectedIdentificationMode } from "../../controlStudio/connectedIdentificationMode";
import { buildControlExperimentPlan, classifyControlExperimentStatus, ControlExperimentClient, type ControlExperimentPlan } from "../../controlStudio/experiment";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import {
  ControlLoopMonitor, controlLoopStaleAfterMs, type AvailableLoopSample, type LoopMonitorSample, type LoopRecording,
} from "../../controlStudio/loopMonitor";
import { RECORDING_SAMPLE_LIMIT, recordPeriodExecutions } from "../../controlStudio/controlStudioLimits";
import { bindRuntimeConnection } from "../../controlStudio/runtimeConnection";
import { connectRuntimeParameters } from "../../controlStudio/runtimeParameterConnection";
import { RuntimeRecordingSession } from "../../controlStudio/runtimeRecording";
import { SymbolicLoopObservation } from "../../controlStudio/symbolicLoopObservation";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { setSelectedRuntimeId } from "../../selectedRuntime";
import { SIMULATOR_RUNTIME_ID } from "../../trustHomeModel";
import { prepareAttachedProject, startAttachedRuntime } from "./control-studio-attached-runtime";
import {
  MATRIX_RECORDING, matrixLoops, restoreMatrixRuntime, setMatrixTimeScale, writeMatrixProject, type MatrixLoop, type MatrixProject,
} from "./control-studio-protocol-matrix";
import { requiredEnvironment, sleep } from "./plant-rendered-harness";

const JOURNEY = "Control Studio runtime protocol matrix";
/** The Manual step is 10 % of output and the Auto step is 1 degC; each experiment runs 3 s of process time (A7: 1 s to 24 h). */
const OUTPUT_STEP = 10;
const SETPOINT_STEP = 1;
const EXPERIMENT_SECONDS = 3;
/** The parameter check multiplies the proportional gain by this factor, then rolls it back (E7.3, E7.5). */
const PARAMETER_FACTOR = 1.01;
const INSPECTION_TIMEOUT_MS = 15_000;
const EXPERIMENT_GRACE_MS = 60_000;

type Launch = "launched" | "attached";
interface StartContext { readonly timeScale: number; readonly launch: Launch }
interface Combination { readonly label: string; readonly entry: MatrixLoop; readonly manual: boolean; readonly context: StartContext }
interface MatrixRow {
  readonly label: string; readonly launch: Launch; readonly timeScale: number; readonly loop: string;
  readonly task: string; readonly output: string; readonly mode: "Manual" | "Auto";
  outcome: "passed" | "failed"; processMs: number | null; wallMs: number; failure: string | null;
}
/** The fields of control.experiment.status (E5.6) that the journey reads. */
interface ExperimentStatus {
  status: string; restoration: string; stop_reason: string | null; elapsed_ms: number;
  last_applied: { value: number } | null;
}
interface RunningRuntime { readonly runtimeId: string; stop(): Promise<void> }

suite(JOURNEY, () => {
  test("exercises the runtime protocol directly against trust-runtime for every D5 combination", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_PROTOCOL_MATRIX !== "1" || process.env.TRUST_PLANT_RENDERED_TEST !== "1") this.skip();
    // Four starts of eight combinations each, one at a time: the 1x runs take the longest.
    this.timeout(40 * 60 * 1000);
    const project = writeMatrixProject(vscode.workspace.workspaceFolders![0].uri.fsPath);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const evidence = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    const rows: MatrixRow[] = [];
    try {
      for (const timeScale of [1, 10]) {
        await runLaunched(project, timeScale, rows);
        await runAttached(project, timeScale, rows);
      }
    } finally {
      restoreMatrixRuntime(project);
      writeTable(evidence, rows);
    }
    assert.strictEqual(rows.length, 32, "D5 is 2 x 2 x 2 x 2 x 2 = 32 combinations");
    const failed = rows.filter(row => row.outcome === "failed");
    assert.strictEqual(failed.length, 0, failed.map(row => `${row.label}: ${row.failure}`).join("\n"));
  });
});

/** Starts the extension's own simulator, as the native journeys do, at the saved time scale. */
async function runLaunched(project: MatrixProject, timeScale: number, rows: MatrixRow[]): Promise<void> {
  await runStart({ timeScale, launch: "launched" }, rows, async () => {
    restoreMatrixRuntime(project);
    setMatrixTimeScale(project, timeScale);
    await setSelectedRuntimeId(SIMULATOR_RUNTIME_ID);
    const started = await runtimeLifecycleService.startLocalSimulator();
    if (!started.ok) throw new Error(`The extension's simulator did not start: ${JSON.stringify(started)}`);
    return { runtimeId: SIMULATOR_RUNTIME_ID, stop: async () => { await runtimeLifecycleService.stopRuntime(); } };
  });
}

/** Attaches to a runtime this test starts itself (`trust-runtime run --simulation --time-scale N`), as the speed journey does. */
async function runAttached(project: MatrixProject, timeScale: number, rows: MatrixRow[]): Promise<void> {
  await runStart({ timeScale, launch: "attached" }, rows, async () => {
    restoreMatrixRuntime(project);
    setMatrixTimeScale(project, timeScale);
    prepareAttachedProject(project.root);
    const runtime = await startAttachedRuntime(project.root, { timeScale, label: `Protocol matrix at ${timeScale}x` });
    return {
      runtimeId: runtime.endpoint,
      stop: async () => { await runtime.stop(); restoreMatrixRuntime(project); },
    };
  });
}

/** Runs the eight combinations of one start. A start that fails takes every combination it would have run with it. */
async function runStart(context: StartContext, rows: MatrixRow[], start: () => Promise<RunningRuntime>): Promise<void> {
  const first = rows.length;
  let running: RunningRuntime | undefined;
  try {
    running = await start();
    for (const combination of combinations(context, matrixLoops(running.runtimeId))) {
      rows.push(await runCombination(combination));
    }
  } catch (error) {
    for (const combination of combinations(context, matrixLoops("")).slice(rows.length - first)) {
      rows.push({ ...rowFor(combination), failure: messageOf(error) });
    }
  } finally {
    await running?.stop();
  }
}

/** Auto is entered first, so each loop goes Manual -> Auto -> Manual and both transfers run. */
function combinations(context: StartContext, loops: readonly MatrixLoop[]): Combination[] {
  return loops.flatMap(entry => [false, true].map(manual => ({
    label: combinationLabel(context, entry, manual), entry, manual, context,
  })));
}

function combinationLabel(context: StartContext, entry: MatrixLoop, manual: boolean): string {
  return `${modeName(manual)} mode, ${entry.output} output (${entry.label}), ${entry.task}, time scale ${context.timeScale}x, ${context.launch}`;
}

function rowFor(combination: Combination): MatrixRow {
  const { label, entry, manual, context } = combination;
  return { label, launch: context.launch, timeScale: context.timeScale, loop: entry.label, task: entry.task, output: entry.output,
    mode: modeName(manual), outcome: "failed", processMs: null, wallMs: 0, failure: null };
}

async function runCombination(combination: Combination): Promise<MatrixRow> {
  const row = rowFor(combination);
  const began = Date.now();
  try {
    row.processMs = await exerciseCombination(combination);
    row.outcome = "passed";
  } catch (error) {
    row.failure = messageOf(error);
  }
  row.wallMs = Date.now() - began;
  return row;
}

/** One combination through the extension's own clients: E6 mode transfer, E3 inspection, E4 recording, E5 experiment, E7 parameter. */
async function exerciseCombination(combination: Combination): Promise<number> {
  const { entry, manual, label, context } = combination;
  const loop = entry.loop;
  const mode = modeName(manual);
  const connection = await bindRuntimeConnection(loop.runtimeId);
  const observer = new LoopObserver(loop);

  // 1. Put the loop in the mode under test through the transfer client, from wherever it is (E6, A6.2).
  await new ConnectedIdentificationMode(loop, connection.request, memoryPersistence()).enter(manual);

  // 2. Inspect the loop's signals (E3): one coherent scan, good and not forced, in the mode under test, then a newer scan.
  const runtimeStatus = (await connection.request("status", {})) as { simulation_time_scale?: number };
  check(runtimeStatus.simulation_time_scale === context.timeScale, label,
    `the runtime runs at time scale ${String(runtimeStatus.simulation_time_scale)}, not ${context.timeScale}`);
  const first = await observer.available(label);
  const inspected = await observer.available(label, first.scan);
  check(inspected.controller.mode === mode, label, `the loop reads ${inspected.controller.mode} after the transfer to ${mode}`);
  const signals = [inspected.processVariable, inspected.manipulatedVariable, ...(inspected.setpoint ? [inspected.setpoint] : [])];
  check(signals.every(signal => signal.quality === "good"), label,
    `not every signal is good and unforced: ${signals.map(signal => `${signal.symbol} ${signal.quality}`).join(", ")}`);

  // 3. Plan the experiment from that scan (E5.2): a plant-input step in Manual, a setpoint step in Auto.
  const plan = experimentPlan(combination, inspected);

  // 4. Record with a record period of two executions (E4, A4.2) while the experiment runs.
  const recording = MATRIX_RECORDING[entry.task];
  check(recordPeriodExecutions(recording.plannedMs, entry.periodMs, RECORDING_SAMPLE_LIMIT, recording.settlingSeconds) === 2, label,
    "the planned duration and settling time do not give a record period of two executions");
  const session = await RuntimeRecordingSession.start(loop, undefined, recording.plannedMs, recording.settlingSeconds);
  const client = new ControlExperimentClient((operation, params) => connection.request(operation, params as Record<string, unknown>));
  let status: ExperimentStatus | undefined;
  let failure: unknown;
  try {
    status = await runExperiment(client, plan, combination);
    await recordOnePeriodMore(session, combination.label);
  } catch (error) {
    failure = error;
  }
  // The recording is stopped and released even when the experiment failed, so no buffer is left behind.
  const stopped = await session.stop();
  await session.release();
  if (!status) throw failure;

  // 5. The experiment ended restored, and the recording shows the step, a record period of two and no gap (E4.5, E5.6, E5.7).
  checkExperiment(status, plan, label);
  checkRecording(stopped, plan, combination);

  // 6. The loop is still in the mode under test (A6.2).
  const after = await observer.available(label);
  check(after.controller.mode === mode, label, `the loop reads ${after.controller.mode} after the experiment`);

  // 7. Write one parameter change and roll it back (E7).
  await checkParameter(combination, loop);
  return status.elapsed_ms;
}

function experimentPlan(combination: Combination, sample: AvailableLoopSample): ControlExperimentPlan {
  const { entry, manual, label } = combination;
  const kind = manual ? "mv_step" : "setpoint_step";
  const result = buildControlExperimentPlan({ loop: entry.loop, sample, kind, amplitude: manual ? OUTPUT_STEP : SETPOINT_STEP,
    durationSeconds: EXPERIMENT_SECONDS });
  if (result.kind !== "accepted") throw new Error(`${label}: the experiment is not valid at this operating point: ${result.reason}`);
  return result.plan;
}

/** Starts the experiment and follows it until its record is terminal, which for a completed run means restoration confirmed (E5.6). */
async function runExperiment(client: ControlExperimentClient, plan: ControlExperimentPlan, combination: Combination): Promise<ExperimentStatus> {
  await client.start(plan, false);
  const deadline = Date.now() + EXPERIMENT_GRACE_MS + plan.durationMs / combination.context.timeScale;
  for (;;) {
    const status = (await client.status(plan.id)) as ExperimentStatus;
    if (classifyControlExperimentStatus(status) === "terminal") return status;
    if (Date.now() > deadline) {
      await client.abort(plan.id).catch(() => undefined);
      throw new Error(`${combination.label}: the experiment is ${status.status} with restoration ${status.restoration} after its duration`);
    }
    await sleep(50);
  }
}

/**
 * The runtime confirms the restoration in the execution that restores the loop (E5.6), so the sample closing there can still
 * average in the step. Recording until one more sample closes makes the recording end on a sample taken after the restoration.
 */
async function recordOnePeriodMore(session: RuntimeRecordingSession, label: string): Promise<void> {
  const closed = (await session.follow()).sampleCount;
  const deadline = Date.now() + EXPERIMENT_GRACE_MS;
  while ((await session.follow()).sampleCount <= closed) {
    if (Date.now() > deadline) throw new Error(`${label}: the recording closed no sample after the restoration`);
    await sleep(20);
  }
}

function checkExperiment(status: ExperimentStatus, plan: ControlExperimentPlan, label: string): void {
  check(status.status === "completed" && status.stop_reason === "duration_complete" && status.restoration === "restored", label,
    `the experiment ended ${status.status} (${status.stop_reason}) with restoration ${status.restoration}`);
  check(status.last_applied !== null && status.last_applied.value === Number(plan.to), label,
    `the step ${String(plan.to)} was not read back after an execution`);
}

function checkRecording(recording: LoopRecording, plan: ControlExperimentPlan, combination: Combination): void {
  const { label, manual } = combination;
  check(recording.evidence.completeness === "complete", label, `the recording is incomplete: ${recording.evidence.reason}`);
  const samples = recording.rawSamples ?? [];
  const sequence = samples.map(sample => sample.scan);
  check(sequence.length >= 3, label, `the recording holds ${sequence.length} samples`);
  // Each sample covers two executions of the loop's task, so the sequence advances by two (E4.4); only the last sample may be partial.
  const steps = sequence.slice(1).map((scan, index) => scan - sequence[index]);
  const last = steps[steps.length - 1];
  check(steps.slice(0, -1).every(step => step === 2) && last >= 1 && last <= 2, label,
    `consecutive samples advance by ${steps.join(", ")} executions, not by a record period of two`);
  const target = Number(plan.to);
  const near = (value: number | null, expected: number): boolean => value !== null && Math.abs(value - expected) < 1e-9;
  if (manual) {
    check(samples.some(sample => near(sample.appliedMv, target)), label, `the plant input never reached the step ${target}`);
  } else {
    check(samples.some(sample => near(sample.sp, target)), label, `the setpoint never reached the step ${target}`);
    check(near(samples[samples.length - 1].sp, Number(plan.from)), label, `the setpoint was not restored to ${String(plan.from)}`);
  }
}

/** Writes the proportional gain and rolls it back in Manual (E7.3, E7.5); in Auto the write is refused (E7.8). */
async function checkParameter(combination: Combination, loop: ControlLoopDefinition): Promise<void> {
  const { entry, manual, label } = combination;
  const { client } = await connectRuntimeParameters(loop);
  const symbol = entry.proportionalGain;
  const before = await client.status();
  const review = await client.review({ ...before.values, [symbol]: before.values[symbol] * PARAMETER_FACTOR });
  if (!manual) {
    let refused = false;
    try {
      await client.apply(review, false);
    } catch {
      refused = true;
    }
    check(refused, label, `${symbol} was written in Auto, but apply needs Manual (E7.3)`);
    check((await client.status()).values[symbol] === before.values[symbol], label, `${symbol} changed although its write was refused`);
    return;
  }
  const applied = await client.apply(review, false);
  check(applied.status === "applied" && applied.values[symbol] === review.values[symbol], label,
    `${symbol} was not confirmed at ${review.values[symbol]} (status ${applied.status}: ${applied.reason ?? "no reason"})`);
  const rolledBack = await client.rollback(await client.reviewRollback(), false);
  check(rolledBack.values[symbol] === before.values[symbol], label, `the rollback did not restore ${symbol} to ${before.values[symbol]}`);
}

/**
 * The extension's own observation of one loop (SymbolicLoopObservation, E3): each call returns one coherent scan, projected the way
 * the monitor shows it, and `available` waits for a usable scan newer than the one it is given.
 */
class LoopObserver {
  private readonly observation = new SymbolicLoopObservation();
  private readonly monitor: ControlLoopMonitor;
  constructor(private readonly loop: ControlLoopDefinition) {
    this.monitor = new ControlLoopMonitor({ staleAfterMs: controlLoopStaleAfterMs(loop), recordingCapacity: 0 });
  }

  async available(label: string, newerThan?: number): Promise<AvailableLoopSample> {
    const deadline = Date.now() + INSPECTION_TIMEOUT_MS;
    let last: LoopMonitorSample | undefined;
    while (Date.now() < deadline) {
      last = await this.observation.observe(this.monitor, this.loop, await runtimeLifecycleService.snapshot(), Date.now());
      if (last.availability === "available" && (newerThan === undefined || last.scan > newerThan)) return last;
      await sleep(20);
    }
    const reason = last && last.availability === "unavailable" ? last.reason : "no reading";
    throw new Error(`${label}: no fresh coherent scan within ${INSPECTION_TIMEOUT_MS / 1000} s (${reason})`);
  }
}

function writeTable(folder: string, rows: MatrixRow[]): void {
  fs.mkdirSync(folder, { recursive: true });
  fs.writeFileSync(path.join(folder, "protocol-matrix-table.json"), JSON.stringify({
    journey: JOURNEY,
    specification: "docs/specs/35-vscode-control-studio.md Part D, D5; docs/specs/control-studio-runtime-protocol.md E3 to E7",
    combinations: rows.length,
    passed: rows.filter(row => row.outcome === "passed").length,
    rows: rows.map(row => ({ combination: row.label, outcome: row.outcome, processMs: row.processMs, wallMs: row.wallMs, failure: row.failure })),
  }, null, 2));
}

/** Every message names its combination, so a failed row says which of the 32 it is. */
function check(condition: boolean, label: string, message: string): void {
  if (!condition) throw new Error(`${label}: ${message}`);
}

function modeName(manual: boolean): "Manual" | "Auto" {
  return manual ? "Manual" : "Auto";
}

/** The journey keeps no identification checkpoint, so each combination enters its mode from the mode the runtime holds. */
function memoryPersistence() {
  return { read: () => undefined, write: async () => undefined };
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
