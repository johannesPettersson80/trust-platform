import * as assert from "assert";
import { identifyStateModel } from "../../controlStudio/identifiedStateModel";
import { identifyPlantFromExcitation } from "../../controlStudio/excitationIdentification";
import { identifyPlant, type IdentificationSample, type IdentifiedModel } from "../../controlStudio/identification";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { medianSpacing } from "../../controlStudio/evenSampling";
import { checkGainSign } from "../../controlStudio/recordedModelIdentification";
import type { RecordedRun } from "../../controlStudio/runValidation";
import { analyzeStepResponse, type StepResponseSettings } from "../../controlStudio/stepResponseIdentification";
import { tuneController } from "../../controlStudio/tuning";

const TIME_CONSTANT = 60;
const BASELINE_OUTPUT = 35;
const INPUT_BEFORE = 75;
const INPUT_AFTER = 85;

/** Unit-step response of a first-order lag, zero before its start. */
function rise(elapsed: number): number {
  return elapsed > 0 ? 1 - Math.exp(-elapsed / TIME_CONSTANT) : 0;
}

/** A held step of `change` output units from `stepTime` to `releaseTime`, sampled every half second. An optional alternating
 * deviation of `noise` gives the baseline a known first-difference RMS (noise times the square root of two). */
function heldStepRecord(change: number, holdSeconds: number, noise = 0): IdentificationSample[] {
  const stepTime = 60, releaseTime = stepTime + holdSeconds;
  const samples: IdentificationSample[] = [];
  for (let index = 0; index * 0.5 <= releaseTime + 0.5; index += 1) {
    const timeSeconds = index * 0.5;
    const held = timeSeconds >= stepTime && timeSeconds < releaseTime;
    const output = BASELINE_OUTPUT + change * (rise(timeSeconds - stepTime) - rise(timeSeconds - releaseTime))
      + (index % 2 === 0 ? noise : -noise);
    samples.push({ timeSeconds, input: held ? INPUT_AFTER : INPUT_BEFORE, output, quality: "good",
      inputUnit: "%", outputUnit: "°C" });
  }
  return samples;
}

const stepSettings: StepResponseSettings = {
  baselineDurationSeconds: 60, finalDurationSeconds: 1, maximumWindowRange: 0.5, maximumWindowSlopePerSecond: 0.01,
  measurementBound: 0.05, onsetThreshold: 0.15, onsetSamples: 3,
  delay: { minimum: 0, maximum: 0, step: 1 }, timeConstant: { minimum: 30, maximum: 90, step: 1 },
  baselineStep: 0.01, gainStep: 0.001, timestampShiftSeconds: 0,
};

/** Alternating 100 and 110 ms execution intervals, as a 100 ms task in a 10 ms resource cycle runs them. */
function jitteredTimes(count: number): number[] {
  const times = [0];
  for (let index = 1; index < count; index += 1) times.push(times[index - 1] + (index % 2 === 1 ? 0.11 : 0.1));
  return times;
}

/** One-scan binary excitation from a 7-stage shift register, as +/-0.5 after a 10-scan baseline at zero. */
function prbsInput(count: number): number[] {
  let register = 0x7f;
  const bit = (): number => {
    const value = register & 1;
    register = (register >> 1) | ((((register >> 0) ^ (register >> 1)) & 1) << 6);
    return value ? 0.5 : -0.5;
  };
  return Array.from({ length: count }, (_, index) => (index < 10 ? 0 : bit()));
}

/** A loop with the given controller action ("direct": increase the output while the measurement is below the setpoint, expecting a
 * positive plant gain; "reverse": above the setpoint, expecting a negative gain), with the units its gain sign is reported in. */
function loopWithAction(action: "direct" | "reverse"): ControlLoopDefinition {
  return { signals: { processVariable: { unit: "°C" }, manipulatedVariable: { unit: "%" } },
    controller: { action } } as unknown as ControlLoopDefinition;
}

/** Closed-loop response of a first-order-with-delay plant to a setpoint step, under the parallel Control.PID law with signed gains: the
 * error is setpoint minus output, the command is Kp e plus the integral, and the integral grows by Ki e dt (N0.5, N6.6). */
function closedLoopResponse(plant: { gain: number; tauSeconds: number; delaySeconds: number }, gains: { kp: number; ki: number },
  setpoint: number, seconds: number, sampleSeconds: number): number[] {
  const lag = Math.exp(-sampleSeconds / plant.tauSeconds);
  const delaySteps = Math.round(plant.delaySeconds / sampleSeconds);
  const commands: number[] = [];
  const outputs: number[] = [];
  let output = 0, integral = 0;
  for (let step = 0; step < Math.round(seconds / sampleSeconds); step += 1) {
    const error = setpoint - output;
    commands.push(gains.kp * error + integral);
    integral += gains.ki * error * sampleSeconds;
    const delayed = step >= delaySteps ? commands[step - delaySteps] : 0;
    output = lag * output + (1 - lag) * plant.gain * delayed;
    outputs.push(output);
  }
  return outputs;
}

/** A held step of 10 input units at `stepSeconds`, sampled every 10 s from 0 to `endSeconds`; the output rises by `response(elapsed)`. */
function stepRecordAt(endSeconds: number, stepSeconds: number, response: (elapsed: number) => number): IdentificationSample[] {
  const samples: IdentificationSample[] = [];
  for (let timeSeconds = 0; timeSeconds <= endSeconds; timeSeconds += 10) {
    samples.push({ timeSeconds, input: timeSeconds >= stepSeconds ? INPUT_AFTER : INPUT_BEFORE,
      output: BASELINE_OUTPUT + response(Math.max(0, timeSeconds - stepSeconds)), quality: "good", inputUnit: "%", outputUnit: "°C" });
  }
  return samples;
}

suite("Control Studio long-process fits", () => {
  test("accepts a held step that has reached 70 % of its final change and reports the fraction", () => {
    // A 75 s hold of a 60 s time constant ends at about 71 % of the final change; the 5 % tail is averaged.
    const result = analyzeStepResponse(heldStepRecord(20, 75), stepSettings);
    assert.ok(result.finalChangeFraction !== undefined, "the held step reports its fraction");
    assert.ok(result.finalChangeFraction! >= 0.65 && result.finalChangeFraction! <= 0.75,
      `fraction ${result.finalChangeFraction}`);
    assert.ok(Math.abs(result.best.gain / 2 - 1) <= 0.01, `gain ${result.best.gain} against 2`);
  });

  test("refuses a held step that has reached only 40 %, naming the fraction and a suggested hold", () => {
    // A 32 s hold ends at about 41 % of the final change. The suggested hold is the delay plus three time constants: 3 min.
    assert.throws(() => analyzeStepResponse(heldStepRecord(20, 32), stepSettings), (error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      return /reached (39|40|41) % of its fitted final change/.test(message) && /hold the step for about 3 min/.test(message);
    });
  });

  test("recovers the gain of a PRBS excitation with one-scan bits at alternating 100 and 110 ms within 1 %", () => {
    const times = jitteredTimes(400);
    const input = prbsInput(400);
    const plantGain = 1.5, lagSeconds = 2;
    // The plant holds each scan's command until the next scan, with the exact discretization of its interval.
    let output = 0;
    const samples: IdentificationSample[] = times.map((timeSeconds, index) => {
      const sample = { timeSeconds, input: input[index], output, quality: "good" as const };
      const decay = index + 1 < times.length ? Math.exp(-(times[index + 1] - timeSeconds) / lagSeconds) : 0;
      output = decay * output + (1 - decay) * plantGain * input[index];
      return sample;
    });
    const fit = identifyPlantFromExcitation(samples, 0.5);
    const firstOrder = fit.models.find(model => model.kind === "first_order");
    assert.ok(firstOrder, "the first-order candidate is fitted");
    assert.ok(Math.abs(firstOrder.gain / plantGain - 1) <= 0.01, `gain ${firstOrder.gain} against ${plantGain}`);
  });

  test("refuses a response buried in the measured noise, naming both values", () => {
    // Baseline noise of 0.05 gives a first-difference RMS of 0.0707; twenty times that is 1.414, above the 1.0 response.
    const samples = heldStepRecord(1.0, 75, 0.05);
    assert.throws(() => analyzeStepResponse(samples, stepSettings), (error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      return /is smaller than 20 times the measured noise/.test(message) && /Permit a larger step in Limits/.test(message);
    });
  });

  test("accepts a response just above the noise threshold", () => {
    // The same noise with a 1.6 response, above the 1.414 threshold, is fitted and accepted.
    const result = analyzeStepResponse(heldStepRecord(1.6, 75, 0.05), stepSettings);
    assert.ok(Math.abs(result.best.gain / 0.16 - 1) <= 0.05, `gain ${result.best.gain} against 0.16`);
  });

  test("refuses a negative plant gain on a loop that increases the output while the measurement is below the setpoint", () => {
    assert.throws(() => checkGainSign(loopWithAction("direct"), -0.42, false), (error: unknown) => error instanceof Error
      && error.message === "The identified gain is −0.42 °C/% (the measurement falls when the output rises), but the loop is set up to increase the output while the measurement is below the setpoint. Check the measured output, the plant input and the controller action in the loop setup.");
    assert.doesNotThrow(() => checkGainSign(loopWithAction("direct"), 0.42, false));
  });

  test("accepts a negative plant gain on a loop that increases the output while the measurement is above the setpoint", () => {
    assert.doesNotThrow(() => checkGainSign(loopWithAction("reverse"), -0.42, false));
    assert.throws(() => checkGainSign(loopWithAction("reverse"), 0.42, false), /the measurement rises when the output rises\), but the loop is set up to increase the output while the measurement is above the setpoint/);
  });

  test("identifies a negative-gain plant on the above-setpoint loop, tunes it to negative Kp and Ki, and its closed loop settles", () => {
    // A 20 unit output fall for the 10 unit input rise: a plant gain of -2 with a 60 s time constant and no delay.
    const identified = analyzeStepResponse(heldStepRecord(-20, 75), stepSettings);
    assert.ok(identified.best.gain < 0, `identified gain ${identified.best.gain}`);
    assert.doesNotThrow(() => checkGainSign(loopWithAction("reverse"), identified.best.gain, false));
    const model: IdentifiedModel = { kind: "first_order_dead_time", gain: identified.best.gain,
      timeConstantSeconds: identified.best.timeConstantSeconds, deadTimeSeconds: identified.best.delaySeconds,
      stable: true, normalizedError: 0 };
    const tuned = tuneController(model, { method: "SIMC", objective: "balanced", action: "reverse" });
    const kp = tuned.controller.kp, ki = kp / tuned.controller.tiSeconds!;
    assert.ok(kp < 0 && ki < 0, `Kp ${kp} and Ki ${ki} must be negative for a negative plant gain`);
    const response = closedLoopResponse({ gain: identified.best.gain, tauSeconds: identified.best.timeConstantSeconds,
      delaySeconds: identified.best.delaySeconds }, { kp, ki }, 1, 600, 1);
    assert.ok(response.slice(-120).every(value => Math.abs(value - 1) <= 0.02), "the predicted closed loop settles on the setpoint");
    assert.ok(Math.max(...response) < 1.5, "the predicted closed loop does not run away");
  });

  test("refuses a negative gain tuned on the below-setpoint loop, with the same message as Identify", () => {
    const model: IdentifiedModel = { kind: "first_order_dead_time", gain: -1.5, timeConstantSeconds: 4,
      deadTimeSeconds: 0.5, stable: true, normalizedError: 0 };
    assert.throws(() => tuneController(model, { method: "SIMC", objective: "balanced", action: "direct" }), (error: unknown) =>
      error instanceof Error && error.message === "The identified gain is −1.5 (the measurement falls when the output rises), but the loop is set up to increase the output while the measurement is below the setpoint. Check the measured output, the plant input and the controller action in the loop setup.");
  });

  test("identifies a state model from a resampled grid whose length differs from the raw scan count", () => {
    const times = jitteredTimes(400);
    const input = prbsInput(400);
    let output = 0;
    const scans = times.map((timeSeconds, index) => {
      const scan = { sequence: index, timeSeconds, values: {
        u: { value: input[index], quality: "good" as const },
        y: { value: output, quality: "good" as const } } };
      const decay = index + 1 < times.length ? Math.exp(-(times[index + 1] - timeSeconds) / 2) : 0;
      output = decay * output + (1 - decay) * 1.5 * input[index];
      return scan;
    });
    const run = { id: "estimation", samples: scans, channels: [{ id: "u", unit: "%" }, { id: "y", unit: "°C" }],
      conditions: { programRevision: "p", modelRevision: "m", sampleTimesSeconds: times } } as unknown as RecordedRun;
    const channels = { inputs: [{ id: "u", unit: "%", operatingRange: { minimum: -1, maximum: 1 } }],
      outputs: [{ id: "y", unit: "°C", operatingRange: { minimum: -1, maximum: 2 } }] } as unknown as Parameters<typeof identifyStateModel>[2];
    const result = identifyStateModel(run, undefined, channels);
    assert.ok(Math.abs((result.model.sampleTimeSeconds ?? Number.NaN) - medianSpacing(times)) <= 1e-9, "the grid step is the median spacing");
  });

  test("refuses a slow first-order response as an integrator that the data cannot yet tell apart from a self-regulating one", () => {
    // A 4500 s time constant stepped for 2300 s: the response still looks like a ramp, so the integrator is not yet acceptable.
    const samples = stepRecordAt(2400, 100, elapsed => 20 * (1 - Math.exp(-elapsed / 4500)));
    const result = identifyPlant(samples);
    assert.strictEqual(result.recommended.kind, "integrating");
    assert.match(result.integratorRefusal ?? "", /still rising like a ramp after 38 min; it cannot yet be told whether it levels off\. Hold the step longer/);
  });

  test("accepts the same slow response as an integrator when the policy declares an integrating process", () => {
    const samples = stepRecordAt(2400, 100, elapsed => 20 * (1 - Math.exp(-elapsed / 4500)));
    const result = identifyPlant(samples, [], "Simulation", undefined, 0.15, { declaredProcess: "integrating" });
    assert.strictEqual(result.recommended.kind, "integrating");
    assert.strictEqual(result.integratorRefusal, undefined);
  });

  test("refuses a true integrator held for 2300 s without curvature unless declared, and not for a closed-loop fit", () => {
    // A ramp has no curvature: a first-order fit with a time constant far beyond the hold fits it as well, so it is ambiguous.
    const samples = stepRecordAt(2400, 100, elapsed => (20 / 2300) * elapsed);
    assert.match(identifyPlant(samples).integratorRefusal ?? "", /still rising like a ramp/);
    assert.strictEqual(identifyPlant(samples, [], "Simulation", undefined, 0.15, { declaredProcess: "integrating" }).integratorRefusal, undefined);
    assert.strictEqual(identifyPlant(samples, [], "Simulation", undefined, 0.15, { closedLoop: true }).integratorRefusal, undefined);
  });
});
