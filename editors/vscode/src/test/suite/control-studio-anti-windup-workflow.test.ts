import * as assert from "assert";

suite("Control Studio anti-windup workflow", () => {
  test("permits a simulation candidate only from qualified saturation-return evidence", async () => {
    const api = workflowApi();
    assert.strictEqual(typeof api.AntiWindupWorkflow, "function",
      "the panel needs one owner that joins qualified saturation evidence to the typed parameter campaign");

    let connects = 0;
    const workflow = new api.AntiWindupWorkflow(loop(), async () => {
      connects++;
      return { source: "simulation", client: client() };
    });
    await assert.rejects(() => workflow.review(4, { status: "not_qualified", reason: "incomplete runtime record" }),
      /qualified saturation-return/i);
    assert.strictEqual(connects, 0, "an incomplete record must not even open a parameter transaction");

    const reviewed = await workflow.review(4, qualifiedEvidence());
    assert.strictEqual(reviewed.status, "reviewed");
    assert.strictEqual(reviewed.candidate.backCalculationGain, 0.25);
    assert.strictEqual(connects, 1);
    assert.strictEqual((await workflow.stage()).status, "staged");
    assert.strictEqual((await workflow.rollback()).status, "rolled-back");
  });

  test("rejects physical staging and a stage request without a reviewed candidate", async () => {
    const api = workflowApi();
    const physical = new api.AntiWindupWorkflow(loop(), async () => ({ source: "physical", client: client() }));
    await assert.rejects(() => physical.review(4, qualifiedEvidence()), /simulation path/i);

    const simulation = new api.AntiWindupWorkflow(loop(), async () => ({ source: "simulation", client: client() }));
    await assert.rejects(() => simulation.stage(), /review one anti-windup candidate/i);
    await assert.rejects(() => simulation.rollback(), /staged anti-windup candidate/i);
  });

  test("requires the declared back-calculation adapter before exposing the candidate path", () => {
    const api = workflowApi();
    assert.deepStrictEqual(api.antiWindupAvailability(loop(), qualifiedEvidence()), { available: true });
    const missingAdapter = loop();
    delete missingAdapter.controller.law.antiWindup;
    assert.deepStrictEqual(api.antiWindupAvailability(missingAdapter, qualifiedEvidence()), {
      available: false,
      reason: "A typed back-calculation anti-windup adapter is required.",
    });
  });
});

function workflowApi(): any {
  try { return require("../../controlStudio/antiWindupWorkflow"); } catch { return {}; }
}

function qualifiedEvidence() {
  return {
    status: "qualified", limit: "upper", enteredAtSeconds: 1, targetReturnAtSeconds: 3, releasedAtSeconds: 5,
    recoverySeconds: 2, peakRequestedOutput: 120, peakAppliedOutput: 100, maximumMismatch: 20,
    integralAtEntry: 40, integralAtTargetReturn: 70, integralAtRelease: 55, integralChangeDuringSaturation: 30,
    recommendation: "Compare a tracked candidate.",
  };
}

function client() {
  let phase = "baseline";
  const baseline = { "Main.Kp": 2, "Main.Ki": 0.5, "Main.Kd": 0, "Main.BackCalculationGain": 0 };
  const candidate = { ...baseline, "Main.BackCalculationGain": 0.25 };
  const status = () => ({ values: phase === "baseline" ? baseline : candidate,
    program_revision: "program", binding_revision: "binding", active_revision: phase === "baseline" ? "base" : "tracked",
    status: "applied", previous_revision: phase === "baseline" ? null : "base",
    previous_values: phase === "baseline" ? null : baseline, reason: null });
  return {
    status: async () => status(),
    review: async (values: Record<string, number>) => ({ observed: status(), values }),
    apply: async () => { phase = "tracked"; return status(); },
    reviewRollback: async () => ({ observed: status(), values: baseline }),
    rollback: async () => { phase = "baseline"; return status(); },
  };
}

function loop(): any {
  return { id: "loop:anti-windup", revision: "loop-revision", parameterBinding: {
    loop_id: "loop:anti-windup", task_name: "MainTask", mode: { symbol: "Main.Manual", manual_value: true }, parameters: [
      { symbol: "Main.Kp", iec_type: "LREAL", unit: "V/m", convention: "proportional gain", minimum: 0, maximum: 100 },
      { symbol: "Main.Ki", iec_type: "LREAL", unit: "V/(m*s)", convention: "integral gain", minimum: 0, maximum: 100 },
      { symbol: "Main.Kd", iec_type: "LREAL", unit: "V*s/m", convention: "derivative gain", minimum: 0, maximum: 100 },
      { symbol: "Main.BackCalculationGain", iec_type: "LREAL", unit: "1/s", convention: "back-calculation gain", minimum: 0, maximum: 100 },
    ]
  }, controller: { law: { kind: "pid", parameterRecordSymbol: "Main.Parameters", parameterRevisionSymbol: "Main.ParameterRevision",
    proportionalGainSymbol: "Main.Kp", integralGainSymbol: "Main.Ki", derivativeGainSymbol: "Main.Kd",
    antiWindup: { method: "back-calculation", trackingGainSymbol: "Main.BackCalculationGain" },
  } } };
}
