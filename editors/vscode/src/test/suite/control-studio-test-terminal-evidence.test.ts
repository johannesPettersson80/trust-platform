import * as assert from "assert";

suite("Control Studio terminal Test evidence", () => {
  test("retains the exact terminal runtime record and its identification for a completed Test", async () => {
    const api = terminalEvidenceApi();
    assert.strictEqual(typeof api.buildTerminalTestEvidence, "function",
      "a completed Test must hand its actual runtime record to identification");
    let identified: any;
    const payload = await api.buildTerminalTestEvidence({
      loop: { id: "loop:terminal" },
      plan: { id: "test:saturation", kind: "project_sequence" },
      result: { experiment_id: "test:saturation", status: "completed", stop_reason: "duration_complete", restoration: "restored" },
      stopRecording: async () => recording(),
      recordIdentification: (_plan: any, terminal: any, samples: any[], evidence: any) => {
        identified = { terminal, samples, evidence };
        return { experimentId: "test:saturation", role: "estimation", qualification: "qualified" };
      },
    });

    assert.deepStrictEqual(identified.terminal, {
      experiment_id: "test:saturation", status: "completed", stop_reason: "duration_complete", restoration: "restored",
    });
    assert.strictEqual(identified.samples.length, 7, "the result must retain the actual runtime scan count");
    assert.strictEqual(payload.recordingEvidence.observedSampleCount, 7);
    assert.deepStrictEqual(payload.recording.rawSamples, recording().rawSamples,
      "the display must receive the same native samples, without substituting editor observations");
  });
});

function terminalEvidenceApi(): any {
  try { return require("../../controlStudio/testTerminalEvidence"); } catch { return {}; }
}

function recording(): any {
  return {
    rawSamples: [
      sample(0, 35, 75, 75, 75), sample(0.1, 45, 105, 100, 80), sample(0.2, 45, 115, 100, 85),
      sample(0.3, 45, 125, 100, 90), sample(0.4, 35, 115, 100, 90), sample(0.5, 35, 105, 100, 90),
      sample(0.6, 35, 95, 95, 90),
    ],
    samples: [],
    evidence: { method: "runtime_scan_buffer", completeness: "complete", observedSampleCount: 7,
      missingScanRanges: [], droppedObservedSamples: 0, actualCycleTimingAvailable: true,
      freshnessAvailable: true, reason: "Complete recording — runtime captured every scan." },
  };
}

function sample(elapsedSeconds: number, sp: number, requested: number, applied: number, integral: number) {
  return { elapsedSeconds, scan: Math.round(elapsedSeconds * 10), sp, pv: sp, requestedMv: null, appliedMv: applied,
    quality: "good", units: { sp: "degC", pv: "degC", mv: "%" }, outputLimits: { minimum: 0, maximum: 100 },
    rateLimits: {}, source: "Simulation", loopRevision: "sha256:saturation", controllerMode: "Auto", safetyState: "Available", events: [],
    additional: [
      { id: "requested-power", channel: "requested-output", role: "state", unit: "%", value: requested, quality: "good" },
      { id: "stored-correction", channel: "retained-integral", role: "state", unit: "%", value: integral, quality: "good" },
    ] };
}
