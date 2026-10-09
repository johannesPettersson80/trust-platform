import * as assert from "assert";
import { compareRecordedRuns, evaluateRecordedRun } from "../../controlStudio/runValidation";
import { ISOLATED_SIMULATION_SAMPLE_LIMIT } from "../../controlStudio/controlStudioLimits";
const run = (values = [0, 1, 1, 1, 1]) => ({ id: "run-a", evidence: "ST-executed", parameters: { gain: 2 },
  conditions: { initialState: { storage: 0 }, events: [{ atSeconds: 0, channel: "command", value: 1 }],
    durationSeconds: 4, sampleTimesSeconds: [0, 1, 2, 3, 4], seed: 31,
    modelRevision: "plant-1", programRevision: "st-1", parameterSchemaRevision: "params-1" },
  channels: [{ id: "response", unit: "m" }],
  samples: values.map((value, i) => ({ timeSeconds: i, sequence: 10 + i, values: { response: { value, quality: "good" } } })) });
const error = { id: "error", kind: "error", channel: "response", window: { start: 1, end: 4 }, reference: 1, statistic: "maximum", maximum: 0.1 };
suite("Control Studio recorded-run validation", () => {
  test("evaluates a run recorded at a record period, whose samples sit at the mean times of their executions (A4.2, E4.4)", () => {
    // Nine executions 1 s apart, averaged three per sample: the samples sit at 1, 4 and 7 s of an 8 s run.
    const averaged = { id: "averaged", evidence: "ST-executed", parameters: {}, conditions: { initialState: {}, events: [], durationSeconds: 8,
      sampleTimesSeconds: [1, 4, 7], seed: 1, modelRevision: "plant-1", programRevision: "st-1", parameterSchemaRevision: "params-1", recordEvery: 3 },
      channels: [{ id: "response", unit: "m" }],
      samples: [1, 4, 7].map((timeSeconds, index) => ({ timeSeconds, sequence: [3, 6, 9][index],
        values: { response: { value: [0.5, 1, 1][index], quality: "good" } } })) };
    const whole = evaluateRecordedRun(averaged, [{ id: "whole-run", kind: "error", channel: "response", window: { start: 0, end: 8 },
      reference: 1, statistic: "maximum", maximum: 0.6 }]);
    assert.strictEqual(whole.status, "Pass", JSON.stringify(whole.requirements));
    assert.strictEqual(whole.requirements[0].value, 0.5, "the window from the start to the end covers every averaged sample");
    assert.throws(() => evaluateRecordedRun({ ...averaged, conditions: { ...averaged.conditions, recordEvery: undefined } }, []),
      /Invalid immutable experiment conditions/, "a run that does not declare its record period must start at 0");
  });

  test("accepts every channel an isolated run records, up to 64", () => {
    const wide = (count: number) => {
      const source: any = run();
      const extra = Array.from({ length: count - 1 }, (_, index) => ({ id: `state-${index}`, unit: "1" }));
      source.channels = [...source.channels, ...extra];
      for (const sample of source.samples) for (const channel of extra) sample.values[channel.id] = { value: 0, quality: "good" };
      return source;
    };
    assert.strictEqual(evaluateRecordedRun(wide(64), [error]).status, "Pass", "a PID group's transfer check records up to 64 channels");
    assert.throws(() => evaluateRecordedRun(wide(65), [error]), /Invalid run channels/);
  });
  test("evaluates engineer-declared error and immutable run evidence", () => {
    const source = run();
    const result = evaluateRecordedRun(source, [error]);
    assert.strictEqual(result.status, "Pass");
    assert.strictEqual(result.requirements[0].value, 0);
    source.conditions.initialState.storage = 99;
    assert.strictEqual(result.run.conditions.initialState.storage, 0);
  });
  test("freezes experiment conditions and rejects an unobservable settling deadline", () => {
    const result = evaluateRecordedRun(run(), [error]);
    assert.ok(Object.isFrozen(result.run.conditions));
    assert.ok(Object.isFrozen(result.run.conditions.initialState));
    assert.throws(() => evaluateRecordedRun(run(), [{ id: "late", kind: "settling", channel: "response",
      window: { start: 0, end: 4 }, target: 1, band: 0.1, holdSeconds: 2, deadlineSeconds: 3 }]));
  });
  test("computes RMS error in Studio from the declared recorded window", () => {
    const requirement = { ...error, statistic: "rms", maximum: 2.3 };
    let result!: ReturnType<typeof evaluateRecordedRun>;
    assert.doesNotThrow(() => { result = evaluateRecordedRun(run([0,2,4,2,4]), [requirement]); }, "RMS is a supported recorded-error measurement");
    assert.strictEqual(result.status, "Pass");
    assert.ok(Math.abs(result.requirements[0].value! - Math.sqrt(5)) < 1e-12);
    assert.strictEqual(evaluateRecordedRun(run([0,2,4,2,4]), [{ ...requirement, maximum: 2.1 }]).status, "Fail");
  });
  test("evaluates directional overshoot, settling, limits, rates, saturation and switching", () => {
    const window = { start: 0, end: 4 }; const common = { channel: "response", window };
    const requirements = [
      { ...common, id: "overshoot", kind: "overshoot", initial: 0, target: 1, maximum: 0.1 },
      { ...common, id: "settle", kind: "settling", target: 1, band: 0.05, holdSeconds: 2, deadlineSeconds: 2 },
      { ...common, id: "bounds", kind: "bounds", minimum: 0, maximum: 1 },
      { ...common, id: "rate", kind: "rate", maximum: 1 },
      { ...common, id: "sat", kind: "saturation", minimum: -1, maximum: 1, maximumSeconds: 3 },
      { ...common, id: "switch", kind: "switching", threshold: 0.2, maximum: 1 },
    ];
    const result = evaluateRecordedRun(run(), requirements);
    assert.strictEqual(result.status, "Pass");
    assert.deepStrictEqual(result.requirements.map((r: any) => r.value), [0, 1, 0, 1, 3, 1]);
    assert.strictEqual(evaluateRecordedRun(run([0, 1.4, 1, 1, 1]), requirements).status, "Fail");
    const down = { ...common, id: "down", kind: "overshoot", initial: 2, target: 1, maximum: 0.1 };
    assert.strictEqual(evaluateRecordedRun(run([2, 0.8, 1, 1, 1]), [down]).status, "Fail");
  });
  test("makes missing, forced and gapped coverage inconclusive", () => {
    const forced = run(); forced.samples[2].values.response.quality = "forced";
    const gap = run(); gap.samples[2].sequence += 1;
    for (const source of [forced, gap, { ...run(), samples: run().samples.slice(0, 4) }]) {
      assert.strictEqual(evaluateRecordedRun(source, [error]).status, "Inconclusive");
      assert.strictEqual(evaluateRecordedRun(source, [error]).requirements[0].limit, 0.1);
    }
  });
  test("requires actual timing and complete references while evaluating independent violations", () => {
    const timing = run(); timing.samples[2].timeSeconds = 2.01;
    const jittered = evaluateRecordedRun(timing, [error]);
    assert.strictEqual(jittered.status, "Pass", "a 10 ms shift is normal task jitter, not a gap");
    assert.strictEqual(jittered.requirements[0].value, 0);
    const gapped = run(); gapped.samples[3].timeSeconds = 3.9;
    const gap = evaluateRecordedRun(gapped, [error]);
    assert.strictEqual(gap.status, "Inconclusive");
    assert.strictEqual(gap.requirements[0].reason, "The recording has a gap of 1.9 s at 2 s.");
    const nan = run(); nan.samples[2].values.response.value = NaN;
    assert.strictEqual(evaluateRecordedRun(nan, [error]).status, "Inconclusive");
    const source = run([0, 2, 2, 2, 2]);
    source.channels.push({ id: "target", unit: "m" });
    const reference = { ...error, reference: { channel: "target" } };
    const result = evaluateRecordedRun(source, [reference, { ...error, id: "observed-error" }]);
    assert.deepStrictEqual(result.requirements.map(r => r.status), ["Inconclusive", "Fail"]);
    assert.strictEqual(result.status, "Fail");
    assert.throws(() => evaluateRecordedRun(run(), [error, error]));
    assert.strictEqual(evaluateRecordedRun(run(), []).status, "Inconclusive");
    const mean = evaluateRecordedRun(run([0, 0.8, 1.2, 1, 1]), [{ ...error, statistic: "mean" }]);
    assert.strictEqual(mean.status, "Pass");
    assert.ok(Math.abs(mean.requirements[0].value! - 0.1) < 1e-12);
  });
  test("ranks only matched qualifying runs and rejects changed evidence or conditions", () => {
    const current = run([0, 1.05, 1.05, 1.05, 1.05]); const candidate = run();
    const comparison = compareRecordedRuns(current, candidate, [error], "error");
    assert.strictEqual(comparison.status, "Pass");
    assert.strictEqual(comparison.preferred, "candidate");
    assert.strictEqual(compareRecordedRuns(current, { ...candidate, evidence: "model-predicted" }, [error], "error").status, "Inconclusive");
    assert.strictEqual(compareRecordedRuns(current, { ...candidate, parameters: { gain: 3 } }, [error], "error").preferred, "candidate");
    candidate.conditions.seed++;
    assert.strictEqual(compareRecordedRuns(current, candidate, [error], "error").status, "Inconclusive");
    assert.strictEqual(compareRecordedRuns(run(), run([0, 2, 2, 2, 2]), [error], "error").preferred, "current");
  });
  test("decodes and evaluates a run longer than 12,000 samples", () => {
    // The isolated run keeps up to 100,000 samples (specification 35, A7; runtime protocol annex, E8.3). A 130 s run at 10 ms has about 13,000.
    const count = 12_001;
    const time = (index: number) => index / 100;
    const longRun = (last: number) => ({ ...run(), conditions: { ...run().conditions, durationSeconds: time(count - 1),
        sampleTimesSeconds: Array.from({ length: count }, (_, index) => time(index)) },
      samples: Array.from({ length: count }, (_, index) => ({ timeSeconds: time(index), sequence: 10 + index,
        values: { response: { value: index === count - 1 ? last : 1, quality: "good" } } })) });
    const requirement = { ...error, window: { start: 0, end: time(count - 1) } };
    const clean = evaluateRecordedRun(longRun(1), [requirement]);
    assert.strictEqual(clean.run.samples.length, count);
    assert.strictEqual(clean.status, "Pass");
    // Only the last sample deviates, so the maximum error is found only if the decoded run is evaluated to its end.
    const late = evaluateRecordedRun(longRun(1.5), [requirement]);
    assert.strictEqual(late.status, "Fail");
    assert.strictEqual(late.requirements[0].value, 0.5);
  });
  test("refuses a run one sample above the isolated-run limit", () => {
    const over = ISOLATED_SIMULATION_SAMPLE_LIMIT + 1;
    const longGrid = { ...run(), conditions: { ...run().conditions, durationSeconds: (over - 1) / 100,
      sampleTimesSeconds: Array.from({ length: over }, (_, index) => index / 100) }, samples: [] };
    assert.throws(() => evaluateRecordedRun(longGrid, [error]), /Invalid immutable experiment conditions/);
    const longSamples = { ...run(), samples: Array.from({ length: over }, () => ({})) };
    assert.throws(() => evaluateRecordedRun(longSamples, [error]), /Invalid run channels or sample structure/);
  });
});
