import * as assert from "assert";

import {
  analyzeStepResponse,
  measureStepResponse,
  qualifyStepResponse,
  validStepResponseSettings,
  type StepResponseSettings,
} from "../../controlStudio/stepResponseIdentification";
import type { IdentificationSample } from "../../controlStudio/identification";

const settings: StepResponseSettings = {
  baselineDurationSeconds: 1,
  finalDurationSeconds: 1,
  maximumWindowRange: 0.1,
  maximumWindowSlopePerSecond: 0.01,
  measurementBound: 0.05,
  onsetThreshold: 0.15,
  onsetSamples: 3,
  delay: { minimum: 0, maximum: 0.2, step: 0.1 },
  timeConstant: { minimum: 0.1, maximum: 0.4, step: 0.1 },
  baselineStep: 0.01,
  gainStep: 0.001,
  timestampShiftSeconds: 0,
};

function samples(overrides: Partial<Record<number, number>> = {}): IdentificationSample[] {
  const result: IdentificationSample[] = [];
  for (let index = 0; index <= 40; index += 1) {
    const timeSeconds = index * 0.1;
    const input = index < 10 ? 75 : index < 40 ? 85 : 75;
    const elapsed = Math.max(0, timeSeconds - 1 - 0.1);
    const output = overrides[index] ?? (index < 10 ? 35 : 35 + 2 * (1 - Math.exp(-elapsed / 0.2)));
    result.push({ timeSeconds, input, output, quality: "good" });
  }
  return result;
}

suite("Control Studio step-response numerics", () => {
  test("qualifies a complete held step and rejects drift", () => {
    assert.strictEqual(qualifyStepResponse(samples(), settings).suitable, true);
    const drifting = samples({ 0: 35, 1: 35.02, 2: 35.04, 3: 35.06, 4: 35.08, 5: 35.1, 6: 35.12, 7: 35.14, 8: 35.16, 9: 35.18 });
    const result = qualifyStepResponse(drifting, settings);
    assert.strictEqual(result.suitable, false);
    assert.match(result.reason ?? "", /baseline|range|slope/i);
  });

  test("detects three consecutive samples above the absolute onset threshold", () => {
    const result = analyzeStepResponse(samples({ 11: 35.1, 12: 35.2, 13: 35.3 }), settings);
    assert.ok(Math.abs(result.detectableOnsetSeconds! - 0.2) < 1e-9);
    assert.strictEqual(result.onsetThreshold, 35.15);
  });

  for (const inputSign of [-1, 1]) for (const outputSign of [-1, 1]) {
    test(`detects sustained onset for input sign ${inputSign} and output sign ${outputSign}`, () => {
      const record = samples({ 11: 35.2, 12: 35.1 }).map((sample) => ({
        ...sample,
        input: 75 + inputSign * (sample.input - 75),
        output: 35 + outputSign * (sample.output - 35),
      }));
      for (const result of [measureStepResponse(record, settings), analyzeStepResponse(record, settings)]) {
        assert.ok(Math.abs(result.detectableOnsetSeconds! - 0.3) < 1e-9,
          "onset must start at the first sustained crossing after the isolated excursion");
        assert.strictEqual(result.onsetThreshold, 35 + outputSign * 0.15);
      }
    });
  }

  test("leaves onset unavailable when the output does not change", () => {
    const record = samples().map((sample) => ({ ...sample, output: 35 }));
    // N2.5: an unchanged output has no onset. The fit refuses it: N3.2 normalizes by the output span, which is zero, so no
    // first-order fit can pass its error check (N3.3). The measurement still reports the onset as unavailable.
    assert.strictEqual(measureStepResponse(record, settings).detectableOnsetSeconds, undefined);
    assert.throws(() => analyzeStepResponse(record, settings), /no measurable final change/);
  });

  test("matches a small explicit brute-force consistency oracle", () => {
    const result = analyzeStepResponse(samples(), settings);
    const baseline = result.baseline.mean;
    const finalMean = result.final.mean;
    const deltaInput = result.inputChange;
    const points = result.points;
    const gainMin = (finalMean - baseline - 2 * settings.measurementBound) / deltaInput;
    const gainMax = (finalMean - baseline + 2 * settings.measurementBound) / deltaInput;
    let count = 0;
    for (let gain = gainMin; gain <= gainMax + 1e-9; gain += settings.gainStep) {
      for (let candidateBaseline = baseline - settings.measurementBound; candidateBaseline <= baseline + settings.measurementBound + 1e-9; candidateBaseline += settings.baselineStep) {
        for (let delay = settings.delay.minimum; delay <= settings.delay.maximum + 1e-9; delay += settings.delay.step) {
          for (let tau = settings.timeConstant.minimum; tau <= settings.timeConstant.maximum + 1e-9; tau += settings.timeConstant.step) {
            const predictedFinal = candidateBaseline + gain * deltaInput;
            const valid = predictedFinal >= finalMean - settings.measurementBound - 1e-8 && predictedFinal <= finalMean + settings.measurementBound + 1e-8 &&
              points.every((point) => {
                const elapsed = point.timeSeconds - result.stepTimeSeconds - delay;
                const predicted = elapsed < 0 ? candidateBaseline : candidateBaseline + gain * deltaInput * (1 - Math.exp(-elapsed / tau));
                return Math.abs(point.measured - predicted) <= settings.measurementBound + 1e-8;
              });
            if (valid) count += 1;
          }
        }
      }
    }
    assert.ok(count > 0, "the independent oracle must exercise a nonempty consistent set");
    assert.strictEqual(result.consistency.count, count);
  });

  test("returns an empty consistent set without inventing intervals", () => {
    const impossible = { ...settings, measurementBound: 0.000001 };
    const result = analyzeStepResponse(samples(), impossible);
    assert.strictEqual(result.consistency.count, 0);
    assert.strictEqual(result.consistency.gain, undefined);
    assert.ok(result.warnings.some((warning) => /parameter combination/i.test(warning)));
  });

  test("rejects zero steps and preserves source samples", () => {
    const source = samples();
    const before = JSON.stringify(source);
    const zeroStep = source.map((sample) => ({ ...sample, input: 75 }));
    const rejected = qualifyStepResponse(zeroStep, settings);
    assert.strictEqual(rejected.suitable, false);
    assert.match(rejected.reason ?? "", /nonzero input step/i);
    assert.throws(() => measureStepResponse(zeroStep, settings), /nonzero input step/i);
    assert.throws(() => analyzeStepResponse(zeroStep, settings), /nonzero input step/i);
    assert.strictEqual(JSON.stringify(source), before);
  });

  test("validates settings and rejects malformed sample quality", () => {
    assert.strictEqual(validStepResponseSettings(settings), true);
    assert.strictEqual(validStepResponseSettings({ ...settings, delay: { minimum: 1, maximum: 0, step: 0.1 } }), false);
    const bad = samples();
    bad[4] = { ...bad[4], quality: "bad" };
    assert.strictEqual(qualifyStepResponse(bad, settings).suitable, false);
  });

  test("requires the first baseline sample even when all remaining timestamps are uniform", () => {
    const incomplete = samples().slice(1);
    for (const timestampShiftSeconds of [-0.1, 0, 0.1]) {
      const rejected = qualifyStepResponse(incomplete, { ...settings, timestampShiftSeconds });
      assert.strictEqual(rejected.suitable, false, "a missing boundary sample must not qualify");
      assert.match(rejected.reason ?? "", /baseline window.*coverage/i);
    }
    assert.strictEqual(qualifyStepResponse(samples(), { ...settings, baselineDurationSeconds: 0.95 }).suitable, true,
      "a requested boundary between sample instants does not imply a missing sample");
  });

  test("rejects windows too short to contain two samples", () => {
    for (const field of ["baselineDurationSeconds", "finalDurationSeconds"]) {
      const rejected = qualifyStepResponse(samples(), { ...settings, [field]: 0.1 });
      assert.strictEqual(rejected.suitable, false);
      assert.match(rejected.reason ?? "", /window.*coverage/i);
    }
  });

  test("requires the final window to stay inside the held step", () => {
    const unchanged = samples().map((sample) => ({ ...sample, output: 35 }));
    const rejected = qualifyStepResponse(unchanged, { ...settings, finalDurationSeconds: 3.1 });
    assert.strictEqual(rejected.suitable, false, "a steady output cannot authorize a pre-step final window");
    assert.match(rejected.reason ?? "", /final window.*held step/i);
  });

  test("supports signed gain lattices and leaves zero delay as a one-sided admissible bound", () => {
    const reversed = samples().map((sample) => ({
      ...sample,
      output: 70 - sample.output,
    }));
    const result = analyzeStepResponse(reversed, { ...settings, delay: { minimum: 0.1, maximum: 0.1, step: 0.1 } });
    assert.ok((result.consistency.gain?.minimum ?? 0) < 0);

    const immediate = samples().map((sample, index) => ({
      ...sample,
      output: index < 10 ? 35 : index < 40 ? 35 + 2 * (1 - Math.exp(-(index * 0.1 - 1) / 0.2)) : 35,
    }));
    const zeroDelay = analyzeStepResponse(immediate, { ...settings, delay: { minimum: 0, maximum: 0, step: 0.1 } });
    assert.strictEqual(zeroDelay.best.delaySeconds, 0);
    assert.strictEqual(zeroDelay.boundaryLimited, false);
  });

  test("does not let timestamp-shift sensitivity relax a real timing gap", () => {
    const gapped = samples();
    gapped[15] = { ...gapped[15], timeSeconds: gapped[14].timeSeconds + 0.2 };
    assert.strictEqual(qualifyStepResponse(gapped, settings).suitable, false);
  });

  test("rejects clipped, saturated, and inconsistent-unit samples", () => {
    const clipped = samples();
    clipped[15] = { ...clipped[15], clipped: true };
    assert.strictEqual(qualifyStepResponse(clipped, settings).suitable, false);
    const saturated = samples();
    saturated[15] = { ...saturated[15], saturated: true };
    assert.strictEqual(qualifyStepResponse(saturated, settings).suitable, false);
    const units = samples().map((sample, index) => ({ ...sample, inputUnit: index < 20 ? "%" : "percent" }));
    assert.strictEqual(qualifyStepResponse(units, settings).suitable, false);
  });
});
