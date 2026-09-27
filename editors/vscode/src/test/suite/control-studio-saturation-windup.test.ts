import * as assert from "assert";

suite("Control Studio saturation and windup evidence", () => {
  test("retains requested output, applied clamp, integral growth and delayed saturation release from one complete runtime record", () => {
    const api = saturationApi();
    assert.strictEqual(typeof api.assessSaturationWindup, "function",
      "Control Studio needs a runtime-record analysis instead of hiding request behind the applied clamp");

    const result = api.assessSaturationWindup({
      outputMinimum: 0,
      outputMaximum: 100,
      requestedOutputId: "requested-power",
      integralStateId: "stored-correction",
      recording: completeRecording([
        sample(0, 35, 75, 75, 75),
        sample(0.1, 45, 105, 100, 80),
        sample(0.2, 45, 115, 100, 85),
        sample(0.3, 45, 125, 100, 90),
        sample(0.4, 35, 115, 100, 90),
        sample(0.5, 35, 105, 100, 90),
        sample(0.6, 35, 95, 95, 90),
      ]),
    });

    assert.deepStrictEqual(result, {
      status: "qualified",
      limit: "upper",
      enteredAtSeconds: 0.1,
      targetReturnAtSeconds: 0.4,
      releasedAtSeconds: 0.6,
      recoverySeconds: 0.2,
      peakRequestedOutput: 125,
      peakAppliedOutput: 100,
      maximumMismatch: 25,
      integralAtEntry: 80,
      integralAtTargetReturn: 90,
      integralAtRelease: 90,
      integralChangeDuringSaturation: 10,
      recommendation: "Recorded output mismatch and retained-integral change require an anti-windup candidate before this saturation-return case can qualify.",
    });
  });

  test("refuses incomplete, guessed, or bad-quality recordings before it presents a windup finding", () => {
    const api = saturationApi();
    assert.strictEqual(typeof api.assessSaturationWindup, "function");
    const input = {
      outputMinimum: 0,
      outputMaximum: 100,
      requestedOutputId: "requested-power",
      integralStateId: "stored-correction",
      recording: completeRecording([
        sample(0, 35, 75, 75, 75), sample(0.1, 45, 110, 100, 80),
        sample(0.2, 35, 95, 95, 80),
      ]),
    };
    assert.deepStrictEqual(api.assessSaturationWindup({ ...input,
      recording: { ...input.recording, evidence: { ...input.recording.evidence, completeness: "incomplete" } },
    }), { status: "not_qualified", reason: "A complete runtime-owned recording is required for saturation and windup analysis." });
    assert.deepStrictEqual(api.assessSaturationWindup({ ...input, integralStateId: "not-recorded" }), {
      status: "not_qualified",
      reason: "The declared retained-integral signal is missing from the runtime record.",
    });
    assert.deepStrictEqual(api.assessSaturationWindup({ ...input,
      recording: completeRecording([sample(0, 35, 75, 75, 75), sample(0.1, 45, 110, 100, 80, "bad"), sample(0.2, 35, 95, 95, 80)]),
    }), { status: "not_qualified", reason: "Saturation analysis requires good-quality requested, applied and integral samples." });
  });

  test("recognizes a bounded target return even when runtime recording begins after the target step", () => {
    const api = saturationApi();
    const result = api.assessSaturationWindup({
      outputMinimum: 0, outputMaximum: 100,
      requestedOutputId: "requested-power", integralStateId: "stored-correction",
      recording: completeRecording([
        sample(0.1, 45, 105, 100, 80), sample(0.2, 45, 115, 100, 85),
        sample(0.3, 45, 125, 100, 90), sample(0.4, 35, 115, 100, 90),
        sample(0.5, 35, 105, 100, 90), sample(0.6, 35, 95, 95, 90),
      ]),
    });
    assert.strictEqual(result.status, "qualified");
    if (result.status === "qualified") {
      assert.strictEqual(result.targetReturnAtSeconds, 0.4);
      assert.strictEqual(result.releasedAtSeconds, 0.6);
    }
  });
});

function saturationApi(): any {
  try { return require("../../controlStudio/saturationWindup"); } catch { return {}; }
}

function completeRecording(rawSamples: any[]) {
  return { rawSamples, evidence: {
    method: "runtime_scan_buffer", completeness: "complete", observedSampleCount: rawSamples.length,
    missingScanRanges: [], droppedObservedSamples: 0, actualCycleTimingAvailable: true,
    freshnessAvailable: true, reason: "Complete recording — runtime captured every scan.",
  } };
}

function sample(elapsedSeconds: number, sp: number, requested: number, applied: number, integral: number,
  quality: "good" | "bad" = "good") {
  return {
    elapsedSeconds, scan: Math.round(elapsedSeconds * 10), sp, pv: sp, requestedMv: null, appliedMv: applied,
    quality, additional: [
      { id: "requested-power", channel: "controller", role: "state", unit: "%", value: requested, quality },
      { id: "stored-correction", channel: "controller", role: "state", unit: "%", value: integral, quality },
    ],
  };
}
