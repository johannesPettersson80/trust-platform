import * as assert from "assert";
import { ConnectedIdentification } from "../../controlStudio/connectedIdentification";
import { ControlExperimentClient, type ControlExperimentPlan } from "../../controlStudio/experiment";
import { formatDuration, formatRate, IdentificationCampaign } from "../../controlStudio/identificationCampaign";
import { identificationAmplitude, identificationMethod } from "../../controlStudio/identificationPolicy";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import type { LoopMonitorSample, LoopRecording } from "../../controlStudio/loopMonitor";
import { baselineWindowSeconds, hasSettled, noiseSigma } from "../../controlStudio/settleDetection";

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

/** The live sample the runtime reports: the controller mode, the setpoint, the measured output and the output. */
function liveSample(mode: "Auto" | "Manual", pv = 45, mv = 30): LoopMonitorSample {
  return { availability: "available", source: "Simulation", loopId: "tank", loopRevision: "rev-1",
    controller: { instance: "Main.TIC101", revision: "sha256:test", mode }, scan: 7, observedAtMs: 700,
    setpoint: { symbol: "Main.TIC101_Setpoint", value: 45, unit: "degC", quality: "good" },
    processVariable: { symbol: "Main.TT101.Value", value: pv, unit: "degC", quality: "good" },
    manipulatedVariable: { symbol: "Main.TIC101_Output", value: mv, unit: "%", quality: "good" },
    outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, saturation: "none", recording: false } as unknown as LoopMonitorSample;
}

function amplitudeInput(loop: ControlLoopDefinition, sample: LoopMonitorSample, kind: string, overrides: Record<string, unknown> = {}): any {
  return { loop, sample, kind, automatic: true, noise: 0.1, validation: false, fallbackAmplitude: 10, onOff: false, ...overrides };
}

/** Floating-point sizes are compared within a tolerance: 2 % of 100 is computed, not typed. */
function near(actual: number, expected: number, message = ""): void {
  assert.ok(Math.abs(actual - expected) < 1e-9, `${actual} is not ${expected}${message ? `: ${message}` : ""}`);
}

/** A simulated first-order process on process time: a step of `change` with a 50 s time constant, a ramp, a drift, an offset
 * that decays with `offsetTauS` (a previous test still dying away), and a noise of about 0.03. It is in its step from `startS`
 * until `stopS`. */
interface Plant { base: number; startS: number; stopS: number; change: number; ramp: number; drift: number; offset: number; offsetTauS: number }

function measured(plant: Plant, seconds: number): number {
  const active = seconds >= plant.startS && seconds < plant.stopS;
  const step = active ? plant.change * (1 - Math.exp(-(seconds - plant.startS) / 50)) : 0;
  const ramp = active ? plant.ramp * (seconds - plant.startS) : 0;
  const decaying = plant.offsetTauS > 0 ? plant.offset * Math.exp(-seconds / plant.offsetTauS) : 0;
  return plant.base + plant.drift * seconds + decaying + step + ramp + 0.025 * (Math.sin(1.7 * seconds) + Math.cos(2.3 * seconds));
}

/** A simulated runtime: it starts and ends experiments on process time, and answers their status, finish and abort requests. */
class FakeRuntime {
  readonly calls: { operation: string; params: Record<string, any> }[] = [];
  experiment: { id: string; startS: number; durationMs: number; status: string; restoration: string; stopReason: string | null } | undefined;
  plant: Plant;
  readonly request = async (operation: string, params: any): Promise<any> => {
    this.calls.push({ operation, params });
    const now = this.clock.now() / 1_000;
    if (operation === "control.experiment.start") {
      this.experiment = { id: params.experiment_id, startS: now, durationMs: params.duration_ms, status: "running",
        restoration: "pending", stopReason: null };
      this.plant = { ...this.plant, base: params.restoration_value, startS: now, stopS: Infinity, change: params.value - params.restoration_value };
      return {};
    }
    const experiment = this.experiment;
    if (!experiment) throw new Error(`${operation} without an experiment`);
    if (operation === "control.experiment.finish" && experiment.status === "running") this.end(experiment, "completed", "finished_early", now);
    if (operation === "control.experiment.abort" && experiment.status === "running") this.end(experiment, "aborted", "user_abort", now);
    const due = experiment.startS + experiment.durationMs / 1_000;
    if (experiment.status === "running" && now >= due) this.end(experiment, "completed", "duration_complete", due);
    if (!["control.experiment.status", "control.experiment.finish", "control.experiment.abort"].includes(operation)) {
      throw new Error(`unexpected ${operation}`);
    }
    return { experiment_id: experiment.id, status: experiment.status, restoration: experiment.restoration, stop_reason: experiment.stopReason };
  };

  constructor(private readonly clock: { now(): number }, options: { ramp?: number; drift?: number; offset?: number; offsetTauS?: number } = {}) {
    this.plant = { base: 45, startS: Infinity, stopS: Infinity, change: 0, ramp: options.ramp ?? 0, drift: options.drift ?? 0,
      offset: options.offset ?? 0, offsetTauS: options.offsetTauS ?? 0 };
  }

  /** An experiment already running when the host restarted (A8.5). */
  restart(id: string, startS: number, durationMs: number, change: number): void {
    this.experiment = { id, startS, durationMs, status: "running", restoration: "pending", stopReason: null };
    this.plant = { ...this.plant, startS, stopS: Infinity, change };
  }

  private end(experiment: { status: string; restoration: string; stopReason: string | null }, status: string, stopReason: string, atS: number): void {
    experiment.status = status; experiment.restoration = "restored"; experiment.stopReason = stopReason;
    this.plant = { ...this.plant, stopS: atS };
  }
}

/** A clock on the wall; the process time of the fake runtime is the same clock, unless a test delivers samples instantly. */
function fakeClock(startMs = 0) {
  let milliseconds = startMs;
  return { now: () => milliseconds, wait: async (duration: number) => { milliseconds += duration; } };
}

/** The recording a campaign follows: one sample per second of process time up to the clock, or up to `throughS` at once. */
function recorderFor(clock: { now(): number }, runtime: FakeRuntime, options: { throughS?: number; silent?: boolean } = {}) {
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
      const limit = options.throughS ?? clock.now() / 1_000;
      const rawSamples: ReturnType<typeof sampleAt>[] = [];
      while (!options.silent && next <= limit + 1e-9) { rawSamples.push(sampleAt(next)); next += 1; }
      return { state: "running", elapsedMs: next * 1_000, sampleCount: next, complete: false, reason: null, rawSamples, nextSample: next };
    },
    async stop(): Promise<LoopRecording> {
      const rawSamples = Array.from({ length: next }, (_, index) => sampleAt(index));
      return { rawSamples, evidence: { method: "runtime_scan_buffer", completeness: "complete", recordingId: "recording-1",
        programRevision: "program", actualCycleTimingAvailable: true, missingScanRanges: [], droppedObservedSamples: 0,
        observedSampleCount: rawSamples.length, freshnessAvailable: true, reason: "complete" } } as unknown as LoopRecording;
    },
    recoveryOwner: () => owner,
  };
}

/** The host's ports, the mode and fit doubles, and the journal, around one simulated runtime. */
function harness(clock: { now(): number }, recorder: ReturnType<typeof recorderFor>, mode: "Auto" | "Manual" = "Auto") {
  const state = { mode: mode as "Auto" | "Manual", journal: undefined as any, retained: [] as LoopRecording[], cleared: 0,
    modeCalls: [] as string[], fitRuns: [] as number[] };
  const ports = {
    sample: async () => liveSample(state.mode),
    recordingOwner: () => undefined,
    persist: async () => undefined,
    clear: async () => { state.cleared += 1; state.journal = undefined; },
    retain: async (recording: LoopRecording) => { state.retained.push(recording); },
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

function starts(runtime: FakeRuntime): Record<string, any>[] {
  return runtime.calls.filter(call => call.operation === "control.experiment.start").map(call => call.params);
}

suite("Control Studio long-process identification campaign", () => {
  test("Automatic identifies a loop in Auto with a setpoint by a setpoint step, sized by the noise and the range", () => {
    const loop = tankLoop();
    assert.deepStrictEqual(identificationMethod(loop, liveSample("Auto")), { kind: "setpoint_step", manual: false, automatic: true });
    near(identificationAmplitude(amplitudeInput(loop, liveSample("Auto"), "setpoint_step", { noise: 0.1 })), 2,
      "20 times the noise is 2, the same as 2 % of the 100 degC range");
    near(identificationAmplitude(amplitudeInput(loop, liveSample("Auto"), "setpoint_step", { noise: 0.5 })), 10,
      "20 times the noise is 10, capped at 10 % of the range");
    near(identificationAmplitude(amplitudeInput(loop, liveSample("Auto", 99), "setpoint_step", { noise: 0.1 })), -2,
      "with 99 degC measured the larger headroom is below, so the step goes down");
    near(identificationAmplitude(amplitudeInput(loop, liveSample("Auto", 99), "setpoint_step", { noise: 0.1, validation: true })), 1,
      "the model check goes the opposite way and is capped by the 1 degC headroom above 99");
  });

  test("Automatic identifies a loop in Manual by a plant-input step, sized by the input range and its headroom", () => {
    const loop = tankLoop();
    assert.deepStrictEqual(identificationMethod(loop, liveSample("Manual")), { kind: "mv_step", manual: true, automatic: true });
    near(identificationAmplitude(amplitudeInput(loop, liveSample("Manual"), "mv_step")), 10, "10 % of the 100 % input range");
    near(identificationAmplitude(amplitudeInput(loop, liveSample("Manual"), "mv_step", { validation: true })), -7.5,
      "the model check steps the other way at 0.75 times the size");
    near(identificationAmplitude(amplitudeInput(loop, liveSample("Manual", 45, 0), "mv_step", { validation: true })), 7.5,
      "at 0 % there is no room below, so the model check keeps the direction at 0.75 times the size: a different size (B5.2)");
  });

  test("Automatic uses a pulse when sustained holds are not permitted and PRBS when the policy asks for frequency content", () => {
    const pulseOnly = tankLoop({ identificationPolicy: { openLoopPermitted: true, sustainedHoldPermitted: false, information: "basic" } });
    assert.strictEqual(identificationMethod(pulseOnly, liveSample("Manual")).kind, "mv_pulse");
    const frequency = tankLoop({ identificationPolicy: { openLoopPermitted: true, sustainedHoldPermitted: true, information: "frequency" } });
    assert.strictEqual(identificationMethod(frequency, liveSample("Manual")).kind, "prbs");
  });

  test("Automatic has no safe method for a loop in Auto without a setpoint where open-loop tests are not permitted", () => {
    const withoutSetpoint = { ...tankLoop().signals, setpoint: undefined } as ControlLoopDefinition["signals"];
    assert.deepStrictEqual(identificationMethod(tankLoop({ signals: withoutSetpoint }), liveSample("Auto")),
      { kind: "mv_step", manual: true, automatic: true }, "open-loop tests permitted: a plant-input step in Manual");
    const forbidden = tankLoop({ signals: withoutSetpoint,
      identificationPolicy: { openLoopPermitted: false, sustainedHoldPermitted: true, information: "basic" } });
    assert.throws(() => identificationMethod(forbidden, liveSample("Auto")), /no declared setpoint binding/);
  });

  test("Automatic never switches a loop in Manual into Auto: without open-loop tests it has no safe method", () => {
    const closedOnly = tankLoop({ identificationPolicy: { openLoopPermitted: false, sustainedHoldPermitted: true, information: "basic" } });
    assert.deepStrictEqual(identificationMethod(closedOnly, liveSample("Auto")), { kind: "setpoint_step", manual: false, automatic: true });
    assert.throws(() => identificationMethod(closedOnly, liveSample("Manual")), /loop is in Manual.*Put the loop in Auto/);
  });

  test("an explicitly chosen method is kept, and a plant-input method for a loop in Auto is marked to switch to Manual", () => {
    const explicit = tankLoop({ experiment: { execution: "control-studio", kind: "mv_step", amplitude: 10, durationSeconds: 600, seed: 1 } });
    assert.deepStrictEqual(identificationMethod(explicit, liveSample("Auto")), { kind: "mv_step", manual: true, automatic: false });
  });

  test("the baseline window is the longest of 20 samples, 5 % of the maximum and 10 % of a declared settling time, capped at 30 minutes", () => {
    near(baselineWindowSeconds(6_000, 1), 300, "5 % of 100 minutes is 5 minutes");
    near(baselineWindowSeconds(6_000, 1, 4_000), 400, "10 % of a declared 4000 s settling time is longer");
    near(baselineWindowSeconds(60_000, 1, 100_000), 1_800, "capped at 30 minutes");
  });

  test("the noise is the root mean square of the first differences over the square root of two", () => {
    assert.ok(Math.abs(noiseSigma([0, 1, 0, 1]) - Math.sqrt(0.5)) < 1e-12);
  });

  test("a settled response is recognised, and a ramp that never settles is not", () => {
    const exponential = Array.from({ length: 501 }, (_, t) => ({ seconds: t, value: 2 * (1 - Math.exp(-t / 50)) }));
    assert.ok(hasSettled(exponential, { mean: 0, noise: 0.01, spacingSeconds: 1 }, 0));
    const ramp = Array.from({ length: 501 }, (_, t) => ({ seconds: t, value: 0.01 * t }));
    assert.ok(!hasSettled(ramp, { mean: 0, noise: 0.01, spacingSeconds: 1 }, 0));
  });

  test("durations and rates are read in plain words", () => {
    assert.strictEqual(formatDuration(720), "12 min");
    assert.strictEqual(formatDuration(13_800), "3 h 50 min");
    assert.strictEqual(formatRate(0.01, "degC"), "0.6 degC/min");
  });

  test("a 5-minute baseline completes in process time when the samples arrive instantly", async () => {
    const clock = fakeClock();
    const runtime = new FakeRuntime(clock);
    const recorder = recorderFor(clock, runtime, { throughS: 400 });
    const campaign = new IdentificationCampaign({ loop: tankLoop(), recorder: recorder as any, experiment: new ControlExperimentClient(runtime.request),
      clock, progress: async () => undefined, cancelled: () => false, offerSetpointStep: true });
    const baseline = await campaign.baseline(100 * 60_000);
    near(baseline.windowSeconds, 300, "100 minutes at 5 % is a 5-minute window");
    assert.ok(clock.now() < 300_000, "no five minutes of wall time were waited for process time that had already passed");
    assert.ok(Math.abs(baseline.mean - 45) < 0.1);
  });

  test("a drifting output is refused, with its rate in the message", async () => {
    const clock = fakeClock();
    const runtime = new FakeRuntime(clock, { drift: 0.01 });
    const campaign = new IdentificationCampaign({ loop: tankLoop(), recorder: recorderFor(clock, runtime) as any,
      experiment: new ControlExperimentClient(runtime.request), clock, progress: async () => undefined, cancelled: () => false,
      offerSetpointStep: true });
    await assert.rejects(campaign.baseline(3_600_000),
      /Main\.TT101\.Value is still changing by 0\.6 degC\/min\. Wait for it to settle, or identify with a setpoint step\./);
  });

  test("a loop in Auto with a setpoint steps its setpoint without a mode change, and finishes the step once it has settled", async () => {
    const clock = fakeClock();
    const runtime = new FakeRuntime(clock);
    const h = harness(clock, recorderFor(clock, runtime));
    const result = await new ConnectedIdentification(runtime.request, h.ports as any, h.options as any).run(tankLoop(), async () => undefined);
    assert.deepStrictEqual(h.state.modeCalls, [], "a setpoint step in Auto keeps the loop in Auto");
    assert.strictEqual(starts(runtime).length, 1);
    assert.strictEqual(starts(runtime)[0].kind, "setpoint_step");
    assert.strictEqual(runtime.calls.filter(call => call.operation === "control.experiment.finish").length, 1,
      "the response settled before the hold ended, so the step is finished once");
    assert.strictEqual(runtime.experiment?.stopReason, "finished_early");
    assert.deepStrictEqual(h.state.fitRuns, [1]);
    assert.ok(result);
  });

  test("a response that never settles runs to its maximum duration, without an early finish", async () => {
    const clock = fakeClock();
    const runtime = new FakeRuntime(clock, { ramp: 0.01 });
    const h = harness(clock, recorderFor(clock, runtime));
    await new ConnectedIdentification(runtime.request, h.ports as any, h.options as any).run(tankLoop(), async () => undefined);
    assert.strictEqual(runtime.calls.filter(call => call.operation === "control.experiment.finish").length, 0);
    assert.strictEqual(runtime.experiment?.stopReason, "duration_complete", "the runtime ends the hold by its duration");
    assert.deepStrictEqual(h.state.fitRuns, [1]);
  });

  test("an explicit plant-input method on a loop in Auto switches to Manual and returns to Auto", async () => {
    const clock = fakeClock();
    const runtime = new FakeRuntime(clock);
    const h = harness(clock, recorderFor(clock, runtime));
    const explicit = tankLoop({ experiment: { execution: "control-studio", kind: "mv_step", amplitude: 10, durationSeconds: 600, seed: 1 } });
    await new ConnectedIdentification(runtime.request, h.ports as any, h.options as any).run(explicit, async () => undefined);
    assert.deepStrictEqual(h.state.modeCalls, ["enter-manual", "restore"], "the loop is in Manual for the step and back in Auto afterwards");
    assert.strictEqual(starts(runtime)[0].kind, "mv_step");
  });

  test("the independent model check records the opposite excitation and does not fit", async () => {
    const clock = fakeClock();
    const runtime = new FakeRuntime(clock);
    const h = harness(clock, recorderFor(clock, runtime));
    const result = await new ConnectedIdentification(runtime.request, h.ports as any, h.options as any).recordValidation(tankLoop(), async () => undefined);
    const start = starts(runtime)[0];
    assert.ok(Number(start.value) < Number(start.restoration_value), "a setpoint step of the opposite sign to the identification step");
    assert.ok(result.run, "the check returns its recording");
    assert.deepStrictEqual(h.state.fitRuns, []);
  });

  test("the five-second supervision rule stops a campaign that receives no samples, and restores it", async () => {
    const clock = fakeClock();
    const runtime = new FakeRuntime(clock);
    const h = harness(clock, recorderFor(clock, runtime, { silent: true }));
    const connected = new ConnectedIdentification(runtime.request, h.ports as any, h.options as any);
    await assert.rejects(connected.run(tankLoop(), async () => undefined), /The runtime stopped sending samples\./);
    assert.strictEqual(starts(runtime).length, 0, "no experiment was started");
    assert.strictEqual(h.state.retained.length, 1, "the recording is stopped and kept");
    assert.strictEqual(h.state.cleared, 1, "the journal is cleared once nothing is left running");
    assert.strictEqual(connected.snapshot().status, "failed");
  });

  test("a missing limit throws an error that names the missing field", async () => {
    const clock = fakeClock();
    const runtime = new FakeRuntime(clock);
    const h = harness(clock, recorderFor(clock, runtime));
    const runWith = (loop: ControlLoopDefinition) => new ConnectedIdentification(runtime.request, h.ports as any, h.options as any)
      .run(loop, async () => undefined);
    await assert.rejects(runWith(tankLoop({ limits: { outputMinimum: 0 } })), /limits\.outputMinimum/);
    await assert.rejects(runWith(tankLoop({ experiment: { execution: "control-studio", kind: "observation", amplitude: 0, seed: 1 } })),
      /maximum experiment duration/);
  });

  test("reattaching after a restart follows the running experiment from its first sample, finishes it once settled, and fits", async () => {
    const clock = fakeClock(30_000);
    const runtime = new FakeRuntime(clock);
    runtime.restart("experiment-1", 30, 540_000, 2);
    const recorder = recorderFor(clock, runtime);
    const h = harness(clock, recorder);
    const plan = { id: "experiment-1", source: "simulation", kind: "setpoint_step", affectedSignal: "Main.TIC101_Setpoint",
      valueType: "LREAL", from: 45, to: 47, unit: "degC", minimum: 0, maximum: 100, durationMs: 540_000, expectedScan: 300,
      stopConditions: ["Abort"], restorationValue: 45, requiresPhysicalApproval: false,
      trajectory: [{ atMs: 0, value: 47 }], task: "$background" } as unknown as ControlExperimentPlan;
    h.state.journal = { schemaVersion: 1, loopRevision: "rev-1", phase: "step", recording: recorder.recoveryOwner(), plan,
      baseline: { mean: 45, noise: 0.03, halfRange: 0.05, spacingSeconds: 1, windowSeconds: 30 }, stepStartSeconds: 30, channelsRemaining: 0 };
    const result = await new ConnectedIdentification(runtime.request, h.ports as any, h.options as any)
      .resume(tankLoop(), recorder as any, async () => undefined);
    assert.strictEqual(runtime.calls.filter(call => call.operation === "control.experiment.finish").length, 1);
    assert.deepStrictEqual(h.state.fitRuns, [1]);
    assert.ok(result);
    assert.strictEqual(h.state.journal, undefined, "the journal is cleared once the campaign has ended");
  });

  test("a long wait for a steady start never shortens the step, and the recording covers the wait and the maximum duration at a record period that follows it (B3.4, A4.2)", async () => {
    const clock = fakeClock();
    // The output is still dying away from an earlier test: 4 degC above its rest, with a 40 s time constant.
    const runtime = new FakeRuntime(clock, { offset: 4, offsetTauS: 40 });
    const recorder = recorderFor(clock, runtime);
    const h = harness(clock, recorder);
    let plannedMs: number | undefined, settlingSeconds: number | undefined;
    (h.options as any).startRecording = async (_loop: unknown, _persistence: unknown, duration: number, settling: number) => {
      plannedMs = duration; settlingSeconds = settling; return recorder;
    };
    let fitted: any[] = [];
    (h.options as any).fit = async (_loop: unknown, runs: readonly any[]) => { fitted = [...runs]; return { model: {}, runs }; };
    await new ConnectedIdentification(runtime.request, h.ports as any, h.options as any).run(tankLoop(), async () => undefined);
    const [start] = starts(runtime);
    assert.ok(start, "the step was sent once the output had settled");
    assert.ok(runtime.plant.startS > 60, `the step waited for the decaying output to settle: it started at ${runtime.plant.startS} s`);
    // A 600 s maximum and a 30 s baseline window (5 % of the maximum): the step holds 570 s however long the wait took.
    assert.strictEqual(start.duration_ms, 570_000);
    // The recording spans the baseline budget (the longer of 5 min and 20 % of the maximum) and then the maximum duration.
    assert.strictEqual(plannedMs, 300_000 + 600_000);
    // The settling time is not known yet, so the record period follows a tenth of the maximum duration (A4.2).
    assert.strictEqual(settlingSeconds, 60);
    // The record keeps the whole recording, as its evidence describes it, and names where the steady baseline window starts:
    // one 30 s window before the step. The fit starts there, so the tail (49 degC at the start) never sets the operating point.
    const [result] = fitted;
    const pv = (sample: any) => sample.values["Main.TT101.Value"].value;
    assert.ok(pv(result.run.samples[0]) > 48, "the retained recording still begins in the tail");
    assert.ok(Math.abs(result.request.baselineStartSeconds - (runtime.plant.startS - 30)) <= 1.5,
      `the steady window starts one window before the step: ${result.request.baselineStartSeconds} s, step at ${runtime.plant.startS} s`);
    const firstFitted = result.run.samples.find((sample: any) => sample.timeSeconds >= result.request.baselineStartSeconds);
    assert.ok(pv(firstFitted) < 45.6, `the fit starts in the steady window, not in the tail: ${pv(firstFitted)} degC`);
  });
});
