import * as assert from "assert";
import { compareRecordedRuns, evaluateRecordedRun } from "../../controlStudio/runValidation";
const run = (values = [0, 1, 1, 1, 1]) => ({ id: "run-a", evidence: "ST-executed", parameters: { gain: 2 },
  conditions: { initialState: { storage: 0 }, events: [{ atSeconds: 0, channel: "command", value: 1 }],
    durationSeconds: 4, sampleTimesSeconds: [0, 1, 2, 3, 4], seed: 31,
    modelRevision: "plant-1", programRevision: "st-1", parameterSchemaRevision: "params-1" },
  channels: [{ id: "response", unit: "m" }],
  samples: values.map((value, i) => ({ timeSeconds: i, sequence: 10 + i, values: { response: { value, quality: "good" } } })) });
const error = { id: "error", kind: "error", channel: "response", window: { start: 1, end: 4 }, reference: 1, statistic: "maximum", maximum: 0.1 };
suite("Control Studio recorded-run validation", () => {
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
    assert.strictEqual(evaluateRecordedRun(timing, [error]).status, "Inconclusive");
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
});
