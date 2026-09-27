import * as assert from "assert";

suite("Control Studio anti-windup panel workflow", () => {
  test("retains one reviewed simulation workflow through Stage and Restore", async () => {
    const api = panelWorkflowApi();
    assert.strictEqual(typeof api.executeAntiWindupCommand, "function",
      "panel command handling needs an owned anti-windup workflow boundary");
    const connect = async () => ({ source: "simulation", client: client() });
    const review = await api.executeAntiWindupCommand({ command: { action: "review", trackingTimeSeconds: 4 },
      loop: loop(), saturationWindup: evidence(), connect });
    assert.strictEqual(review.result.status, "reviewed");
    const stage = await api.executeAntiWindupCommand({ command: { action: "stage" }, loop: loop(),
      workflow: review.workflow, connect });
    assert.strictEqual(stage.result.status, "staged");
    const rollback = await api.executeAntiWindupCommand({ command: { action: "rollback" }, loop: loop(),
      workflow: stage.workflow, connect });
    assert.strictEqual(rollback.result.status, "rolled-back");
    assert.strictEqual(rollback.workflow, undefined);
  });

  test("does not create workflow authority from incomplete terminal evidence", async () => {
    const api = panelWorkflowApi();
    let connects = 0;
    await assert.rejects(() => api.executeAntiWindupCommand({ command: { action: "review", trackingTimeSeconds: 4 },
      loop: loop(), saturationWindup: { status: "not_qualified", reason: "missing samples" },
      connect: async () => { connects++; return { source: "simulation", client: client() }; } }), /qualified saturation-return/i);
    assert.strictEqual(connects, 0);
  });

  test("keeps candidate authority in the current loop and clears it when the panel resets", async () => {
    const api = panelWorkflowApi();
    assert.strictEqual(typeof api.AntiWindupPanelSession, "function",
      "one panel-scoped owner must retain terminal saturation evidence and the staged candidate together");
    const session = new api.AntiWindupPanelSession();
    const terminal: any = session.augmentTerminalPayload(loop(), { saturationWindup: evidence() });
    assert.deepStrictEqual(terminal.antiWindup, { available: true });
    await session.execute({ action: "review", trackingTimeSeconds: 4 }, loop(), async () => ({ source: "simulation", client: client() }));
    session.reset();
    await assert.rejects(() => session.execute({ action: "stage" }, loop(), async () => ({ source: "simulation", client: client() })),
      /review one anti-windup candidate/i);
  });

  test("launches the isolated comparison only from its reviewed candidate and qualified terminal finding", async () => {
    const api = panelWorkflowApi();
    const session = new api.AntiWindupPanelSession();
    session.augmentTerminalPayload(loop(), { saturationWindup: evidence() });
    await session.execute({ action: "review", trackingTimeSeconds: 4 }, loop(), async () => ({ source: "simulation", client: client() }));
    let called: any;
    const result = await session.verify(loop(), async () => { throw new Error("campaign test supplies its own runner"); }, async (input: any) => {
      called = input;
      return { baseline: { projectRevision: "project" }, candidate: { projectRevision: "project" }, comparison: { status: "qualified", recommendation: "recommend_tracking" } };
    });
    assert.strictEqual(called.candidate.backCalculationGain, 0.25);
    assert.strictEqual(called.saturationWindup.status, "qualified");
    assert.strictEqual(result.comparison.recommendation, "recommend_tracking");
  });
});

function panelWorkflowApi(): any {
  try { return require("../../controlStudio/antiWindupPanelWorkflow"); } catch { return {}; }
}

function evidence() {
  return { status: "qualified", limit: "upper", enteredAtSeconds: 1, targetReturnAtSeconds: 3, releasedAtSeconds: 5,
    recoverySeconds: 2, peakRequestedOutput: 120, peakAppliedOutput: 100, maximumMismatch: 20,
    integralAtEntry: 40, integralAtTargetReturn: 70, integralAtRelease: 55, integralChangeDuringSaturation: 30,
    recommendation: "Compare a tracked candidate." };
}

function client() {
  let phase = "baseline";
  const baseline = { "Main.Kp": 2, "Main.Ki": 0.5, "Main.Kd": 0, "Main.BackCalculationGain": 0 };
  const candidate = { ...baseline, "Main.BackCalculationGain": 0.25 };
  const status = () => ({ values: phase === "baseline" ? baseline : candidate, program_revision: "program",
    binding_revision: "binding", active_revision: phase === "baseline" ? "base" : "tracked", status: "applied",
    previous_revision: phase === "baseline" ? null : "base", previous_values: phase === "baseline" ? null : baseline, reason: null });
  return { status: async () => status(), review: async (values: Record<string, number>) => ({ observed: status(), values }),
    apply: async () => { phase = "tracked"; return status(); }, reviewRollback: async () => ({ observed: status(), values: baseline }),
    rollback: async () => { phase = "baseline"; return status(); } };
}

function loop(): any {
  return { id: "loop:anti-windup", revision: "loop-revision", parameterBinding: { loop_id: "loop:anti-windup", task_name: "MainTask",
    mode: { symbol: "Main.Manual", manual_value: true }, parameters: [
      { symbol: "Main.Kp", iec_type: "LREAL", unit: "V/m", convention: "proportional gain", minimum: 0, maximum: 100 },
      { symbol: "Main.Ki", iec_type: "LREAL", unit: "V/(m*s)", convention: "integral gain", minimum: 0, maximum: 100 },
      { symbol: "Main.Kd", iec_type: "LREAL", unit: "V*s/m", convention: "derivative gain", minimum: 0, maximum: 100 },
      { symbol: "Main.BackCalculationGain", iec_type: "LREAL", unit: "1/s", convention: "back-calculation gain", minimum: 0, maximum: 100 },
    ] }, controller: { law: { kind: "pid", parameterRecordSymbol: "Main.Parameters", parameterRevisionSymbol: "Main.ParameterRevision",
      proportionalGainSymbol: "Main.Kp", integralGainSymbol: "Main.Ki", derivativeGainSymbol: "Main.Kd",
      antiWindup: { method: "back-calculation", trackingGainSymbol: "Main.BackCalculationGain" } } } };
}
