import * as assert from "assert";

suite("Control Studio isolated anti-windup recorded run", () => {
  test("maps only actual ST channels into qualified saturation evidence and immutable comparison conditions", () => {
    const api = recordedRunApi();
    assert.strictEqual(typeof api.antiWindupRunFromRecordedSt, "function",
      "the isolated result needs a strict runtime-row adapter before comparison can use it");
    const result = api.antiWindupRunFromRecordedSt(loop(), run());
    assert.strictEqual(result.trackingGain, 0);
    assert.strictEqual(result.assessment.status, "qualified");
    assert.strictEqual(result.conditions.loopId, "loop:anti");
    assert.strictEqual(result.conditions.source, "simulation");
    assert.ok(result.conditions.scenarioDigest.startsWith("sha256:"));
    assert.strictEqual(result.oscillationDetected, false);
  });

  test("rejects non-ST evidence, missing unlimited request and bad-quality observations", () => {
    const api = recordedRunApi();
    assert.throws(() => api.antiWindupRunFromRecordedSt(loop(), { ...run(), evidence: "model-predicted" }), /actual ST/i);
    const missing = run(); delete missing.samples[1].values["Main.RequestedOutput"];
    assert.throws(() => api.antiWindupRunFromRecordedSt(loop(), missing), /requested-output/i);
    const bad = run(); bad.samples[2].values["Main.Integral"].quality = "bad";
    assert.throws(() => api.antiWindupRunFromRecordedSt(loop(), bad), /good-quality/i);
  });
});

function recordedRunApi(): any { try { return require("../../controlStudio/antiWindupRecordedRun"); } catch { return {}; } }
function loop(): any {
  return { id: "loop:anti", revision: "sha256:loop-v1", limits: { outputMinimum: 0, outputMaximum: 100 },
    controller: { saturationEvidence: { requestedOutputId: "requested", integralStateId: "integral" }, law: {
      kind: "pid", parameterRecordSymbol: "Main.Parameters", parameterRevisionSymbol: "Main.ParameterRevision",
      proportionalGainSymbol: "Main.Kp", integralGainSymbol: "Main.Ki", derivativeGainSymbol: "Main.Kd",
      antiWindup: { method: "back-calculation", trackingGainSymbol: "Main.BackCalculationGain" } } },
    parameterBinding: { loop_id: "loop:anti", task_name: "MainTask", mode: { symbol: "Main.Manual", manual_value: true }, parameters: [
      parameter("Main.Kp", "%/degC"), parameter("Main.Ki", "%/(degC*s)"), parameter("Main.Kd", "%*s/degC"), parameter("Main.BackCalculationGain", "1/s"),
    ] }, signals: { setpoint: signal("Main.SP", "degC"), processVariable: signal("Main.PV", "degC"), manipulatedVariable: signal("Main.MV", "%"),
      additional: [{ id: "requested", symbol: "Main.RequestedOutput", unit: "%", role: "state" }, { id: "integral", symbol: "Main.Integral", unit: "%", role: "state" }] } };
}
function parameter(symbol: string, unit: string) { return { symbol, iec_type: "LREAL", unit, convention: "gain", minimum: 0, maximum: 100 }; }
function signal(symbol: string, unit: string) { return { symbol, unit, range: { minimum: 0, maximum: 100 } }; }
function run(): any {
  const row = (timeSeconds: number, sp: number, pv: number, mv: number, requested: number, integral: number) => ({ timeSeconds,
    sequence: Math.round(timeSeconds * 10), values: {
      "Main.SP": value(sp), "Main.PV": value(pv), "Main.MV": value(mv),
      "Main.RequestedOutput": value(requested), "Main.Integral": value(integral),
    } });
  return { id: "run:baseline", evidence: "ST-executed", parameters: { "Main.Kp": 2, "Main.Ki": 0.5, "Main.Kd": 0, "Main.BackCalculationGain": 0 },
    conditions: { initialState: { "Main.Manual": 0, "Main.SP": 35, "Main.Integral": 75 }, events: [{ atSeconds: 0.1, channel: "Main.SP", value: 50 }, { atSeconds: 2, channel: "Main.SP", value: 35 }],
      durationSeconds: 4, sampleTimesSeconds: [0, 0.1, 0.2, 2, 2.1, 2.2, 3], seed: 7, modelRevision: "model", programRevision: "program", parameterSchemaRevision: "schema" },
    channels: ["Main.SP", "Main.PV", "Main.MV", "Main.RequestedOutput", "Main.Integral"].map(symbol => ({ id: symbol, unit: symbol.includes("SP") || symbol.includes("PV") ? "degC" : "%" })),
    samples: [row(0, 35, 35, 75, 75, 75), row(0.1, 50, 35, 100, 105, 76), row(0.2, 50, 36, 100, 110, 77),
      row(2, 35, 42, 100, 110, 90), row(2.1, 35, 41, 100, 105, 89), row(2.2, 35, 40, 95, 95, 88), row(3, 35, 35, 75, 75, 80)] };
}
function value(value: number) { return { value, quality: "good" }; }
