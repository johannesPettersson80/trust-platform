import * as assert from "assert";
import { BoundedAnalysisJob } from "../../controlStudio/boundedAnalysisJob";
import { identifyPlantFromExcitation } from "../../controlStudio/excitationIdentification";
import { fitRecordedModel } from "../../controlStudio/recordedModelIdentification";
import { pidLoop } from "./control-studio-pid-fixtures";

/** A feedback-relay test on a delayed first-order plant: steady baseline, then switching at y0 ± band. */
function relayRecord(options: { gain: number; tau: number; delay: number; u0: number; y0: number; d: number; band: number; noise: number }) {
  const dt = 0.1, steps = Math.round(options.delay / dt), a = Math.exp(-dt / options.tau);
  let seed = 20260927, x = 0, high = true;
  const random = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296) * 2 - 1;
  const applied: number[] = [], samples: { timeSeconds: number; input: number; output: number }[] = [];
  for (let k = 0; k <= 600; k++) {
    const output = options.y0 + x + options.noise * random();
    let input = options.u0;
    if (k >= 60 && k < 599) {
      if (output >= options.y0 + options.band) high = false;
      else if (output <= options.y0 - options.band) high = true;
      input = options.u0 + (high ? options.d : -options.d);
    }
    applied.push(input);
    samples.push({ timeSeconds: Number((k * dt).toFixed(10)), input, output });
    x = a * x + (1 - a) * options.gain * ((applied[k - steps] ?? options.u0) - options.u0);
  }
  return samples;
}

/** A PI loop in Auto on a delayed first-order plant, recorded through a setpoint step. */
function setpointStepRecord(options: { gain: number; tau: number; delay: number; kp: number; ki: number }) {
  const dt = 0.1, steps = Math.round(options.delay / dt), a = Math.exp(-dt / options.tau);
  let x = 0, integral = 0;
  const applied: number[] = [], samples: { timeSeconds: number; input: number; output: number }[] = [];
  for (let k = 0; k <= 400; k++) {
    const output = 20 + x, setpoint = k >= 20 ? 25 : 20, error = setpoint - output;
    integral += options.ki * error * dt;
    const input = Math.min(100, Math.max(0, options.kp * error + integral));
    applied.push(input);
    samples.push({ timeSeconds: Number((k * dt).toFixed(10)), input, output });
    x = a * x + (1 - a) * options.gain * (applied[k - steps] ?? 0);
  }
  return samples;
}

/** A PI loop in Auto on a delayed first-order plant whose measurement is noisy: the controller moves the plant input every scan,
 * also before the setpoint step at `stepAt` samples (0.1 s each). */
function noisySetpointStepRecord(options: { gain: number; tau: number; delay: number; kp: number; ki: number; noise: number; stepAt: number }) {
  const dt = 0.1, steps = Math.round(options.delay / dt), a = Math.exp(-dt / options.tau);
  let seed = 20261008, x = 0, integral = 0;
  const random = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296) * 2 - 1;
  const applied: number[] = [], samples: { timeSeconds: number; input: number; output: number }[] = [];
  for (let k = 0; k <= 400; k++) {
    const output = 20 + x + options.noise * random(), setpoint = k >= options.stepAt ? 25 : 20, error = setpoint - output;
    integral += options.ki * error * dt;
    const input = Math.min(100, Math.max(0, 10 + options.kp * error + integral));
    applied.push(input);
    samples.push({ timeSeconds: Number((k * dt).toFixed(10)), input, output });
    x = a * x + (1 - a) * options.gain * ((applied[k - steps] ?? 10) - 10);
  }
  return samples;
}

function near(actual: number | undefined, expected: number, tolerance: number, label: string): void {
  assert.ok(actual !== undefined && Math.abs(actual - expected) <= tolerance, `${label}: expected ${expected} ± ${tolerance}, got ${actual}`);
}

suite("Control Studio excitation identification", () => {
  test("fits a delayed first-order model to a relay recording from its baseline", () => {
    const samples = relayRecord({ gain: 0.2, tau: 5, delay: 1, u0: 75, y0: 35, d: 5, band: 0.04, noise: 0.02 })
      .map(sample => ({ ...sample, quality: "good" as const }));
    const fit = identifyPlantFromExcitation(samples, 0.1);
    assert.strictEqual(fit.recommended.kind, "first_order_dead_time", "The heater's one-second delay needs the delayed first-order model");
    near(fit.recommended.gain, 0.2, 0.01, "process gain K");
    near(fit.recommended.timeConstantSeconds, 5, 0.5, "time constant τ");
    near(fit.recommended.deadTimeSeconds, 1, 0.15, "dead time L");
    assert.ok(fit.recommended.normalizedError <= 0.1, `normalized error ${fit.recommended.normalizedError}`);
    assert.strictEqual(fit.input0, 75, "The operating point is the baseline before the first switch, not before the largest one");
    near(fit.output0, 35, 0.02, "baseline temperature");
    assert.strictEqual(fit.points.length, samples.length, "Every sample has a model prediction for the chart");
  });

  test("identifies a relay recording as a delayed first-order transfer function", async () => {
    const loop = pidLoop();
    loop.experiment = { execution: "control-studio", kind: "relay", amplitude: 1, durationSeconds: 60, seed: 1, relayDirection: "positive" };
    loop.identificationPolicy = { maximumNormalizedError: 0.1 };
    const recorded = relayRecord({ gain: 2, tau: 5, delay: 1, u0: 0, y0: 1, d: 1, band: 0.1, noise: 0.02 });
    const run: any = { projectRevision: "project", request: { execution: "connected-runtime" }, evaluation: { status: "Pass" },
      run: { id: "relay", evidence: "ST-executed", parameters: {}, channels: [{ id: "Main.MV", unit: "V" }, { id: "Main.PV", unit: "m" }],
        samples: recorded.map((sample, sequence) => ({ timeSeconds: sample.timeSeconds, sequence,
          values: { "Main.MV": { value: sample.input, quality: "good" }, "Main.PV": { value: sample.output, quality: "good" } } })),
        conditions: { initialState: {}, events: [], durationSeconds: 60, sampleTimesSeconds: recorded.map(sample => sample.timeSeconds),
          seed: 1, modelRevision: "connected-plant:project", programRevision: "project", parameterSchemaRevision: loop.revision } } };
    const result: any = await fitRecordedModel(loop, [run], new BoundedAnalysisJob(), () => false).catch((error: Error) => ({ error }));
    assert.strictEqual(result.model?.kind, "transfer-function",
      `A relay test must first try the spec's simple models, got ${result.model?.kind ?? result.error?.message}`);
    near(result.model.delaySeconds, 1, 0.15, "dead time L");
    near(result.model.numerator[0] / result.model.denominator[1], 2, 0.1, "process gain K");
    near(result.model.denominator[0] / result.model.denominator[1], 5, 0.5, "time constant τ");
    assert.deepStrictEqual(result.model.operatingPoint, { inputs: [0], outputs: [result.model.operatingPoint.outputs[0]] });
    near(result.model.operatingPoint.outputs[0], 1, 0.02, "baseline output");
  });

  test("takes the operating point over the steady baseline window a loop in Auto records before its setpoint step (N1.3, N2.2)", () => {
    const samples = noisySetpointStepRecord({ gain: 1.5, tau: 4, delay: 0.5, kp: 1, ki: 0.3, noise: 0.02, stepAt: 100 })
      .map(sample => ({ ...sample, quality: "good" as const }));
    // The controller moves the plant input with the noise from the first scan, so no sample is a first input change.
    assert.throws(() => identifyPlantFromExcitation(samples, 0.1), /steady baseline of at least three samples/);
    const fit = identifyPlantFromExcitation(samples, 0.1, 9.95);
    const window = samples.filter(sample => sample.timeSeconds <= 9.95);
    near(fit.input0, window.reduce((sum, sample) => sum + sample.input, 0) / window.length, 1e-9, "u0, the mean plant input over the window");
    near(fit.output0, window.reduce((sum, sample) => sum + sample.output, 0) / window.length, 1e-9, "y0, the mean measurement over the window");
    near(fit.recommended.gain, 1.5, 0.1, "process gain K");
    near((fit.recommended.timeConstantSeconds ?? 0) + (fit.recommended.deadTimeSeconds ?? 0), 4.5, 0.4, "lag plus delay τ + L");
  });

  test("fits a noisy closed-loop setpoint step from the steady baseline window its record names", async () => {
    const loop = pidLoop();
    loop.experiment = { execution: "control-studio", kind: "automatic", amplitude: 5, durationSeconds: 40, seed: 1 };
    loop.identificationPolicy = { maximumNormalizedError: 0.1 };
    const recorded = noisySetpointStepRecord({ gain: 1.5, tau: 4, delay: 0.5, kp: 1, ki: 0.3, noise: 0.02, stepAt: 100 });
    const run: any = { projectRevision: "project", evaluation: { status: "Pass" },
      request: { execution: "connected-runtime", experiment: { kind: "setpoint_step" }, baselineStartSeconds: 5, baselineEndSeconds: 9.95 },
      run: { id: "noisy-setpoint-step", evidence: "ST-executed", parameters: {}, channels: [{ id: "Main.MV", unit: "V" }, { id: "Main.PV", unit: "m" }],
        samples: recorded.map((sample, sequence) => ({ timeSeconds: sample.timeSeconds, sequence,
          values: { "Main.MV": { value: sample.input, quality: "good" }, "Main.PV": { value: sample.output, quality: "good" } } })),
        conditions: { initialState: {}, events: [], durationSeconds: 40, sampleTimesSeconds: recorded.map(sample => sample.timeSeconds),
          seed: 1, modelRevision: "connected-plant:project", programRevision: "project", parameterSchemaRevision: loop.revision } } };
    const result: any = await fitRecordedModel(loop, [run], new BoundedAnalysisJob(), () => false).catch((error: Error) => ({ error }));
    assert.strictEqual(result.model?.kind, "transfer-function", `got ${result.model?.kind ?? result.error?.message}`);
    near(result.model.numerator[0] / result.model.denominator[result.model.denominator.length - 1], 1.5, 0.1, "process gain K");
  });

  test("fits the simple delayed first-order model to the closed-loop step that Automatic runs", async () => {
    const loop = pidLoop();
    loop.experiment = { execution: "control-studio", kind: "automatic", amplitude: 5, durationSeconds: 40, seed: 1 };
    loop.identificationPolicy = { maximumNormalizedError: 0.1 };
    const recorded = setpointStepRecord({ gain: 1.5, tau: 4, delay: 0.5, kp: 1, ki: 0.3 });
    const run: any = { projectRevision: "project", request: { execution: "connected-runtime", experiment: { kind: "setpoint_step" } }, evaluation: { status: "Pass" },
      run: { id: "setpoint-step", evidence: "ST-executed", parameters: {}, channels: [{ id: "Main.MV", unit: "V" }, { id: "Main.PV", unit: "m" }],
        samples: recorded.map((sample, sequence) => ({ timeSeconds: sample.timeSeconds, sequence,
          values: { "Main.MV": { value: sample.input, quality: "good" }, "Main.PV": { value: sample.output, quality: "good" } } })),
        conditions: { initialState: {}, events: [], durationSeconds: 40, sampleTimesSeconds: recorded.map(sample => sample.timeSeconds),
          seed: 1, modelRevision: "connected-plant:project", programRevision: "project", parameterSchemaRevision: loop.revision } } };
    const result: any = await fitRecordedModel(loop, [run], new BoundedAnalysisJob(), () => false).catch((error: Error) => ({ error }));
    assert.strictEqual(result.model?.kind, "transfer-function",
      `The executed setpoint step must first try the spec's simple models, got ${result.model?.kind ?? result.error?.message}`);
    near(result.model.numerator[0] / result.model.denominator[1], 1.5, 0.1, "process gain K");
    // The simplest adequate model may lump the 0.5 s delay into the lag: τ + L stays the plant's 4.5 s.
    near(result.model.denominator[0] / result.model.denominator[1] + result.model.delaySeconds, 4.5, 0.3, "lag plus delay τ + L");
  });
});
