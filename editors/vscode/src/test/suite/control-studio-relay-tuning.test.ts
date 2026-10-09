import * as assert from "assert";

suite("Control Studio relay tuning", () => {
  test("excludes baseline and the incomplete initial relay cycle from repeatability", () => {
    const samples = [
      [0, 0, 0], [0.1, 0, 1], [0.3, 0.2, -1], [0.7, -0.2, 1],
      [1.1, 0.2, -1], [1.5, -0.2, 1], [1.9, 0.2, -1], [2.3, -0.2, 1],
      [2.7, 0.2, -1], [3.1, -0.2, 1], [3.2, 0, 0],
    ].map(([timeSeconds, processValue, output]) => ({ timeSeconds, processValue, output }));
    const result = relayApi().relayPiFromMeasuredRecord({ highOutput: 1, lowOutput: -1, samples });
    assert.strictEqual(result.status, "qualified", "recording startup and restoration are not complete relay periods");
    assert.strictEqual(result.cycles.length, 3);
    assert.ok(result.cycles.every((cycle: { periodSeconds: number }) => Math.abs(cycle.periodSeconds - 0.8) < 1e-12));
  });

  test("reports intermediate actuator values instead of missing relay transitions", () => {
    const samples = [-1, 0, 1, 0, -1, 0, 1, 0, -1, 0, 1].map((output, index) => ({
      timeSeconds: index * 0.1, output, processValue: Math.sin(index),
    }));
    assert.throws(() => relayApi().relayPiFromMeasuredRecord({ highOutput: 1, lowOutput: -1, samples }),
      /clipped or intermediate output values/, "Ramped relay cycles must not be treated as an ideal two-level relay");
  });

  test("calculates an unapplied PI candidate only from qualified feedback cycles", () => {
    const api = relayApi();
    assert.strictEqual(typeof api.calculateRelayPiCandidate, "function",
      "relay tuning needs a qualified-cycle calculation rather than a time-driven output guess");
    const result = api.calculateRelayPiCandidate({
      highOutput: 80,
      lowOutput: 70,
      cycles: [
        { peak: 35.2, trough: 34.8, periodSeconds: 4 },
        { peak: 35.21, trough: 34.81, periodSeconds: 4.1 },
        { peak: 35.19, trough: 34.79, periodSeconds: 3.9 },
      ],
    });
    assert.strictEqual(result.status, "qualified");
    assert.ok(Math.abs(result.ultimateGain - 31.8309886) < 1e-5);
    assert.ok(Math.abs(result.periodSeconds - 4) < 1e-9);
    assert.ok(Math.abs(result.candidate.proportionalGain - 14.3239449) < 1e-5);
    assert.ok(Math.abs(result.candidate.integralTimeSeconds - 3.3333333) < 1e-5);
    assert.ok(Math.abs(result.candidate.integralGain - 4.2971835) < 1e-5);
    assert.strictEqual(result.candidate.applied, false);
    assert.deepStrictEqual(api.calculateRelayPiCandidate({ highOutput: 80, lowOutput: 70,
      cycles: [{ peak: 35.2, trough: 34.8, periodSeconds: 4 }] }), {
      status: "not_qualified",
      reason: "At least three complete feedback relay cycles are required.",
    });
  });

  test("rejects nonrepeatable relay amplitudes and periods before offering a candidate", () => {
    const api = relayApi();
    for (const variant of ["amplitude", "period"] as const) {
      const cycles = [1, 1, 2].map(factor => ({
        peak: variant === "amplitude" ? factor : 1,
        trough: 0,
        periodSeconds: variant === "period" ? factor : 1,
      }));
      const result = api.calculateRelayPiCandidate({ highOutput: 80, lowOutput: 70, cycles });
      assert.strictEqual(result.status, "not_qualified", `${variant} variation must block the candidate`);
      assert.match(result.reason, new RegExp(variant, "i"));
      assert.strictEqual("candidate" in result, false);
    }
  });

  test("extracts relay cycles only from measured feedback-driven output transitions", () => {
    const api = relayApi();
    assert.strictEqual(typeof api.extractRelayCycles, "function");
    const cycles = api.extractRelayCycles({ highOutput: 80, lowOutput: 70, samples: [
      { timeSeconds: 0, processValue: 35.0, output: 80 },
      { timeSeconds: 1, processValue: 35.2, output: 70 },
      { timeSeconds: 2, processValue: 34.8, output: 80 },
      { timeSeconds: 3, processValue: 35.21, output: 70 },
      { timeSeconds: 4, processValue: 34.79, output: 80 },
      { timeSeconds: 5, processValue: 35.19, output: 70 },
      { timeSeconds: 6, processValue: 34.81, output: 80 },
      { timeSeconds: 7, processValue: 35.22, output: 70 },
      { timeSeconds: 8, processValue: 34.78, output: 80 },
    ] });
    assert.deepStrictEqual(cycles, [
      { peak: 35.21, trough: 34.79, periodSeconds: 2 },
      { peak: 35.19, trough: 34.79, periodSeconds: 2 },
      { peak: 35.22, trough: 34.78, periodSeconds: 2 },
    ]);
    assert.throws(() => api.extractRelayCycles({ highOutput: 80, lowOutput: 70, samples: [
      { timeSeconds: 0, processValue: 35, output: 80 }, { timeSeconds: 1, processValue: 35.1, output: 80 },
      { timeSeconds: 2, processValue: 35.2, output: 80 },
    ] }), /transitions/i);
  });

  test("builds an unapplied relay PI candidate only after measured cycles qualify", () => {
    const api = relayApi();
    assert.strictEqual(typeof api.relayPiFromMeasuredRecord, "function");
    const result = api.relayPiFromMeasuredRecord({ highOutput: 80, lowOutput: 70, samples: [
      { timeSeconds: 0, processValue: 35.0, output: 80 }, { timeSeconds: 1, processValue: 35.2, output: 70 },
      { timeSeconds: 2, processValue: 34.8, output: 80 }, { timeSeconds: 3, processValue: 35.21, output: 70 },
      { timeSeconds: 4, processValue: 34.79, output: 80 }, { timeSeconds: 5, processValue: 35.19, output: 70 },
      { timeSeconds: 6, processValue: 34.81, output: 80 }, { timeSeconds: 7, processValue: 35.22, output: 70 },
      { timeSeconds: 8, processValue: 34.78, output: 80 },
    ] });
    assert.strictEqual(result.status, "qualified");
    assert.strictEqual(result.candidate.applied, false);
    assert.strictEqual(result.cycles.length, 3);
  });

});

function relayApi(): any {
  try { return require("../../controlStudio/relayTuning"); } catch { return {}; }
}
