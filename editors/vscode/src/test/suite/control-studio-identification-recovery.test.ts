import * as assert from "assert";
import { ConnectedIdentification } from "../../controlStudio/connectedIdentification";
import { identificationRecovery } from "../../controlStudio/connectedIdentificationRecovery";
import { classifyControlExperimentStatus, type ControlExperimentPlan } from "../../controlStudio/experiment";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import type { LoopMonitorSample, LoopRecording } from "../../controlStudio/loopMonitor";
import { refreshTestExperiment, type TestExperimentLifecycle } from "../../controlStudio/testExperimentLifecycle";
import { RuntimeControlError } from "../../runtimeControlClient";

/** A tank whose setpoint is 45 degC, measured in 0–100 degC, driven by a 0–100 % output in Auto. */
function tankLoop(overrides: Record<string, unknown> = {}): ControlLoopDefinition {
  return {
    schemaVersion: 2, id: "tank", name: "Tank", revision: "rev-1", projectId: "project", runtimeId: "runtime", topology: "closed-loop",
    controller: { instance: "Main.TIC101", type: "Control.PID", revision: "sha256:test", action: "direct", taskPeriodMs: 100,
      manualOutputSignal: "Main.TIC101_ManualOutput", manualOutputType: "REAL" },
    signals: {
      setpoint: { symbol: "Main.TIC101_Setpoint", iecType: "LREAL", unit: "degC", range: { minimum: 0, maximum: 100 }, role: "SP" },
      processVariable: { symbol: "Main.TT101.Value", iecType: "LREAL", unit: "degC", range: { minimum: 0, maximum: 100 }, role: "PV" },
      manipulatedVariable: { symbol: "Main.TIC101_Output", iecType: "LREAL", unit: "%", range: { minimum: 0, maximum: 100 }, role: "MV" },
    },
    limits: { outputMinimum: 0, outputMaximum: 100 },
    observedSampleTiming: { nominalPeriodMs: 100 },
    experiment: { execution: "control-studio", kind: "observation", amplitude: 0, durationSeconds: 600, seed: 1 },
    ...overrides,
  } as unknown as ControlLoopDefinition;
}

/** The same tank with an explicit plant-input step: its loop is switched to Manual for the step and back to Auto afterwards. */
function mvStepLoop(): ControlLoopDefinition {
  return tankLoop({ experiment: { execution: "control-studio", kind: "mv_step", amplitude: 10, durationSeconds: 600, seed: 1 } });
}

/** The live sample the runtime reports: the controller mode, the setpoint, the measured output and the output. */
function liveSample(mode: "Auto" | "Manual", pv = 45, mv = 30): LoopMonitorSample {
  return { availability: "available", source: "Simulation", loopId: "tank", loopRevision: "rev-1",
    controller: { instance: "Main.TIC101", revision: "sha256:test", mode }, scan: 7, observedAtMs: 700,
    setpoint: { symbol: "Main.TIC101_Setpoint", value: 45, unit: "degC", quality: "good" },
    processVariable: { symbol: "Main.TT101.Value", value: pv, unit: "degC", quality: "good" },
    manipulatedVariable: { symbol: "Main.TIC101_Output", value: mv, unit: "%", quality: "good" },
    outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, saturation: "none", recording: false } as unknown as LoopMonitorSample;
}

/** A first-order step on process time with a slow drift and a small periodic disturbance, in its step from `startS` until `stopS`. */
interface Plant { base: number; startS: number; stopS: number; change: number; ramp: number; drift: number }

function measured(plant: Plant, seconds: number): number {
  const active = seconds >= plant.startS && seconds < plant.stopS;
  const step = active ? plant.change * (1 - Math.exp(-(seconds - plant.startS) / 50)) : 0;
  const ramp = active ? plant.ramp * (seconds - plant.startS) : 0;
  return plant.base + plant.drift * seconds + step + ramp + 0.025 * (Math.sin(1.7 * seconds) + Math.cos(2.3 * seconds));
}

type Experiment = { id: string; startS: number; durationMs: number; status: string; restoration: string; stopReason: string | null };

/**
 * How a start whose acknowledgement was lost stands in the runtime: acknowledged, accepted and running, accepted and already ended,
 * never received, or accepted but its status unreadable.
 */
type StartOutcome = "acknowledged" | "running" | "terminal" | "absent" | "unreadable";

/** A simulated runtime whose experiment starts, ends and answers status on process time, as the runtime's control surface does. */
class ReconcileRuntime {
  readonly calls: { operation: string; params: Record<string, any> }[] = [];
  experiment: Experiment | undefined;
  plant: Plant = { base: 45, startS: Infinity, stopS: Infinity, change: 0, ramp: 0, drift: 0 };

  constructor(private readonly clock: { now(): number }, private readonly start: StartOutcome = "acknowledged") {}

  readonly request = async (operation: string, params: any): Promise<any> => {
    this.calls.push({ operation, params });
    const now = this.clock.now() / 1_000;
    if (operation === "control.experiment.start") {
      this.accept(params, now);
      if (this.start === "acknowledged") return {};
      throw new Error("the start acknowledgement was lost");
    }
    if (operation === "control.experiment.status" && this.start === "unreadable") throw new Error("transport lost");
    if (!this.experiment) throw new RuntimeControlError("No experiment holds this identifier.", "experiment_not_found");
    if (operation === "control.experiment.finish" && this.experiment.status === "running") {
      this.end(this.experiment, "completed", "finished_early", now);
    }
    if (operation === "control.experiment.abort" && this.experiment.status === "running") this.end(this.experiment, "aborted", "user_abort", now);
    if (!["control.experiment.status", "control.experiment.finish", "control.experiment.abort"].includes(operation)) {
      throw new Error(`unexpected ${operation}`);
    }
    const experiment = this.experiment;
    const due = experiment.startS + experiment.durationMs / 1_000;
    if (experiment.status === "running" && now >= due) this.end(experiment, "completed", "duration_complete", due);
    return { experiment_id: experiment.id, status: experiment.status, restoration: experiment.restoration, stop_reason: experiment.stopReason };
  };

  /** An experiment already running when the host reopens (A8.5). */
  restart(id: string, startS: number, durationMs: number, change: number): void {
    this.experiment = { id, startS, durationMs, status: "running", restoration: "pending", stopReason: null };
    this.plant = { ...this.plant, startS, stopS: Infinity, change };
  }

  private accept(params: any, now: number): void {
    if (this.start === "absent") return;
    if (this.start === "terminal") {
      this.experiment = { id: params.experiment_id, startS: now, durationMs: params.duration_ms, status: "completed",
        restoration: "restored", stopReason: "duration_complete" };
      return;
    }
    this.experiment = { id: params.experiment_id, startS: now, durationMs: params.duration_ms, status: "running", restoration: "pending",
      stopReason: null };
    this.plant = { ...this.plant, base: params.restoration_value, startS: now, stopS: Infinity, change: params.value - params.restoration_value };
  }

  private end(experiment: Experiment, status: string, stopReason: string, atS: number): void {
    experiment.status = status; experiment.restoration = "restored"; experiment.stopReason = stopReason;
    this.plant = { ...this.plant, stopS: atS };
  }
}

/** A clock on the wall that the campaign waits on: each wait moves process and wall time together. */
function fakeClock(startMs = 0) {
  let milliseconds = startMs;
  return { now: () => milliseconds, wait: async (duration: number) => { milliseconds += duration; } };
}

function turn(): Promise<void> {
  return new Promise<void>(resolve => setImmediate(resolve));
}

async function turns(count: number): Promise<void> {
  for (let index = 0; index < count; index += 1) await turn();
}

async function until(condition: () => boolean): Promise<void> {
  for (let index = 0; index < 5_000 && !condition(); index += 1) await turn();
  assert.ok(condition(), "the awaited condition was reached");
}

/** A clock the test moves by hand: each wait only yields to the event loop, so a background follow observes every move. */
function manualClock(startMs: number) {
  const time = { ms: startMs };
  return { time, clock: { now: () => time.ms, wait: () => turn() } };
}

/** The recording a campaign follows: one sample per second of process time up to the clock; its release is logged. */
function recorderFor(clock: { now(): number }, runtime: ReconcileRuntime, log: string[] = []) {
  let next = 0;
  // A sample stands for ten executions of the 100 ms task: a record period of ten, so its sequence steps by ten (A4.2, E4.4).
  const sampleAt = (seconds: number) => ({ elapsedSeconds: seconds, observedAtMs: seconds * 1_000, actualCycleSeconds: 1,
    scan: Math.round(seconds * 10), source: "Simulation", loopRevision: "rev-1", sp: 45, pv: measured(runtime.plant, seconds),
    requestedMv: 30, appliedMv: 30, units: { sp: "degC", pv: "degC", mv: "%" }, quality: "good", freshnessMs: null,
    controllerMode: "Auto", outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, safetyState: "Unavailable", events: [], additional: [] });
  const owner = { schemaVersion: 1, recordingId: "recording-1", projectId: "project", runtimeId: "runtime", loopId: "tank",
    loopRevision: "rev-1", source: "Simulation", programRevision: "program", bindings: [{ id: "pv", symbol: "Main.TT101.Value", iec_type: "LREAL" }] };
  return {
    async follow() {
      const limit = clock.now() / 1_000;
      const rawSamples: ReturnType<typeof sampleAt>[] = [];
      while (next <= limit + 1e-9) { rawSamples.push(sampleAt(next)); next += 1; }
      return { state: "running", elapsedMs: next * 1_000, sampleCount: next, complete: false, reason: null, rawSamples, nextSample: next };
    },
    async stop(): Promise<LoopRecording> {
      const rawSamples = Array.from({ length: next }, (_, index) => sampleAt(index));
      return { rawSamples, evidence: { method: "runtime_scan_buffer", completeness: "complete", recordingId: "recording-1",
        programRevision: "program", actualCycleTimingAvailable: true, missingScanRanges: [], droppedObservedSamples: 0,
        observedSampleCount: rawSamples.length, freshnessAvailable: true, reason: "complete" } } as unknown as LoopRecording;
    },
    async release(): Promise<void> { log.push("release"); },
    recoveryOwner: () => owner,
  };
}

/** The host's ports and the mode double, with the journal and the samples the host keeps. */
function harness(clock: { now(): number; wait(milliseconds: number): Promise<void> }, recorder: ReturnType<typeof recorderFor>,
  mode: "Auto" | "Manual" = "Auto", log: string[] = []) {
  const state = { mode: mode as "Auto" | "Manual", journal: undefined as any, retained: [] as LoopRecording[], cleared: 0,
    modeCalls: [] as string[], fitRuns: [] as number[] };
  const ports = {
    sample: async () => liveSample(state.mode),
    recordingOwner: () => undefined,
    persist: async () => undefined,
    clear: async () => { state.cleared += 1; state.journal = undefined; },
    retain: async (recording: LoopRecording) => { log.push("retain"); state.retained.push(recording); },
    mode: { read: () => undefined, write: async () => undefined },
    campaign: { read: () => state.journal, write: async (value: unknown) => { state.journal = value; } },
  };
  const options = {
    clock,
    startRecording: async () => recorder,
    createMode: () => ({
      processNoise: undefined as number | undefined,
      enter: async (manual: boolean) => { state.modeCalls.push(manual ? "enter-manual" : "enter-auto"); state.mode = manual ? "Manual" : "Auto"; },
      restore: async () => { state.modeCalls.push("restore"); state.mode = "Auto"; },
    }),
    fit: async (_loop: ControlLoopDefinition, runs: readonly unknown[]) => { state.fitRuns.push(runs.length); return { model: {}, runs }; },
  };
  return { state, ports, options };
}

function planFor(id: string, durationMs: number): ControlExperimentPlan {
  return { id, source: "simulation", kind: "setpoint_step", affectedSignal: "Main.TIC101_Setpoint", valueType: "LREAL", from: 45, to: 47,
    unit: "degC", minimum: 0, maximum: 100, durationMs, expectedScan: 300, stopConditions: ["Abort"], restorationValue: 45,
    requiresPhysicalApproval: false, trajectory: [{ atMs: 0, value: 47 }], task: "$background" } as unknown as ControlExperimentPlan;
}

/** A journalled step whose experiment was dispatched: the state a reopened host finds. */
function journalFor(recorder: ReturnType<typeof recorderFor>, plan: ControlExperimentPlan, stepStartSeconds: number) {
  return { schemaVersion: 1, loopRevision: "rev-1", phase: "step", recording: recorder.recoveryOwner(), plan,
    baseline: { mean: 45, noise: 0.03, halfRange: 0.05, spacingSeconds: 1, windowSeconds: 30 }, stepStartSeconds, channelsRemaining: 0 };
}

function bindTo(runtime: ReconcileRuntime) {
  return async () => ({ source: "simulation", endpoint: "", request: runtime.request }) as any;
}

const starts = (runtime: ReconcileRuntime) => runtime.calls.filter(call => call.operation === "control.experiment.start");
const countOf = (runtime: ReconcileRuntime, operation: string) => runtime.calls.filter(call => call.operation === operation).length;

suite("Control Studio reconciles an unknown identification start by its identity", () => {
  test("a start the runtime holds as running is followed to its end, without a second start", async () => {
    const clock = fakeClock();
    const runtime = new ReconcileRuntime(clock, "running");
    const h = harness(clock, recorderFor(clock, runtime));
    await new ConnectedIdentification(runtime.request, h.ports as any, h.options as any).run(mvStepLoop(), async () => undefined);
    assert.strictEqual(starts(runtime).length, 1, "the start is sent once");
    assert.strictEqual(countOf(runtime, "control.experiment.abort"), 0, "nothing is aborted");
    assert.deepStrictEqual(h.state.fitRuns, [1], "the followed experiment is fitted");
    assert.deepStrictEqual(h.state.modeCalls, ["enter-manual", "restore"], "the mode returns once the experiment has ended");
    assert.strictEqual(h.state.journal, undefined);
  });

  test("a start the runtime holds as already ended is concluded: its mode returns and its journal clears, with no second start", async () => {
    const clock = fakeClock();
    const runtime = new ReconcileRuntime(clock, "terminal");
    const h = harness(clock, recorderFor(clock, runtime));
    const outcome = await new ConnectedIdentification(runtime.request, h.ports as any, h.options as any)
      .run(mvStepLoop(), async () => undefined).then(() => "concluded", error => String(error));
    assert.ok(outcome === "concluded" || /measured experiment|stopped before its hold/.test(outcome), outcome);
    assert.strictEqual(starts(runtime).length, 1, "the start is sent once");
    assert.strictEqual(countOf(runtime, "control.experiment.abort"), 0, "nothing is aborted");
    assert.deepStrictEqual(h.state.modeCalls, ["enter-manual", "restore"]);
    assert.strictEqual(h.state.journal, undefined);
  });

  test("a start the runtime never received is cleared: nothing ran, the mode returns, and no second start is sent", async () => {
    const clock = fakeClock();
    const runtime = new ReconcileRuntime(clock, "absent");
    const h = harness(clock, recorderFor(clock, runtime));
    await assert.rejects(new ConnectedIdentification(runtime.request, h.ports as any, h.options as any).run(mvStepLoop(), async () => undefined),
      /holds no experiment/);
    assert.strictEqual(starts(runtime).length, 1, "the start is sent once");
    assert.strictEqual(countOf(runtime, "control.experiment.abort"), 0, "nothing is aborted");
    assert.deepStrictEqual(h.state.modeCalls, ["enter-manual", "restore"]);
    assert.strictEqual(h.state.journal, undefined);
    assert.strictEqual(h.state.retained.length, 1, "the baseline recording is kept");
  });

  test("a start whose status cannot be read keeps its journal and mode for reopening, and sends nothing further", async () => {
    const clock = fakeClock();
    const runtime = new ReconcileRuntime(clock, "unreadable");
    const h = harness(clock, recorderFor(clock, runtime));
    const connected = new ConnectedIdentification(runtime.request, h.ports as any, h.options as any);
    await assert.rejects(connected.run(mvStepLoop(), async () => undefined), /cannot be read/);
    assert.strictEqual(starts(runtime).length, 1, "the start is sent once");
    assert.strictEqual(countOf(runtime, "control.experiment.abort"), 0, "an unreconciled experiment is never aborted");
    assert.deepStrictEqual(h.state.modeCalls, ["enter-manual"], "the mode is not returned while the experiment's state is unknown");
    assert.ok(h.state.journal, "the journal is kept for reconciliation on reopening");
    assert.strictEqual(connected.cancellationOwner().snapshot()?.status, "cancellation-unconfirmed", "dependent work stays blocked");
  });
});

suite("Control Studio reattaches an interrupted identification without aborting it", () => {
  test("a revision mismatch with a running experiment returns no mode until the experiment has ended", async () => {
    const { time, clock } = manualClock(10_000);
    const runtime = new ReconcileRuntime(clock);
    runtime.restart("experiment-1", 0, 60_000, 2);
    const recorder = recorderFor(clock, runtime);
    const h = harness(clock, recorder);
    h.state.journal = journalFor(recorder, planFor("experiment-1", 60_000), 0);
    const modeCalls: string[] = [];
    let completedCalls = 0;
    const mode = { processNoise: undefined, enter: async () => undefined, restore: async () => { modeCalls.push("restore"); } };
    const ports = { ...h.ports, mode: { read: () => ({ loopRevision: "rev-1", original: { program_revision: "program", values: {} } }),
      write: async () => undefined } };
    const recovery = identificationRecovery(tankLoop({ revision: "rev-2" }), ports as any, () => { completedCalls += 1; },
      { clock, bind: bindTo(runtime), recover: async () => recorder as any, createMode: () => mode as any });
    await assert.rejects(recovery!.cancel(), /still running/);
    await turns(20);
    assert.deepStrictEqual(modeCalls, [], "the mode is not returned underneath a running experiment");
    assert.strictEqual(runtime.experiment?.status, "running");
    time.ms = 80_000;   // the experiment's duration has passed
    await until(() => completedCalls === 1);
    assert.deepStrictEqual(modeCalls, ["restore"], "the mode returns once the experiment has ended");
    assert.strictEqual(countOf(runtime, "control.experiment.abort"), 0, "nothing is aborted");
    assert.strictEqual(recovery!.snapshot(), undefined);
  });

  test("reopening reattaches a running experiment, finishes it once settled, returns the mode, then fits, and never aborts it", async () => {
    const clock = fakeClock(30_000);
    const runtime = new ReconcileRuntime(clock);
    runtime.restart("experiment-1", 30, 540_000, 2);
    const recorder = recorderFor(clock, runtime);
    const h = harness(clock, recorder);
    h.state.journal = journalFor(recorder, planFor("experiment-1", 540_000), 30);
    const modeAtFit: string[][] = [];
    const fit = async (_loop: unknown, runs: readonly unknown[]) => {
      modeAtFit.push([...h.state.modeCalls]);
      h.state.fitRuns.push(runs.length);
      return { model: {}, runs };
    };
    const recovery = identificationRecovery(tankLoop(), h.ports as any, () => undefined,
      { clock, bind: bindTo(runtime), recover: async () => recorder as any, createMode: h.options.createMode as any, fit: fit as any });
    const result = await recovery!.resume!(async () => undefined);
    assert.strictEqual(countOf(runtime, "control.experiment.finish"), 1, "the settled step is finished once");
    assert.deepStrictEqual(modeAtFit, [["restore"]], "the mode is returned before the fit");
    assert.deepStrictEqual(h.state.fitRuns, [1]);
    assert.strictEqual(countOf(runtime, "control.experiment.abort"), 0, "nothing is aborted");
    assert.ok(result);
    assert.strictEqual(h.state.journal, undefined);
  });

  test("closing a reattached identification releases its clients without aborting, restoring or clearing anything", async () => {
    const { clock } = manualClock(30_000);
    const runtime = new ReconcileRuntime(clock);
    runtime.restart("experiment-1", 30, 540_000, 2);
    const recorder = recorderFor(clock, runtime);
    const h = harness(clock, recorder);
    h.state.journal = journalFor(recorder, planFor("experiment-1", 540_000), 30);
    let completedCalls = 0;
    const recovery = identificationRecovery(tankLoop(), h.ports as any, () => { completedCalls += 1; },
      { clock, bind: bindTo(runtime), recover: async () => recorder as any, createMode: h.options.createMode as any });
    const reattached = recovery!.resume!(async () => undefined);
    await turns(20);
    const callsAtRelease = runtime.calls.length;
    recovery!.release!();
    await assert.rejects(reattached, /released/);
    await turns(20);
    assert.strictEqual(runtime.calls.length, callsAtRelease, "a released client sends nothing further");
    assert.strictEqual(countOf(runtime, "control.experiment.abort"), 0, "closing never aborts");
    assert.strictEqual(runtime.experiment?.status, "running", "the runtime keeps the experiment");
    assert.ok(h.state.journal, "the journal stays for reopening");
    assert.deepStrictEqual(h.state.modeCalls, [], "a released client does not return the original mode");
    assert.strictEqual(completedCalls, 0);
  });

  test("reopening an identification whose experiment never started clears its journal, restores the mode and completes the recovery", async () => {
    const clock = fakeClock(30_000);
    const runtime = new ReconcileRuntime(clock, "absent");
    const recorder = recorderFor(clock, runtime);
    const h = harness(clock, recorder);
    h.state.journal = journalFor(recorder, planFor("experiment-1", 540_000), 30);
    let completedCalls = 0;
    const recovery = identificationRecovery(tankLoop(), h.ports as any, () => { completedCalls += 1; },
      { clock, bind: bindTo(runtime), recover: async () => recorder as any, createMode: h.options.createMode as any });
    await assert.rejects(recovery!.resume!(async () => undefined), /never started/);
    assert.strictEqual(countOf(runtime, "control.experiment.abort"), 0);
    assert.strictEqual(starts(runtime).length, 0, "no start is sent on reopening");
    assert.deepStrictEqual(h.state.modeCalls, ["restore"]);
    assert.strictEqual(h.state.journal, undefined);
    assert.strictEqual(h.state.retained.length, 1, "the recording is kept");
    assert.strictEqual(completedCalls, 1, "nothing is left owned, so the recovery completes");
    assert.strictEqual(recovery!.snapshot(), undefined);
  });
});

suite("Control Studio keeps an identification's recording before releasing its buffer", () => {
  test("a completed identification keeps its recording, then releases the runtime buffer once", async () => {
    const clock = fakeClock();
    const runtime = new ReconcileRuntime(clock, "acknowledged");
    const log: string[] = [];
    const h = harness(clock, recorderFor(clock, runtime, log), "Auto", log);
    await new ConnectedIdentification(runtime.request, h.ports as any, h.options as any).run(tankLoop(), async () => undefined);
    assert.deepStrictEqual(log, ["retain", "release"], "the samples are kept before the buffer is released, exactly once");
    assert.strictEqual(h.state.fitRuns.length, 1);
  });
});

suite("Control Studio Test page reopening reconciles a start journalled with its identity only", () => {
  /** A lifecycle whose recording owner was never journalled: the identity alone is what reopening finds. */
  function identityOnly(status: () => Promise<unknown>) {
    const seen = { posted: [] as [string, unknown][], scheduled: 0, cleared: 0, recovered: 0, terminal: [] as unknown[] };
    const lifecycle: TestExperimentLifecycle = {
      experimentId: () => "experiment-1",
      loop: () => tankLoop(),
      recovery: () => ({ experimentId: "experiment-1" }) as any,
      recordingActive: () => false,
      recoverOwner: async () => { seen.recovered += 1; },
      recovered: () => { seen.recovered += 1; },
      status: () => status(),
      classify: classifyControlExperimentStatus,
      terminal: async value => ({ concluded: value }),
      terminalObserved: value => { seen.terminal.push(value); },
      clear: async () => { seen.cleared += 1; },
      post: async (phase, payload) => { seen.posted.push([phase, payload]); },
      schedule: () => { seen.scheduled += 1; },
    };
    return { lifecycle, seen };
  }

  test("a running identity is attached and followed, without recovering a recording owner", async () => {
    const { lifecycle, seen } = identityOnly(async () => ({ experiment_id: "experiment-1", status: "running", restoration: "pending",
      stop_reason: null }));
    await refreshTestExperiment(lifecycle);
    assert.deepStrictEqual(seen.posted.map(([phase]) => phase), ["running"]);
    assert.strictEqual(seen.scheduled, 1, "the experiment keeps being followed");
    assert.strictEqual(seen.recovered, 0, "no recording owner is recovered from an identity alone");
    assert.strictEqual(seen.cleared, 0);
  });

  test("a terminal identity is concluded and its journal cleared", async () => {
    const { lifecycle, seen } = identityOnly(async () => ({ experiment_id: "experiment-1", status: "completed", restoration: "restored",
      stop_reason: "duration_complete" }));
    await refreshTestExperiment(lifecycle);
    assert.deepStrictEqual(seen.posted.map(([phase]) => phase), ["terminal"]);
    assert.strictEqual(seen.terminal.length, 1);
    assert.strictEqual(seen.cleared, 1, "a concluded identity is cleared");
  });

  test("an identity the runtime does not hold is cleared, since nothing was applied", async () => {
    const { lifecycle, seen } = identityOnly(async () => {
      throw new RuntimeControlError("No experiment holds this identifier.", "experiment_not_found");
    });
    await refreshTestExperiment(lifecycle);
    assert.deepStrictEqual(seen.posted.map(([phase]) => phase), ["failed"]);
    assert.match(String((seen.posted[0][1] as { reason: string }).reason), /holds no experiment/);
    assert.strictEqual(seen.cleared, 1);
  });

  test("an identity whose status cannot be read stays journalled for the next reconciliation", async () => {
    const { lifecycle, seen } = identityOnly(async () => {
      throw new Error("transport lost");
    });
    await refreshTestExperiment(lifecycle);
    assert.deepStrictEqual(seen.posted.map(([phase]) => phase), ["failed"]);
    assert.match(String((seen.posted[0][1] as { reason: string }).reason), /could not be read/);
    assert.strictEqual(seen.cleared, 0, "an unreadable identity is kept");
  });
});
