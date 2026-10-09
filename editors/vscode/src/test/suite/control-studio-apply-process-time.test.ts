import * as assert from "assert";

import { channel, pidInitialization, pidLoop } from "./control-studio-pid-fixtures";
import { finalTenthStart, independentResponseReason, scoreIndependentResponse } from "../../controlStudio/independentModelScore";
import { configuredPlantDrivesLoop, configuredPlantRequest, isolatedPlantName, readConfiguredPlantBinding, verificationPlant,
  type ConfiguredPlantBinding, type VerificationPlant } from "../../controlStudio/configuredPlantVerification";
import { assertCommissioningStartingPoint, awaitCommissioningStartingPoint, StartingPointWaitCancelled } from "../../controlStudio/commissioningStartingPoint";
import { assessObservation, followAppliedResponse, ObservationRecord, ObservationRequirementFailed, observedRangesOf, PermittedRangeLeft, resumeApplyObservation, ObservationDetached } from "../../controlStudio/applyObservation";
import { classifyMainWorkflowFailure, MainWorkflowError } from "../../controlStudio/mainWorkflowFailure";
import { restoreUnavailableReason } from "../../controlStudio/restoreAvailability";
import { RuntimeTransferClient, transferRejection } from "../../controlStudio/runtimeTransfer";
import { RuntimeRecordingSession } from "../../controlStudio/runtimeRecording";
import { PidCommissioning } from "../../controlStudio/pidCommissioning";
import { ProportionalCommissioning } from "../../controlStudio/proportionalCommissioning";
import { pidTransferBinding } from "../../controlStudio/pidTransfer";
import { proportionalTransferBinding } from "../../controlStudio/proportionalTransfer";

/** Specification 35: validation (B5), application (B6) and the mode an operation returns the loop to (A6.2). */
const CONFIGURED: VerificationPlant = { kind: "configured", plantId: "plant-1" };
/** The permitted range of the measured output in the observation tests. */
const RANGE = { symbol: "P1.RoomC", unit: "degC", minimum: 0, maximum: 10 };
/** The requirements of the process-time wait tests: 4 s of test duration, a 0.02 settling band. */
const TASK: any = { referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 };

function singleOutputModel(): any {
  return { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0, revision: "first-order", source: "provided",
    validation: "validated", evidence: ["bench"], normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")],
    operatingPoint: { inputs: [0], outputs: [2] } };
}

function twoOutputModel(): any {
  return { ...singleOutputModel(), outputs: [channel("y", "m"), channel("z", "m")], operatingPoint: { inputs: [0], outputs: [2, 5] } };
}

function pick(values: Record<string, number | boolean>, symbols: readonly string[]): Record<string, number> {
  const picked: Record<string, number> = {};
  for (const symbol of symbols) picked[symbol] = Number(values[symbol]);
  return picked;
}

type Kind = "pid" | "proportional";

/**
 * A fake runtime for one reviewed commissioning. The controller transfers and the parameter record share one store, as they do in
 * the runtime, so the mode and the gain can be read back after a failure (A6.2, B6.3, B6.4). Only the runtime's answers are
 * replaced: the commissioning classes run unchanged.
 */
function fakeCommissioning(kind: Kind, foundInAuto: boolean) {
  const loop: any = pidLoop();
  loop.controller.pidInitialization = pidInitialization();
  loop.signals.processVariable.range = { minimum: 0, maximum: 10 };
  // Gains are bounded by magnitude, ±1e6, and keep their sign for a reverse-acting loop (numerical annex N6.4).
  for (const parameter of loop.parameterBinding.parameters) { parameter.minimum = -1e6; parameter.maximum = 1e6; }
  if (kind === "proportional") loop.controller.law = { kind: "proportional", gainSymbol: "Main.Kp" };
  const binding: any = kind === "pid" ? pidTransferBinding(loop) : proportionalTransferBinding(loop);
  const paramSymbols: string[] = loop.parameterBinding.parameters.map((parameter: any) => parameter.symbol);
  const defaults: Record<string, number> = { "Main.SP": 2, "Main.PV": 2.004, "Main.MV": 0, "Main.ManualCommand": 0,
    "Main.Integral": 0, "Main.PreviousError": 0, "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 };
  const world: any = { values: {} as Record<string, number | boolean>, active: "original", previous: null, previousValues: null, applies: [] as boolean[] };
  for (const member of [...binding.states, ...binding.guards, binding.output, ...(binding.additional_outputs ?? [])]) {
    world.values[member.symbol] = defaults[member.symbol] ?? 0;
  }
  world.values[binding.mode.symbol] = foundInAuto ? !binding.mode.manual_value : binding.mode.manual_value;

  const status = () => ({ program_revision: "program", binding_revision: "transfer", values: { ...world.values }, runtime_time_ns: 1_000_000_000,
    status: "idle", reason: null, transaction: null, supported_tracking: ["parallel-pid-v1", "proportional"] });
  const paramStatus = () => ({ program_revision: "program", binding_revision: "parameters", active_revision: world.active,
    previous_revision: world.previous, status: "applied", transaction_id: null, reason: null, supports_request_id: true,
    values: pick(world.values, paramSymbols), previous_values: world.previousValues });
  const parameters: any = {
    status: async () => paramStatus(),
    apply: async (review: any) => {
      world.previous = world.active; world.previousValues = pick(world.values, paramSymbols); world.active = "applied-1";
      Object.assign(world.values, review.values);
      return paramStatus();
    },
    reviewRollback: async () => ({ transaction_id: "rollback-1", values: world.previousValues, observed: paramStatus() }),
    rollback: async (review: any) => {
      Object.assign(world.values, review.values); world.active = world.previous; world.previous = null; world.previousValues = null;
      return paramStatus();
    },
    cancelOwner: async () => "terminal",
  };

  const proto: any = RuntimeTransferClient.prototype;
  const savedTransfer = { status: proto.status, apply: proto.apply, cancel: proto.cancel };
  proto.status = async () => status();
  proto.apply = async (values: Record<string, number>, manual: boolean) => {
    Object.assign(world.values, values);
    world.values[binding.mode.symbol] = manual ? binding.mode.manual_value : !binding.mode.manual_value;
    world.applies.push(manual);
    return status();
  };
  proto.cancel = async () => undefined;

  // The recording answers with the response it is told to show: a departure from the permitted range, a stop with no samples, or a
  // recording that keeps running until the test lets it stop. Its buffer is released only by an explicit release() (runtimeRecording.ts).
  const recording: { mode: "out-of-range" | "stopped" | "running"; started: boolean; finish: boolean; polls: number; releases: number; stops: number;
    startArgs: unknown[] } = { mode: "out-of-range", started: false, finish: false, polls: 0, releases: 0, stops: 0, startArgs: [] };
  const sessions: any = RuntimeRecordingSession;
  const savedStart = sessions.start;
  sessions.start = async (...args: unknown[]) => {
    recording.started = true; recording.startArgs = args;
    const follow = async () => {
      if (recording.mode === "running") {
        if (recording.finish) return { state: "stopped", elapsedMs: 4000, sampleCount: 0, complete: false, reason: "stopped", rawSamples: [], nextSample: 0 };
        recording.polls++;
        return { state: "recording", elapsedMs: recording.polls * 100, sampleCount: 0, complete: false, reason: null, rawSamples: [], nextSample: 0 };
      }
      return recording.mode === "out-of-range"
        ? { state: "recording", elapsedMs: 0, sampleCount: 1, complete: false, reason: null, rawSamples: [{ pv: 25 }], nextSample: 1 }
        : { state: "stopped", elapsedMs: 4000, sampleCount: 0, complete: false, reason: "stopped", rawSamples: [], nextSample: 0 };
    };
    return { loop, follow,
      stop: async () => { recording.stops++; return { rawSamples: [], evidence: { completeness: "incomplete", reason: "fake recording" } }; },
      release: async () => { recording.releases++; } };
  };

  const connection: any = require("../../controlStudio/runtimeConnection");
  const savedBind = connection.bindRuntimeConnection;
  connection.bindRuntimeConnection = async () => ({ source: "simulation", endpoint: "fake", request: async () => ({}) });

  // A P law writes the complete record: its integral and derivative gains are explicit zero terms (Specification 35, C0 and C13).
  const candidate = kind === "pid" ? { "Main.Kp": 2, "Main.Ki": 0.5, "Main.Kd": 0 } : { "Main.Kp": 2, "Main.Ki": 0, "Main.Kd": 0 };
  const review: any = { observed: paramStatus(), values: candidate, transaction_id: "apply-1" };
  return {
    kind, loop, binding, model: singleOutputModel(), task: TASK, parameters, review, world, recording,
    restore: () => {
      proto.status = savedTransfer.status; proto.apply = savedTransfer.apply; proto.cancel = savedTransfer.cancel;
      sessions.start = savedStart; connection.bindRuntimeConnection = savedBind;
    },
  };
}

async function withFake(kind: Kind, foundInAuto: boolean, body: (fake: any) => Promise<void>): Promise<void> {
  const fake = fakeCommissioning(kind, foundInAuto);
  try { await body(fake); } finally { fake.restore(); }
}

async function reviewedCommissioning(fake: any): Promise<any> {
  return fake.kind === "pid"
    ? PidCommissioning.review(fake.loop, fake.model, fake.task, fake.parameters, fake.review)
    : ProportionalCommissioning.review(fake.loop, fake.model, fake.task, fake.parameters, fake.review);
}

const inAuto = (fake: any): boolean => fake.world.values[fake.binding.mode.symbol] !== fake.binding.mode.manual_value;
const lastWrite = (fake: any): boolean => fake.world.applies[fake.world.applies.length - 1];

/** Waits, in small steps, until a condition the fake runtime reaches later holds. */
async function until(condition: () => boolean): Promise<void> {
  for (let waited = 0; !condition(); waited += 5) {
    if (waited > 2000) throw new Error("the condition did not hold in time");
    await new Promise(resolve => setTimeout(resolve, 5));
  }
}

suite("Control Studio apply and validation in process time", () => {
  test("assesses a long response recorded at a record period, whose last sample covers the end of the test (A4.2, E4.4, B6.3)", () => {
    // A 0.5 s task recorded two executions per sample: the samples are 1 s apart, and the last one averages fewer executions.
    const units = { sp: "degC", pv: "degC", mv: "V" };
    const times = [...Array.from({ length: 20 }, (_, index) => index), 19.75];
    const rawSamples = times.map((elapsedSeconds, index) => ({ scan: index === 20 ? 41 : 2 * (index + 1), elapsedSeconds, controllerMode: "Auto",
      sp: 5, pv: 5, appliedMv: 2, quality: "good", units }));
    const criteria: any = { target: 5, durationSeconds: 20, settlingTimeSeconds: 5, maximumError: 0.1, taskPeriodMs: 500,
      processRange: { minimum: 0, maximum: 10 }, outputLimits: { minimum: -20, maximum: 20 },
      observed: { channels: [{ id: "PV", source: "processVariable", unit: "degC" }],
        requirements: [{ id: "bounds", kind: "bounds", channel: "PV", window: { start: 0, end: 20 }, minimum: 0, maximum: 10 }] } };
    const { assessCommissioningRecording } = require("../../controlStudio/commissioningAssessment");
    assert.strictEqual(assessCommissioningRecording({ rawSamples, samples: [], evidence: { completeness: "complete" } }, criteria), 0);
  });

  suite("independent model check (B5.2)", () => {
    const points = (residuals: number[]) => residuals.map((residual, index) => ({ measured: index / (residuals.length - 1), residual }));

    test("passes when the whole response and its final 10 % agree within the limit", () => {
      const score = scoreIndependentResponse(points(new Array(20).fill(0.05)), 0.15);
      assert.strictEqual(score.status, "Pass");
      assert.strictEqual(independentResponseReason(score),
        "Frozen predictions agree with independent plant measurements over the whole response and its final 10 %.");
    });

    test("fails on the whole response, and says what to change", () => {
      const score = scoreIndependentResponse(points(new Array(20).fill(0.3)), 0.15);
      assert.strictEqual(score.status, "Fail");
      assert.strictEqual(score.wholeWithinLimit, false);
      assert.match(independentResponseReason(score), /over the whole response.*a different method, a larger step, or a longer duration/);
    });

    test("fails on its final 10 % alone, although the whole response is within the limit", () => {
      const score = scoreIndependentResponse(points([...new Array(18).fill(0), 0.3, 0.3]), 0.15);
      assert.strictEqual(score.wholeWithinLimit, true, `the whole response passes: ${score.normalizedError}`);
      assert.strictEqual(score.finalWithinLimit, false, `the final 10 % fails: ${score.finalNormalizedError}`);
      assert.strictEqual(score.status, "Fail");
      assert.match(independentResponseReason(score), /not its final 10 %/);
    });

    test("the final 10 % always holds at least one point", () => {
      assert.deepStrictEqual([finalTenthStart(20), finalTenthStart(5), finalTenthStart(1)], [18, 4, 0]);
    });
  });

  suite("plant choice of isolated checks (B5.3, A9)", () => {
    test("the configured plant runs only when the target is simulated and it drives the loop's signals", () => {
      const loop: any = pidLoop();
      loop.plantId = "plant-1";
      const drives: ConfiguredPlantBinding = { id: "plant-1", commandSources: ["%QL0"], feedbackTargets: ["%IL0"] };
      assert.strictEqual(configuredPlantDrivesLoop(loop, drives), true);
      assert.deepStrictEqual(verificationPlant(loop, { simulated: true }, drives), CONFIGURED);
      assert.deepStrictEqual(verificationPlant(loop, { simulated: false }, drives), { kind: "model" }, "a physical target never uses the configured plant");
      assert.deepStrictEqual(verificationPlant(loop, { simulated: true }, { ...drives, feedbackTargets: ["%IL9"] }), { kind: "model" },
        "a plant whose feedback does not reach the measured output is not this loop's plant");
      assert.deepStrictEqual(verificationPlant(loop, { simulated: true }, { ...drives, commandSources: ["%QL9"] }), { kind: "model" },
        "a plant that does not take the loop's plant input is not this loop's plant");
      assert.deepStrictEqual(verificationPlant(loop, { simulated: true }, { ...drives, id: "other" }), { kind: "model" });
      assert.deepStrictEqual(verificationPlant(loop, { simulated: true }, undefined), { kind: "model" }, "without the saved bindings the model runs");
    });

    test("the loop's plant binding is read from the connected runtime's saved configuration", async () => {
      const answer = (plants: unknown[]) => async (operation: string) => {
        assert.strictEqual(operation, "plant.read");
        return { plants };
      };
      const binding = await readConfiguredPlantBinding(answer([{ id: "plant-1", command_sources: ["%QL0"], feedback_targets: ["%IL0"] }]), "plant-1");
      assert.deepStrictEqual(binding, { id: "plant-1", commandSources: ["%QL0"], feedbackTargets: ["%IL0"] });
      assert.strictEqual(await readConfiguredPlantBinding(answer([{ id: "other", command_sources: [], feedback_targets: [] }]), "plant-1"), undefined);
      assert.strictEqual(await readConfiguredPlantBinding(async () => { throw new Error("offline"); }, "plant-1"), undefined,
        "a runtime that cannot answer leaves the model in use");
      let asked = false;
      assert.strictEqual(await readConfiguredPlantBinding(async () => { asked = true; return {}; }, undefined), undefined);
      assert.strictEqual(asked, false, "a loop without a plant asks nothing");
    });

    test("a configured choice runs the request on the plant; a model choice keeps the request's own model", () => {
      const request = { step_ms: 100, duration_ms: 1000, simulation_override: { model_revision: "m1", model_provenance: "fitted", simulation_toml: "[simulation]" } };
      const configured = configuredPlantRequest({ cascade: undefined } as never, request as never, CONFIGURED) as { simulation_override?: unknown };
      assert.strictEqual(configured.simulation_override, undefined);
      assert.strictEqual(request.simulation_override.model_revision, "m1", "the request itself is not changed");
      assert.deepStrictEqual(configuredPlantRequest({} as never, request as never, { kind: "model" }), request);
    });

    test("every isolated run names the plant it ran against (A9)", () => {
      assert.strictEqual(isolatedPlantName(CONFIGURED, { revision: "m1" }), "configured plant plant-1");
      assert.strictEqual(isolatedPlantName({ kind: "model" }, { revision: "m1" }), "model m1");
    });
  });

  suite("operating-point wait in process time (B6.2)", () => {
    test("the bound is measured in process time from the runtime's timestamps, not from the wall clock", async () => {
      let reads = 0;
      const read = async () => { const at = reads++; return { values: { "Main.MV": 0, "Main.PV": 9 }, runtime_time_ns: at * 1e9 }; };
      await assert.rejects(() => awaitCommissioningStartingPoint(pidLoop(), singleOutputModel(), TASK, ["Main.MV"], ["Main.PV"], read,
        { progress: () => undefined, cancelled: () => false, pollMs: 0, wallClockMs: () => 0 }),
        (error: any) => error instanceof MainWorkflowError && /did not settle at 2 m ± 0\.02 m within 60 s of process time; it is 9 m\./.test(error.message)
          && error.failure.nextAction === "validate");
      assert.ok(reads >= 61, `60 s of process time at one second per reading takes at least 61 readings: ${reads}`);
    });

    test("the wait ends with 'The runtime stopped sending values.' when no newer runtime value arrives for 5 s", async () => {
      let wall = 0;
      const read = async () => ({ values: { "Main.MV": 0, "Main.PV": 9 }, runtime_time_ns: 0 });
      await assert.rejects(() => awaitCommissioningStartingPoint(pidLoop(), singleOutputModel(), TASK, ["Main.MV"], ["Main.PV"], read,
        { progress: () => undefined, cancelled: () => false, pollMs: 0, wallClockMs: () => (wall += 2000) }),
        (error: any) => error instanceof MainWorkflowError && error.message === "The runtime stopped sending values."
          && error.failure.nextAction === "connect-runtime");
    });

    test("progress shows each measured output with its validated and current value, and Abort ends the wait", async () => {
      let progressed = "";
      let reads = 0;
      const read = async () => ({ values: { "Main.MV": 0, "Main.PV": 2, "Main.Extra": 9 }, runtime_time_ns: reads++ * 1e8 });
      const wait = { progress: (text: string) => { progressed = text; }, cancelled: () => progressed !== "", pollMs: 0, wallClockMs: () => 0 };
      await assert.rejects(() => awaitCommissioningStartingPoint(pidLoop(), twoOutputModel(), TASK, ["Main.MV"], ["Main.PV", "Main.Extra"], read, wait),
        (error: unknown) => error instanceof StartingPointWaitCancelled);
      assert.strictEqual(progressed, "Main.PV settled at 2 m ± 0.02 m (now 2 m); Waiting for Main.Extra to settle at 5 ± 0.02 (now 9)…");
    });

    test("a plant input away from its validated point is refused at once, with nothing to do until it is back (B6.7)", async () => {
      let reads = 0;
      const read = async () => ({ values: { "Main.MV": 100, "Main.PV": 2 }, runtime_time_ns: reads++ * 1e8 });
      await assert.rejects(() => awaitCommissioningStartingPoint(pidLoop(), singleOutputModel(), TASK, ["Main.MV"], ["Main.PV"], read,
        { progress: () => undefined, cancelled: () => false, pollMs: 0, wallClockMs: () => 0 }),
        (error: any) => error instanceof MainWorkflowError && /Main\.MV is 100 V, but the settings were validated from 0 V/.test(error.message)
          && error.failure.nextAction === "none");
      assert.strictEqual(reads, 1, "nothing waits for an input that cannot settle by itself");
    });

    test("the synchronous check names an output that has not settled as a wait (A11.2)", () => {
      assert.throws(() => assertCommissioningStartingPoint(pidLoop(), singleOutputModel(), TASK, ["Main.MV"], ["Main.PV"], { "Main.MV": 0, "Main.PV": 9 }),
        (error: any) => error instanceof MainWorkflowError && error.failure.nextAction === "wait"
          && /Wait for the process to settle there/.test(error.message));
    });
  });

  suite("apply observation owned by the runtime (B6.3, A8.5)", () => {
    const ranges = observedRangesOf({ signals: { processVariable: { symbol: RANGE.symbol, unit: RANGE.unit, range: { minimum: 0, maximum: 10 } } } } as never);
    /** A second measured output of role PV, recorded as the auxiliary channel `pv-secondary`, with its own range of 1 to 3 bar (B1.4). */
    const secondary = { id: "pv-secondary", channel: "secondary", role: "PV", symbol: "P2.Pressure", unit: "bar", iecType: "LREAL",
      direction: "read", address: "%IL8", range: { minimum: 1, maximum: 3 } };
    const bothRanges = observedRangesOf({ signals: { processVariable: { symbol: RANGE.symbol, unit: RANGE.unit, range: { minimum: 0, maximum: 10 } },
      additional: [secondary] } } as never);
    const recordedSecondary = (value: number | null) => ({ id: "pv-secondary", channel: "secondary", role: "PV", unit: "bar", value, quality: "good" });

    test("the observation follows the recording to its end in process time, then returns it whole", async () => {
      const pages = [
        { state: "recording", elapsedMs: 1000, sampleCount: 1, complete: false, reason: null, rawSamples: [{ pv: 5 }], nextSample: 1 },
        { state: "stopped", elapsedMs: 4000, sampleCount: 1, complete: true, reason: null, rawSamples: [], nextSample: 1 },
      ];
      let follows = 0;
      const stopped = { rawSamples: [{ pv: 6 }], evidence: { completeness: "complete" } };
      const recording = { follow: async () => pages[Math.min(follows++, pages.length - 1)], stop: async () => stopped };
      const returned = await followAppliedResponse(recording as never, { durationMs: 4000, ranges, checkCancelled: () => undefined, pollMs: 1 });
      assert.strictEqual(returned, stopped as never);
      assert.strictEqual(follows, 2, "the recording is followed page by page until it stops");
    });

    test("the response is recorded for the reviewed duration from the adopted target, so the target change never shortens it (B6.3)", async () => {
      // The target is adopted 2 s into the recording; the reviewed duration is 4 s, so the recording stops once it reaches 6 s.
      const sample = (seconds: number) => ({ pv: 5, elapsedSeconds: seconds, controllerMode: "Auto", sp: seconds >= 2 ? 7 : 5 });
      const pages = [0, 1, 2, 3, 4, 5, 6, 7, 8].map(seconds => ({ state: "recording", elapsedMs: seconds * 1000, sampleCount: seconds + 1,
        complete: false, reason: null, rawSamples: [sample(seconds)], nextSample: seconds + 1 }));
      let follows = 0;
      let stops = 0;
      const recording = { follow: async () => pages[follows++], stop: async () => { stops += 1; return { rawSamples: [], evidence: {} }; } };
      await followAppliedResponse(recording as never, { durationMs: 4000, ranges, checkCancelled: () => undefined, pollMs: 0,
        responseStart: (value: any) => value.controllerMode === "Auto" && value.sp === 7 });
      assert.strictEqual(follows, 7, "following ends with the sample 4 s after the adopted target, not at 4 s of recording");
      assert.strictEqual(stops, 1);
    });

    test("a measured output that leaves its permitted range ends the observation at once, naming the signal (B6.3, B6.7)", async () => {
      let follows = 0;
      let stops = 0;
      const recording = {
        follow: async () => { follows++; return { state: "recording", elapsedMs: 100, sampleCount: 2, complete: false, reason: null, rawSamples: [{ pv: 12.5 }], nextSample: 1 }; },
        stop: async () => { stops++; return { rawSamples: [] }; },
      };
      await assert.rejects(() => followAppliedResponse(recording as never, { durationMs: 4000, ranges, checkCancelled: () => undefined, pollMs: 1 }),
        (error: any) => error instanceof PermittedRangeLeft && error.message === "P1.RoomC left its permitted range (12.5 °C outside 0 °C..10 °C)"
          && error.failure.nextAction === "none");
      assert.deepStrictEqual([follows, stops], [1, 0], "no further page is read after the departure");
    });

    test("a secondary measured output that leaves its own range ends the observation at once, naming that output (B6.3, B6.7)", async () => {
      let follows = 0;
      let stops = 0;
      const recording = {
        follow: async () => { follows++; return { state: "recording", elapsedMs: 100, sampleCount: 2, complete: false, reason: null,
          rawSamples: [{ pv: 5, additional: [recordedSecondary(3.5)] }], nextSample: 1 }; },
        stop: async () => { stops++; return { rawSamples: [] }; },
      };
      await assert.rejects(() => followAppliedResponse(recording as never, { durationMs: 4000, ranges: bothRanges, checkCancelled: () => undefined, pollMs: 1 }),
        (error: any) => error instanceof PermittedRangeLeft && error.message === "P2.Pressure left its permitted range (3.5 bar outside 1 bar..3 bar)"
          && error.failure.nextAction === "none");
      assert.deepStrictEqual([follows, stops], [1, 0], "the process variable is inside its range, so the departure is the secondary output's alone");
    });

    test("a secondary measured output that stays inside its own range does not end the observation (B6.3)", async () => {
      const sample = { pv: 5, additional: [recordedSecondary(2)] };
      const pages = [
        { state: "recording", elapsedMs: 1000, sampleCount: 1, complete: false, reason: null, rawSamples: [sample], nextSample: 1 },
        { state: "stopped", elapsedMs: 4000, sampleCount: 1, complete: true, reason: null, rawSamples: [], nextSample: 1 },
      ];
      let follows = 0;
      const stopped = { rawSamples: [sample], evidence: { completeness: "complete" } };
      const recording = { follow: async () => pages[Math.min(follows++, pages.length - 1)], stop: async () => stopped };
      const returned = await followAppliedResponse(recording as never, { durationMs: 4000, ranges: bothRanges, checkCancelled: () => undefined, pollMs: 1 });
      assert.strictEqual(returned, stopped as never);
      assert.strictEqual(follows, 2, "the recording is followed to its end, as a response inside every range is");
    });

    test("a secondary measured output that leaves its range after the last page is named when the recording stops (B6.3)", async () => {
      let stops = 0;
      const recording = {
        follow: async () => ({ state: "stopped", elapsedMs: 4000, sampleCount: 1, complete: true, reason: null, rawSamples: [], nextSample: 1 }),
        stop: async () => { stops++; return { rawSamples: [{ pv: 5, additional: [recordedSecondary(0.5)] }] }; },
      };
      await assert.rejects(() => followAppliedResponse(recording as never, { durationMs: 4000, ranges: bothRanges, checkCancelled: () => undefined, pollMs: 1 }),
        (error: any) => error instanceof PermittedRangeLeft && error.message === "P2.Pressure left its permitted range (0.5 bar outside 1 bar..3 bar)");
      assert.strictEqual(stops, 1, "the stopped recording is checked before it is returned");
    });

    test("the observation stops with the connection-loss message when no newer value arrives for 5 s", async () => {
      let wall = 0;
      const recording = {
        follow: async () => ({ state: "recording", elapsedMs: 0, sampleCount: 0, complete: false, reason: null, rawSamples: [], nextSample: 0 }),
        stop: async () => ({ rawSamples: [] }),
      };
      await assert.rejects(() => followAppliedResponse(recording as never, { durationMs: 4000, ranges, checkCancelled: () => undefined, pollMs: 0,
        wallClockMs: () => (wall += 2000) }),
        (error: any) => error instanceof MainWorkflowError && error.message === "The runtime stopped sending values."
          && error.failure.nextAction === "connect-runtime");
    });

    test("a recording the runtime ended reports the runtime's reason and asks for the runtime to be connected again (A6.7)", async () => {
      const recording = {
        follow: async () => ({ state: "failed", elapsedMs: 300, sampleCount: 3, complete: false, reason: "scan_reset", rawSamples: [], nextSample: 0 }),
        stop: async () => ({ rawSamples: [] }),
      };
      await assert.rejects(() => followAppliedResponse(recording as never, { durationMs: 4000, ranges, checkCancelled: () => undefined, pollMs: 0 }),
        (error: any) => error instanceof MainWorkflowError && /The runtime ended the recording: scan_reset\./.test(error.message)
          && error.failure.nextAction === "connect-runtime");
    });

    test("an observed response that fails a requirement is a failed requirement that offers Restore previous settings (B6.3)", () => {
      const incomplete: any = { rawSamples: [], evidence: { completeness: "incomplete" } };
      assert.throws(() => assessObservation(incomplete, { target: 3, durationSeconds: 4, settlingTimeSeconds: 2, maximumError: 0.1, taskPeriodMs: 100,
        processRange: { minimum: 0, maximum: 10 }, outputLimits: { minimum: -20, maximum: 20 } } as never),
        (error: any) => error instanceof ObservationRequirementFailed && error.failure.cause === "settings" && error.failure.nextAction === "restore");
    });

    test("the observation's identity is journalled with every checkpoint written after it begins (B6.3)", async () => {
      const writes: any[] = [];
      const journal = new ObservationRecord(async checkpoint => { writes.push(checkpoint); });
      const write = journal.checkpointWriter();
      await write({ id: "before" } as never);
      journal.begin(5_000_000_000, 4000);
      await write({ id: "during" } as never);
      assert.strictEqual(writes[0].observation, undefined);
      assert.deepStrictEqual(writes[1].observation, { startedAtProcessNs: 5_000_000_000, durationMs: 4000 });
    });

    test("reopening resumes a journalled observation by following the runtime's recording and assessing it (A8.5)", async () => {
      await assert.rejects(() => resumeApplyObservation(pidLoop() as never, { id: "x" } as never, {} as never, { retain: async () => undefined }),
        /holds no applied response to resume/);
      const opened: unknown[] = [];
      const checkpoint = { recording: { recordingId: "r1" }, observation: { startedAtProcessNs: 1, durationMs: 4000 } } as never;
      const follower = {
        follow: async () => ({ state: "stopped", elapsedMs: 4000, sampleCount: 0, complete: false, reason: "stopped", rawSamples: [], nextSample: 0 }),
        stop: async () => ({ rawSamples: [], evidence: { completeness: "incomplete" } }),
        release: async () => undefined,
      };
      await assert.rejects(() => resumeApplyObservation(pidLoop() as never, checkpoint, {} as never, {
        open: async owner => { opened.push(owner); return follower as never; },
        retain: async () => undefined,
      }), (error: unknown) => error instanceof ObservationRequirementFailed);
      assert.deepStrictEqual(opened, [{ recordingId: "r1" }], "the journalled recording is reopened, not started again");
    });
  });

  suite("retained recordings and closing the panel (A8.5, A10.4, runtimeRecording.ts)", () => {
    test("settling an observing campaign never aborts it: it completes, and Restore stays available (B6.3, A10.4)", async () => {
      await withFake("pid", true, async fake => {
        fake.recording.mode = "running";
        const commission = await reviewedCommissioning(fake);
        const running = commission.run();
        await until(() => fake.recording.polls > 0);
        const closing = commission.settleForClose();
        fake.recording.finish = true;
        const result = await running;
        assert.deepStrictEqual(result.failure, { cause: "settings", nextAction: "restore" }, "the observation completed: nothing was aborted or restored");
        assert.strictEqual(fake.world.values["Main.Kp"], 2, "the reviewed settings stay applied for the engineer's decision");
        assert.strictEqual((await closing)?.status, "failed", "settling waits for the campaign's own outcome");
      });
    });

    test("the runtime's buffer is released only after the samples are stored (runtimeRecording.ts)", async () => {
      await withFake("pid", true, async fake => {
        const commission = await reviewedCommissioning(fake);
        await commission.run();
        assert.strictEqual(fake.recording.releases, 0, "nothing is released before the samples are stored");
        await commission.releaseRecording();
        assert.strictEqual(fake.recording.releases, 1, "the buffer is released once, after storing");
      });
    });

    test("reopening stores the resumed recording before releasing the runtime's buffer, and replays no write (A8.5)", async () => {
      const order: string[] = [];
      const checkpoint = { recording: { recordingId: "r2" }, observation: { startedAtProcessNs: 1, durationMs: 4000 } } as never;
      const follower = {
        follow: async () => ({ state: "stopped", elapsedMs: 4000, sampleCount: 0, complete: false, reason: "stopped", rawSamples: [], nextSample: 0 }),
        stop: async () => ({ rawSamples: [], evidence: { completeness: "complete" } }),
        release: async () => { order.push("release"); },
      };
      await assert.rejects(() => resumeApplyObservation(pidLoop() as never, checkpoint, {} as never, {
        open: async () => follower as never,
        retain: async () => { order.push("retain"); },
      }), (error: unknown) => error instanceof ObservationRequirementFailed);
      assert.deepStrictEqual(order, ["retain", "release"], "the samples are stored before the buffer is released");
    });
  });

  suite("controller transfer and Restore (A6.3, B6.4)", () => {
    test("a transfer the runtime reverted in the same scan keeps the loop as it was, with the runtime's reason and no Restore (A6.3, B6.7)", () => {
      const reverted = transferRejection({ status: "failed", reason: "first controller output Main.MV violates reviewed transfer tolerance",
        transaction: { restored: true } } as never);
      assert.ok(reverted instanceof MainWorkflowError);
      assert.deepStrictEqual(classifyMainWorkflowFailure(reverted), { cause: "settings", nextAction: "validate" });
      assert.match(reverted.message, /first controller output Main\.MV violates reviewed transfer tolerance\. The runtime reverted the transfer in the same scan, so the loop kept its mode and values\./);
    });

    test("a transfer that failed without restoration keeps today's plain recovery path (A6.3)", () => {
      const failed = transferRejection({ status: "rejected", reason: "stale program, binding, or expected values", transaction: { restored: false } } as never);
      assert.ok(!(failed instanceof MainWorkflowError));
      assert.strictEqual(failed.message, "stale program, binding, or expected values");
    });

    test("Restore is unavailable once the settings Apply wrote are no longer the active ones (B6.4)", () => {
      const applied: any = { program_revision: "program", binding_revision: "parameters", active_revision: "applied-1", previous_revision: "original",
        status: "applied", transaction_id: null, reason: null, supports_request_id: true, values: { "Main.Kp": 2 }, previous_values: { "Main.Kp": 1 } };
      assert.strictEqual(restoreUnavailableReason(applied, applied), undefined);
      assert.strictEqual(restoreUnavailableReason(applied, undefined), undefined, "without an Apply there is nothing to compare");
      assert.match(restoreUnavailableReason({ ...applied, active_revision: "someone-else" }, applied) ?? "",
        /Restore is unavailable: the controller settings or program changed after Apply/);
    });
  });

  test("a loop found in Auto starts from wherever its controller output is; in Manual the output must be the validated one (B6.2, A6.4)", async () => {
    for (const kind of ["pid", "proportional"] as const) {
      await withFake(kind, true, async fake => {
        fake.world.values["Main.MV"] = 0.36;
        const result = await (await reviewedCommissioning(fake)).run();
        assert.ok(fake.world.applies.includes(false), `${kind}: the Auto entry is written from the running output`);
        assert.doesNotMatch(result.reason ?? "", /validated from/, `${kind}: the running controller's output is not compared`);
      });
      await withFake(kind, false, async fake => {
        fake.world.values["Main.MV"] = 0.36;
        await assert.rejects(() => reviewedCommissioning(fake), /Main\.MV is 0\.36 V, but the settings were validated from 0 V/,
          `${kind}: a Manual output is a setting and must already be the validated one`);
      });
    }
  });

  test("in Manual a plant input within the Auto-entry allowance for the measured noise counts as the validated one (B6.2, A6.4)", async () => {
    const noise = 0.05;
    for (const kind of ["pid", "proportional"] as const) {
      // The candidate (Kp 2, PID Ki 0.5) allows Kp·2n + Ki·Δt·(|e| + n), about 0.2 V, around the validated 0 V.
      for (const [output, accepted] of [[0.15, true], [0.36, false]] as const) {
        await withFake(kind, false, async fake => {
          fake.world.values["Main.MV"] = output;
          const review = () => kind === "pid"
            ? PidCommissioning.review(fake.loop, fake.model, fake.task, fake.parameters, fake.review, undefined, undefined, noise)
            : ProportionalCommissioning.review(fake.loop, fake.model, fake.task, fake.parameters, fake.review, undefined, noise);
          if (accepted) await review();
          else await assert.rejects(review, /Main\.MV is 0\.36 V, but the settings were validated from 0 V ± 0\.2/, `${kind}: beyond the allowance is refused`);
        });
      }
    }
  });

  test("enters Auto, and returns to Auto after a failed Apply, within the A6.4 allowance for the measured noise", async () => {
    const noise = 0.05;
    for (const kind of ["pid", "proportional"] as const) {
      await withFake(kind, true, async fake => {
        const proto: any = RuntimeTransferClient.prototype;
        const fakeApply = proto.apply;
        const autoEntries: number[] = [];
        // Only a transfer that moves the loop from Manual to Auto is an Auto entry; the target change in Auto is not.
        proto.apply = async function (values: unknown, manual: boolean, tolerance: number, ...rest: unknown[]) {
          if (!manual && !inAuto(fake)) autoEntries.push(tolerance);
          return fakeApply.call(this, values, manual, tolerance, ...rest);
        };
        const commission = kind === "pid"
          ? await PidCommissioning.review(fake.loop, fake.model, fake.task, fake.parameters, fake.review, undefined, undefined, noise)
          : await ProportionalCommissioning.review(fake.loop, fake.model, fake.task, fake.parameters, fake.review, undefined, noise);
        const result = await commission.run();
        assert.strictEqual(result.status, "failed", "the fake recording leaves the permitted range, so Apply returns the loop to Auto");
        const cycle = fake.loop.controller.taskPeriodMs / 1000;
        // Kp·2n + Ki·Δt·(|e| + n): the candidate (Kp 2, PID Ki 0.5) enters with e = 2 − 2.004; the previous gain (Kp 1) returns.
        const expected = kind === "pid" ? [2 * 2 * noise + 0.5 * cycle * (0.004 + noise), 2 * 1 * noise] : [2 * 2 * noise, 2 * 1 * noise];
        assert.strictEqual(autoEntries.length, 2, `${kind}: Apply enters Auto, then the recovery returns to Auto`);
        autoEntries.forEach((tolerance, index) => assert.ok(Math.abs(tolerance - expected[index]) < 1e-12,
          `${kind}: Auto entry ${index + 1} allows ${tolerance}, A6.4 gives ${expected[index]}`));
      });
    }
  });

  test("measures the noise single executions see over a closed-loop baseline recorded at a record period (A6.4, A4.2)", () => {
    const { measuredProcessNoise } = require("../../controlStudio/processNoiseEvidence");
    const { recordingFingerprint } = require("../../controlStudio/independentModelValidation");
    const loop = pidLoop();
    const pv = loop.signals.processVariable.symbol, mv = loop.signals.manipulatedVariable.symbol;
    // Samples one second apart average ten executions of a 100 ms task. In Auto the input moves during the baseline.
    loop.controller.taskPeriodMs = 100;
    const samples = Array.from({ length: 40 }, (_, index) => ({ timeSeconds: index, sequence: 10 * (index + 1), values: {
      [pv]: { value: 45 + (index % 2 ? 0.01 : -0.01), quality: "good" }, [mv]: { value: 38 + index / 100, quality: "good" } } }));
    const run = { id: "baseline", evidence: "Simulation", channels: [{ id: pv, unit: "degC" }, { id: mv, unit: "%" }], samples,
      conditions: { durationSeconds: 39, sampleTimesSeconds: samples.map(sample => sample.timeSeconds) } };
    const record = { projectRevision: "program", request: { baselineStartSeconds: 0, baselineEndSeconds: 29 }, run };
    const model = { evidence: [recordingFingerprint(run)] };
    // Thirty samples alternating ±0.01 about 45: a standard deviation of 0.01·√(30/29), times three, times √10.
    const expected = 3 * 0.01 * Math.sqrt(30 / 29) * Math.sqrt(10);
    assert.ok(Math.abs(measuredProcessNoise(loop, model, [{ runs: [record] }]) - expected) < 1e-12);
  });

  suite("mode returned to by a failed or restored operation (A6.2)", () => {
    test("a failed PID Apply that found the loop in Auto returns it to Auto, with the previous gain restored", async () => {
      await withFake("pid", true, async fake => {
        const result = await (await reviewedCommissioning(fake)).run();
        assert.strictEqual(result.status, "failed");
        assert.strictEqual(result.reason, "Main.PV left its permitted range (25 m outside 0 m..10 m); the previous settings were restored.");
        assert.deepStrictEqual(result.failure, { cause: "model-or-data", nextAction: "none" });
        assert.strictEqual(result.recoveryConfirmed, true);
        assert.strictEqual(inAuto(fake), true, "the loop returns to the mode Apply found it in");
        assert.strictEqual(fake.world.values["Main.Kp"], 1, "the previous gain is back");
        assert.strictEqual(lastWrite(fake), false, "the last write returns the loop to Auto");
      });
    });

    test("a failed PID Apply that found the loop in Manual leaves it in Manual", async () => {
      await withFake("pid", false, async fake => {
        const result = await (await reviewedCommissioning(fake)).run();
        assert.strictEqual(result.status, "failed");
        assert.strictEqual(inAuto(fake), false, "the loop keeps the mode Apply found it in");
        assert.strictEqual(fake.world.values["Main.Kp"], 1);
        assert.strictEqual(lastWrite(fake), true, "the last write keeps the loop in Manual");
      });
    });

    test("a failed proportional Apply that found the loop in Auto returns it to Auto, with the previous gain restored", async () => {
      await withFake("proportional", true, async fake => {
        const result = await (await reviewedCommissioning(fake)).run();
        assert.strictEqual(result.status, "failed");
        assert.deepStrictEqual(result.failure, { cause: "model-or-data", nextAction: "none" });
        assert.strictEqual(result.recoveryConfirmed, true);
        assert.strictEqual(inAuto(fake), true, "the loop returns to the mode Apply found it in");
        assert.strictEqual(fake.world.values["Main.Kp"], 1, "the previous gain is back");
        assert.strictEqual(lastWrite(fake), false);
      });
    });

    test("a failed proportional Apply that found the loop in Manual leaves it in Manual", async () => {
      await withFake("proportional", false, async fake => {
        const result = await (await reviewedCommissioning(fake)).run();
        assert.strictEqual(result.status, "failed");
        assert.strictEqual(inAuto(fake), false);
        assert.strictEqual(fake.world.values["Main.Kp"], 1);
        assert.strictEqual(lastWrite(fake), true);
      });
    });

    test("a failed requirement keeps the reviewed settings applied, and Restore returns the loop to Auto with the previous settings (B6.3, B6.4)", async () => {
      await withFake("pid", true, async fake => {
        fake.recording.mode = "stopped";
        const commission = await reviewedCommissioning(fake);
        const result = await commission.run();
        assert.strictEqual(result.status, "failed");
        assert.deepStrictEqual(result.failure, { cause: "settings", nextAction: "restore" });
        assert.strictEqual(inAuto(fake), true);
        assert.strictEqual(fake.world.values["Main.Kp"], 2, "the reviewed gain stays applied until the engineer decides");
        await commission.rollback();
        assert.strictEqual(inAuto(fake), true, "Restore returns the mode from before Apply");
        assert.strictEqual(fake.world.values["Main.Kp"], 1, "Restore brings back the previous gain");
        const recorded = commission.recordedOutcome();
        assert.strictEqual(recorded.recoveryConfirmed, true);
        assert.strictEqual(recorded.failure, undefined);
      });
    });

    test("a Restore reviewed later from Apply's report returns a loop found in Auto to Auto (A6.2, B6.4)", async () => {
      await withFake("pid", true, async fake => {
        fake.recording.mode = "stopped";
        // Each confirmed transfer carries its transaction record, as the runtime's does (E6.8), so the report can be reviewed.
        const proto: any = RuntimeTransferClient.prototype;
        const fakeApply = proto.apply;
        proto.apply = async function (values: Record<string, number>, ...rest: unknown[]) {
          const before = { ...fake.world.values };
          const status = await fakeApply.call(this, values, ...rest);
          return { ...status, transaction: { initial_values: before, applied_values: values, observed_values: { ...fake.world.values },
            confirmation: "output-image" } };
        };
        const result = await (await reviewedCommissioning(fake)).run();
        assert.strictEqual(result.status, "failed", "a failed requirement keeps the settings applied until the engineer decides");
        assert.strictEqual(result.foundMode, "auto", "Apply's report names the mode it found the loop in");
        const restore = await PidCommissioning.reviewRecovery(fake.loop, fake.model, fake.task, fake.parameters,
          await fake.parameters.reviewRollback(), result);
        assert.match(restore.rollbackDescription(), /and return to Auto/);
        await restore.rollback();
        assert.strictEqual(inAuto(fake), true, "Restore returns the mode from before Apply");
        assert.strictEqual(fake.world.values["Main.Kp"], 1, "Restore brings back the previous gain");
      });
    });

    test("records the applied response at a record period that follows the requirements' settling time (A4.2, B6.3)", async () => {
      for (const kind of ["pid", "proportional"] as const) {
        await withFake(kind, true, async fake => {
          await (await reviewedCommissioning(fake)).run();
          assert.strictEqual(fake.recording.startArgs[3], TASK.settlingTimeSeconds, `${kind}: the observation names the settling time`);
        });
      }
    });

    test("closing during the observation detaches it at once: the runtime keeps recording and nothing is restored (A8.5, A10.4)", async () => {
      for (const kind of ["pid", "proportional"] as const) {
        await withFake(kind, true, async fake => {
          fake.recording.mode = "running";
          const commission = await reviewedCommissioning(fake);
          const run = commission.run();
          await until(() => fake.recording.polls > 0);
          assert.strictEqual(commission.detachForClose(), true, `${kind}: an observing campaign detaches`);
          await assert.rejects(run, (error: unknown) => error instanceof ObservationDetached);
          assert.strictEqual(fake.recording.stops, 0, `${kind}: the runtime keeps the recording`);
          assert.strictEqual(inAuto(fake), true, `${kind}: the reviewed settings stay applied while the runtime records`);
          assert.strictEqual(fake.world.values["Main.Kp"], 2);
        });
      }
    });

    test("a Restore that fails leaves the recovery unresolved, so the session keeps its interlock (A6.3)", async () => {
      await withFake("pid", true, async fake => {
        fake.recording.mode = "stopped";
        const commission = await reviewedCommissioning(fake);
        const result = await commission.run();
        assert.deepStrictEqual(result.failure, { cause: "settings", nextAction: "restore" });
        // The connection drops while Restore reads the parameter state.
        fake.parameters.status = async () => { throw new Error("communication lost"); };
        await assert.rejects(() => commission.rollback(), /communication lost/);
        assert.strictEqual(commission.recoveryConfirmed, false, "the commissioning no longer claims a confirmed recovery");
        assert.strictEqual(commission.recordedOutcome().recoveryConfirmed, false, "the retained outcome says the recovery is unresolved");
        assert.match(String(commission.recordedOutcome().reason), /Recovery unconfirmed: .*communication lost/);
      });
    });

    test("a decimal target on a REAL setpoint is sent and assessed as the runtime holds it", async () => {
      await withFake("pid", true, async fake => {
        // A REAL setpoint holds 0.1 as 0.10000000149011612; the runtime refuses any other value (runtime protocol annex, E6).
        fake.loop.signals.setpoint.iecType = "REAL";
        fake.task = { ...fake.task, referenceChange: 0.1 - (fake.model.operatingPoint?.outputs[0] ?? 0) };
        const commission = await reviewedCommissioning(fake);
        assert.strictEqual((commission as any).target(), Math.fround(0.1));
      });
    });

    test("a failed requirement on a proportional loop found in Auto is restored to Auto as well (B6.4)", async () => {
      await withFake("proportional", true, async fake => {
        fake.recording.mode = "stopped";
        const commission = await reviewedCommissioning(fake);
        const result = await commission.run();
        assert.deepStrictEqual(result.failure, { cause: "settings", nextAction: "restore" });
        await commission.rollback();
        assert.strictEqual(inAuto(fake), true);
        assert.strictEqual(fake.world.values["Main.Kp"], 1);
        assert.strictEqual(commission.recordedOutcome().failure, undefined);
      });
    });
  });
});
