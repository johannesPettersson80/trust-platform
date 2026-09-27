import * as assert from "assert";

suite("Control Studio anti-windup isolated campaign", () => {
  test("runs the exact reviewed baseline and tracked candidate through one immutable isolated scenario", async () => {
    const api = campaignApi();
    assert.strictEqual(typeof api.runAntiWindupIsolatedCampaign, "function",
      "Review needs a campaign owner that executes the matched actual-ST comparison");
    const requests: any[] = [];
    const result = await api.runAntiWindupIsolatedCampaign({ loop: loop(), candidate: candidate(), saturationWindup: finding(),
      verify: async (request: any) => { requests.push(request); return { projectRevision: "project", request, run: { id: String(request.parameter_overrides[3].value), evidence: "ST-executed" }, evaluation: {} }; },
      toEvidence: (_loop: any, run: any) => ({ trackingGain: Number(run.id), conditions: conditions(), assessment: finding(), oscillationDetected: false }),
      compare: (input: any) => ({ status: "qualified", recommendation: "recommend_tracking", compared: [input.baseline.trackingGain, input.candidate.trackingGain] }),
    });
    assert.deepStrictEqual(requests.map(request => request.parameter_overrides[3].value), [0, 0.25]);
    assert.deepStrictEqual(requests[0].initial_values, requests[1].initial_values);
    assert.deepStrictEqual(requests[0].events, requests[1].events);
    assert.deepStrictEqual(result.comparison, { status: "qualified", recommendation: "recommend_tracking", compared: [0, 0.25] });
  });

  test("does not compare runs when their actual-ST project revision changes", async () => {
    const api = campaignApi();
    let index = 0;
    await assert.rejects(() => api.runAntiWindupIsolatedCampaign({ loop: loop(), candidate: candidate(), saturationWindup: finding(),
      verify: async (request: any) => ({ projectRevision: index++ === 0 ? "project-a" : "project-b", request, run: {}, evaluation: {} }),
    }), /project changed/i);
  });
});

function campaignApi(): any { try { return require("../../controlStudio/antiWindupIsolatedCampaign"); } catch { return {}; } }
function candidate() { const values = { "Main.Kp": 2, "Main.Ki": 0.5, "Main.Kd": 0, "Main.BackCalculationGain": 0.25 };
  return { trackingTimeSeconds: 4, backCalculationGain: 0.25, values, review: { observed: { values: { ...values, "Main.BackCalculationGain": 0 }, program_revision: "program", binding_revision: "binding", active_revision: "base", status: "idle", previous_revision: null, previous_values: null, reason: null }, values } }; }
function loop(): any { return { id: "loop:anti", revision: "sha256:loop-v1", limits: { outputMinimum: 0, outputMaximum: 100 },
  controller: { saturationEvidence: { requestedOutputId: "requested", integralStateId: "integral" }, antiWindupVerification: { stepMs: 100, durationMs: 4000, seed: 7, wallBudgetMs: 10000,
    initialValues: [{ symbol: "Main.Manual", value: 0 }, { symbol: "Main.SP", value: 35 }, { symbol: "Main.Integral", value: 75 }], events: [{ atMs: 100, symbol: "Main.SP", value: 50 }, { atMs: 2000, symbol: "Main.SP", value: 35 }] }, law: {
      kind: "pid", parameterRecordSymbol: "Main.Parameters", parameterRevisionSymbol: "Main.ParameterRevision", proportionalGainSymbol: "Main.Kp", integralGainSymbol: "Main.Ki", derivativeGainSymbol: "Main.Kd", antiWindup: { method: "back-calculation", trackingGainSymbol: "Main.BackCalculationGain" } } },
  parameterBinding: { loop_id: "loop:anti", task_name: "MainTask", mode: { symbol: "Main.Manual", manual_value: true }, parameters: [
    parameter("Main.Kp", "%/degC"), parameter("Main.Ki", "%/(degC*s)"), parameter("Main.Kd", "%*s/degC"), parameter("Main.BackCalculationGain", "1/s") ] },
  signals: { setpoint: signal("Main.SP", "degC"), processVariable: signal("Main.PV", "degC"), manipulatedVariable: signal("Main.MV", "%"), additional: [{ id: "requested", symbol: "Main.RequestedOutput", unit: "%", role: "state" }, { id: "integral", symbol: "Main.Integral", unit: "%", role: "state" }] } }; }
function parameter(symbol: string, unit: string) { return { symbol, iec_type: "LREAL", unit, convention: "gain", minimum: 0, maximum: 100 }; }
function signal(symbol: string, unit: string) { return { symbol, unit, range: { minimum: 0, maximum: 100 } }; }
function conditions() { return { loopId: "loop:anti", loopRevision: "sha256:loop-v1", source: "simulation", scenarioDigest: "scenario", plantInitialConditionDigest: "plant", controllerInitialConditionDigest: "controller", samplePeriodMs: 100, durationMs: 4000, outputMinimum: 0, outputMaximum: 100 }; }
function finding() { return { status: "qualified", limit: "upper", enteredAtSeconds: 0.1, targetReturnAtSeconds: 2, releasedAtSeconds: 3, recoverySeconds: 1, peakRequestedOutput: 125, peakAppliedOutput: 100, maximumMismatch: 25, integralAtEntry: 75, integralAtTargetReturn: 90, integralAtRelease: 80, integralChangeDuringSaturation: 15, recommendation: "Compare." }; }
