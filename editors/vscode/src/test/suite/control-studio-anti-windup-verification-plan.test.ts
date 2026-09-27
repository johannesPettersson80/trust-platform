import * as assert from "assert";

suite("Control Studio anti-windup isolated verification plan", () => {
  test("freezes identical actual-ST conditions and changes only the declared tracking gain", () => {
    const api = verificationPlanApi();
    assert.strictEqual(typeof api.buildAntiWindupVerificationPlan, "function",
      "the matched candidate needs one isolated-ST plan builder rather than two look-alike Test requests");
    const plan = api.buildAntiWindupVerificationPlan({ loop: loop(), baselineValues: baseline(), trackingTimeSeconds: 4,
      scenario: scenario(), saturationWindup: evidence() });
    assert.deepStrictEqual(plan.baseline.parameter_overrides.map((value: any) => [value.symbol, value.value]), [
      ["Main.Kp", 2], ["Main.Ki", 0.5], ["Main.Kd", 0], ["Main.BackCalculationGain", 0],
    ]);
    assert.deepStrictEqual(plan.candidate.parameter_overrides.map((value: any) => [value.symbol, value.value]), [
      ["Main.Kp", 2], ["Main.Ki", 0.5], ["Main.Kd", 0], ["Main.BackCalculationGain", 0.25],
    ]);
    for (const key of ["step_ms", "duration_ms", "seed", "max_total_cycles", "wall_budget_ms", "initial_values", "events", "observations"]) {
      assert.deepStrictEqual(plan.baseline[key], plan.candidate[key], `${key} must be frozen across the two actual-ST runs`);
    }
    assert.deepStrictEqual(plan.baseline.observations.map((value: any) => value.symbol), [
      "Main.SP", "Main.PV", "Main.MV", "Main.RequestedOutput", "Main.Integral",
    ]);
    assert.deepStrictEqual(plan.requirements, [{ id: "applied-output-bounds", kind: "bounds", channel: "Main.MV",
      window: { start: 0, end: 4 }, minimum: 0, maximum: 100 }]);
  });

  test("rejects an unqualified saturation record or an undeclared typed adapter", () => {
    const api = verificationPlanApi();
    assert.throws(() => api.buildAntiWindupVerificationPlan({ loop: loop(), baselineValues: baseline(), trackingTimeSeconds: 4,
      scenario: scenario(), saturationWindup: { status: "not_qualified", reason: "incomplete" } }), /qualified saturation-return/i);
    const missing = loop(); delete missing.controller.law.antiWindup;
    assert.throws(() => api.buildAntiWindupVerificationPlan({ loop: missing, baselineValues: baseline(), trackingTimeSeconds: 4,
      scenario: scenario(), saturationWindup: evidence() }), /typed back-calculation/i);
    assert.throws(() => api.buildAntiWindupVerificationPlan({ loop: loop(), baselineValues: baseline(), trackingTimeSeconds: 4,
      scenario: { ...scenario(), wallBudgetMs: 10_001 }, saturationWindup: evidence() }), /per-run wall budget/i);
  });

  test("uses the loop's declared immutable scenario when Control Studio runs the normal automatic route", () => {
    const api = verificationPlanApi();
    const configured = loop();
    configured.controller.antiWindupVerification = declaredScenario();
    const plan = api.buildAntiWindupVerificationPlan({ loop: configured, baselineValues: baseline(), trackingTimeSeconds: 4,
      saturationWindup: evidence() });
    assert.deepStrictEqual(plan.baseline.initial_values, { "Main.Manual": 0, "Main.SP": 35, "Main.Integral": 75 });
    assert.deepStrictEqual(plan.candidate.events, [{ at_ms: 100, symbol: "Main.SP", value: 45 }, { at_ms: 2000, symbol: "Main.SP", value: 35 }]);
  });
});

function verificationPlanApi(): any {
  try { return require("../../controlStudio/antiWindupVerificationPlan"); } catch { return {}; }
}

function baseline() { return { "Main.Kp": 2, "Main.Ki": 0.5, "Main.Kd": 0, "Main.BackCalculationGain": 0 }; }
function scenario() { return { stepMs: 100, durationMs: 4000, seed: 7, wallBudgetMs: 10000,
  initialValues: { "Main.Manual": 0, "Main.SP": 0, "Main.Integral": 0 },
  events: [{ atMs: 100, symbol: "Main.SP", value: 45 }, { atMs: 2000, symbol: "Main.SP", value: 35 }] }; }
function declaredScenario() { return { stepMs: 100, durationMs: 4000, seed: 7, wallBudgetMs: 10000,
  initialValues: [{ symbol: "Main.Manual", value: 0 }, { symbol: "Main.SP", value: 35 }, { symbol: "Main.Integral", value: 75 }],
  events: [{ atMs: 100, symbol: "Main.SP", value: 45 }, { atMs: 2000, symbol: "Main.SP", value: 35 }] }; }
function evidence() { return { status: "qualified", limit: "upper", enteredAtSeconds: 0.1, targetReturnAtSeconds: 2,
  releasedAtSeconds: 3, recoverySeconds: 1, peakRequestedOutput: 125, peakAppliedOutput: 100, maximumMismatch: 25,
  integralAtEntry: 75, integralAtTargetReturn: 90, integralAtRelease: 85, integralChangeDuringSaturation: 15,
  recommendation: "Compare a tracked candidate." }; }
function loop(): any {
  return { id: "loop:anti-windup", revision: "loop-v1", limits: { outputMinimum: 0, outputMaximum: 100 },
    parameterBinding: { loop_id: "loop:anti-windup", task_name: "MainTask", mode: { symbol: "Main.Manual", manual_value: true }, parameters: [
      { symbol: "Main.Kp", iec_type: "LREAL", unit: "%/degC", convention: "proportional gain", minimum: 0, maximum: 100 },
      { symbol: "Main.Ki", iec_type: "LREAL", unit: "%/(degC*s)", convention: "integral gain", minimum: 0, maximum: 100 },
      { symbol: "Main.Kd", iec_type: "LREAL", unit: "%*s/degC", convention: "derivative gain", minimum: 0, maximum: 100 },
      { symbol: "Main.BackCalculationGain", iec_type: "LREAL", unit: "1/s", convention: "back-calculation gain", minimum: 0, maximum: 100 },
    ] }, controller: { saturationEvidence: { requestedOutputId: "requested", integralStateId: "integral" }, law: {
      kind: "pid", parameterRecordSymbol: "Main.Parameters", parameterRevisionSymbol: "Main.ParameterRevision",
      proportionalGainSymbol: "Main.Kp", integralGainSymbol: "Main.Ki", derivativeGainSymbol: "Main.Kd",
      antiWindup: { method: "back-calculation", trackingGainSymbol: "Main.BackCalculationGain" },
    } }, signals: { setpoint: { symbol: "Main.SP", unit: "degC" }, processVariable: { symbol: "Main.PV", unit: "degC" },
      manipulatedVariable: { symbol: "Main.MV", unit: "%" }, additional: [
        { id: "requested", symbol: "Main.RequestedOutput", unit: "%", role: "state" },
        { id: "integral", symbol: "Main.Integral", unit: "%", role: "state" },
      ] } };
}
