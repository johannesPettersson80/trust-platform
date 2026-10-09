import * as assert from "assert";
import { blockAveraged, evenlySampled, MAXIMUM_FIT_SAMPLES } from "../../controlStudio/evenSampling";
import { identifyPlantFromExcitation } from "../../controlStudio/excitationIdentification";
import type { IdentificationSample } from "../../controlStudio/identification";
import { retainedPeriodicRecordingSource } from "../../controlStudio/periodicRecordingSource";
import { evaluateRecordedRun } from "../../controlStudio/runValidation";
import {
  analyzeStepResponse,
  measureStepResponse,
  qualifyStepResponse,
  type StepResponseSettings,
} from "../../controlStudio/stepResponseIdentification";

/** Execution times of a 100 ms task in a 10 ms resource: the intervals alternate between 110 and 100 ms. */
function taskTimes(count: number): number[] {
  const times = [0];
  for (let index = 1; index < count; index += 1) times.push(times[index - 1] + (index % 2 === 1 ? 0.11 : 0.1));
  return times;
}

const BASELINE = 35;
const INPUT_LOW = 75;
const INPUT_HIGH = 85;
const GAIN = 0.2;
const TIME_CONSTANT = 1;

/** A first-order response to a 10-unit input step, held from `stepIndex` up to `releaseIndex`, sampled at `times`. */
function stepRecord(times: readonly number[], stepIndex: number, releaseIndex: number): IdentificationSample[] {
  return times.map((timeSeconds, index) => {
    const rise = 1 - Math.exp(-Math.max(0, timeSeconds - times[stepIndex]) / TIME_CONSTANT);
    const fall = 1 - Math.exp(-Math.max(0, timeSeconds - times[releaseIndex]) / TIME_CONSTANT);
    return { timeSeconds, input: index >= stepIndex && index < releaseIndex ? INPUT_HIGH : INPUT_LOW,
      output: BASELINE + GAIN * (INPUT_HIGH - INPUT_LOW) * (rise - fall), quality: "good" as const };
  });
}

const settings: StepResponseSettings = {
  baselineDurationSeconds: 1, finalDurationSeconds: 1, maximumWindowRange: 0.1, maximumWindowSlopePerSecond: 0.01,
  measurementBound: 0.05, onsetThreshold: 0.15, onsetSamples: 3,
  delay: { minimum: 0, maximum: 0, step: 0.1 }, timeConstant: { minimum: 0.5, maximum: 1.5, step: 0.001 },
  baselineStep: 0.01, gainStep: 0.001, timestampShiftSeconds: 0,
};

suite("Control Studio jittered sampling", () => {
  test("identifies a step recorded at alternating 100/110 ms as the same model as an even 105 ms grid", () => {
    // Both recordings place the step at 1.05 s and its release at 21.0 s, so only the sample spacing differs.
    const jittered = analyzeStepResponse(stepRecord(taskTimes(260), 10, 200), settings);
    const even = analyzeStepResponse(stepRecord(Array.from({ length: 260 }, (_, index) => index * 0.105), 10, 200), settings);
    assert.ok(Math.abs(jittered.best.gain / even.best.gain - 1) <= 0.02,
      `gain ${jittered.best.gain} against ${even.best.gain}`);
    assert.ok(Math.abs(jittered.best.timeConstantSeconds / even.best.timeConstantSeconds - 1) <= 0.02,
      `time constant ${jittered.best.timeConstantSeconds} against ${even.best.timeConstantSeconds}`);
    assert.ok(Math.abs(even.best.timeConstantSeconds - TIME_CONSTANT) <= 0.02 * TIME_CONSTANT,
      `the even grid recovers the time constant: ${even.best.timeConstantSeconds}`);
  });

  test("refuses a 0.42 s gap and names its start time", () => {
    // The 0.32 s shift after index 123 makes the interval from 12.3 s to 12.72 s 0.42 s long.
    const times = Array.from({ length: 140 }, (_, index) => Number((index * 0.1 + (index >= 124 ? 0.32 : 0)).toFixed(10)));
    const gapped = stepRecord(times, 10, 130);
    const qualification = qualifyStepResponse(gapped, settings);
    assert.strictEqual(qualification.suitable, false);
    assert.match(qualification.suitable ? "" : qualification.reason, /The recording has a gap of 0\.42 s at 12\.3 s\./);
    assert.throws(() => measureStepResponse(gapped, settings), /The recording has a gap of 0\.42 s at 12\.3 s\./);
  });

  test("accepts a 20,000-sample recording and fits it on at most 5,000 block-averaged points", function () {
    this.timeout(20_000);
    const times = Array.from({ length: 20_000 }, (_, index) => index * 0.01);
    const result = analyzeStepResponse(stepRecord(times, 100, 15_000),
      { ...settings, timeConstant: { minimum: 0.5, maximum: 1.5, step: 0.01 } });
    assert.ok(Number.isFinite(result.best.squaredError), "the fit completes");
    assert.ok(result.points.length <= MAXIMUM_FIT_SAMPLES, `the fit sees ${result.points.length} points`);
    assert.ok(Math.abs(result.best.gain / GAIN - 1) <= 0.01, `gain ${result.best.gain}`);
    assert.ok(Math.abs(result.best.timeConstantSeconds - TIME_CONSTANT) <= 0.02 * TIME_CONSTANT,
      `time constant ${result.best.timeConstantSeconds}`);
  });

  test("fits a first-order plant from jittered PRBS excitation", () => {
    // A 7-stage maximal-length shift register gives the binary excitation. Each bit is held for 20 scans (about 2 s),
    // because a bit that changes inside one grid interval is only partly seen by that interval's sample.
    const GAIN_TRUE = 2;
    const times = taskTimes(400);
    let register = 0x7f;
    const bits = Array.from({ length: 20 }, () => {
      const bit = register & 1;
      register = (register >> 1) | ((((register >> 0) ^ (register >> 1)) & 1) << 6);
      return bit ? 0.5 : -0.5;
    });
    const input = times.map((_, index) => (index < 10 ? 0 : bits[Math.floor((index - 10) / 20)]));
    // The plant is simulated exactly, with each input held over the real interval to the next scan.
    let output = 0;
    const samples: IdentificationSample[] = times.map((timeSeconds, index) => {
      const sample = { timeSeconds, input: input[index], output, quality: "good" as const };
      const decay = index + 1 < times.length ? Math.exp(-(times[index + 1] - timeSeconds) / TIME_CONSTANT) : 0;
      output = decay * output + (1 - decay) * GAIN_TRUE * input[index];
      return sample;
    });
    const fit = identifyPlantFromExcitation(samples, 0.5);
    const firstOrder = fit.models.find(model => model.kind === "first_order");
    assert.ok(firstOrder, "the first-order candidate is fitted");
    assert.ok(Math.abs(firstOrder.gain / GAIN_TRUE - 1) <= 0.1, `gain ${firstOrder.gain}`);
  });

  test("accepts a periodic recording whose cycles jitter between 100 and 110 ms, and refuses a gap", () => {
    const recording = (cycles: (number | null)[]) => ({
      rawSamples: cycles.map((actualCycleSeconds, index) => ({ quality: "good", appliedMv: 1 + index / 100, pv: 2 + index / 50,
        actualCycleSeconds, scan: index + 1, loopRevision: "loop", outputLimits: { minimum: 0, maximum: 10 } })),
      evidence: { recordingId: "recording", completeness: "complete", missingScanRanges: [], droppedObservedSamples: 0,
        actualCycleTimingAvailable: true },
    }) as any;
    // The first scan has no predecessor, so its cycle time is unknown.
    const jitter = Array.from({ length: 40 }, (_, index) => (index === 0 ? null : index % 2 === 1 ? 0.11 : 0.1));
    const accepted = retainedPeriodicRecordingSource(recording(jitter), "recording", "loop");
    assert.ok(accepted, "the jittered recording is accepted");
    assert.ok(Math.abs(accepted.sampleTimeSeconds - 0.105) <= 0.01, `sample time ${accepted.sampleTimeSeconds}`);
    assert.strictEqual(accepted.input.length, accepted.output.length);
    assert.ok(accepted.input.length >= 35 && accepted.input.length <= 42, `${accepted.input.length} grid points`);
    const gapped = jitter.map((cycle, index) => (index === 20 ? 0.25 : cycle));
    assert.strictEqual(retainedPeriodicRecordingSource(recording(gapped), "recording", "loop"), undefined,
      "a 0.25 s cycle is a gap and is refused");
  });

  test("holds declared sample values through jittered scans, and refuses a gap with its time", () => {
    const grid = Array.from({ length: 26 }, (_, index) => Number((index / 10).toFixed(10)));
    const scan = (timeSeconds: number, sequence: number) => ({ timeSeconds, sequence,
      values: { response: { value: 1, quality: "good" as const } } });
    const run = (samples: unknown[]) => ({ id: "run-a", evidence: "physical-recorded", parameters: { gain: 2 },
      conditions: { initialState: {}, events: [], durationSeconds: 2.5, sampleTimesSeconds: grid, seed: 1,
        modelRevision: "plant", programRevision: "st", parameterSchemaRevision: "params" },
      channels: [{ id: "response", unit: "m" }], samples });
    const bounds = { id: "bounds", kind: "bounds", channel: "response", window: { start: 0.2, end: 1 }, minimum: 0.5, maximum: 1.5 };
    const jittered = taskTimes(12).map((time, index) => scan(time, 10 + index));
    assert.strictEqual(evaluateRecordedRun(run(jittered), [bounds]).status, "Pass",
      "jittered scans cover every declared sample");
    const early = Array.from({ length: 13 }, (_, index) => scan(Number((index * 0.1).toFixed(10)), 10 + index));
    const late = Array.from({ length: 9 }, (_, index) => scan(Number((1.62 + index * 0.1).toFixed(10)), 23 + index));
    const gapped = evaluateRecordedRun(run([...early, ...late]), [{ ...bounds, window: { start: 0.2, end: 2 } }]);
    assert.strictEqual(gapped.status, "Inconclusive");
    assert.match(gapped.requirements[0].reason, /The recording has a gap of 0\.42 s at 1\.2 s\./);
  });

  test("interpolates a named measured output between jittered scans and holds the other channels", () => {
    const scans = taskTimes(40).map((time, index) => ({ timeSeconds: time, sequence: 7 + index,
      values: { PV: { value: time, quality: "good" as const }, MV: { value: index, quality: "good" as const } } }));
    const { samples } = evenlySampled(scans, undefined, { linear: ["PV"] });
    assert.ok(samples.every(entry => Math.abs((entry.values.PV.value as number) - entry.timeSeconds) < 1e-9),
      "a linear output reproduces a ramp at every grid time");
    assert.ok(samples.every(entry => Number.isInteger(entry.values.MV.value as number)), "the held command takes whole scan values");
  });

  test("block-averages a long recording without mixing the two sides of a step", () => {
    const samples = Array.from({ length: 12_000 }, (_, index) => ({ timeSeconds: index / 100, input: index < 3000 ? 0 : 1,
      output: index / 100, quality: "good" as const }));
    const averaged = blockAveraged(samples, MAXIMUM_FIT_SAMPLES, [30]);
    assert.ok(averaged.length <= MAXIMUM_FIT_SAMPLES, `${averaged.length} blocks`);
    assert.ok(averaged.every(sample => sample.input === 0 || sample.input === 1), "no block mixes the two sides of the step");
    assert.ok(averaged.every((sample, index) => index === 0 || sample.timeSeconds > averaged[index - 1].timeSeconds),
      "block times stay ordered");
  });
});
