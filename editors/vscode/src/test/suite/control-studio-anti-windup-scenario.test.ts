import * as assert from "assert";

suite("Control Studio legacy anti-windup verification key", () => {
  test("loads a legacy anti-windup verification key without validating or using it", () => {
    const api = loopApi();
    const configured = loop();
    for (const legacy of [scenario(), { ...scenario(), events: [{ atMs: 125, symbol: "Main.SP", value: 50 }] }, "not a scenario"]) {
      const withLegacy = { ...configured, controller: { ...configured.controller, antiWindupVerification: legacy } };
      assert.deepStrictEqual(api.validateControlLoopDefinition(withLegacy, scope()), { kind: "accepted", loop: withLegacy });
      assert.strictEqual(api.decodeControlLoopDefinition(withLegacy).kind, "accepted");
    }
  });
});

function loopApi(): any { try { return require("../../controlStudio/loopDefinition"); } catch { return {}; } }
function scope() { return { projectId: "project:anti", runtimeId: "simulator" }; }
function scenario() { return { stepMs: 100, durationMs: 6000, seed: 7, wallBudgetMs: 10000,
  initialValues: [{ symbol: "Main.Manual", value: 0 }, { symbol: "Main.SP", value: 35 }, { symbol: "Main.Integral", value: 75 }],
  events: [{ atMs: 100, symbol: "Main.SP", value: 50 }, { atMs: 2000, symbol: "Main.SP", value: 35 }] }; }
function loop(): any {
  return { schemaVersion: 2, id: "loop:anti", name: "Anti-windup", revision: "sha256:loop-v1", projectId: "project:anti", runtimeId: "simulator",
    controller: { instance: "Main", type: "Control.PID", design: "pid", revision: "sha256:controller-v1", action: "direct", taskPeriodMs: 100,
      modeSignal: "Main.Manual", modeWhenTrue: "Manual", saturationEvidence: { requestedOutputId: "requested", integralStateId: "integral" },
      law: { kind: "pid", parameterRecordSymbol: "Main.Parameters", parameterRevisionSymbol: "Main.ParameterRevision",
        proportionalGainSymbol: "Main.Kp", integralGainSymbol: "Main.Ki", derivativeGainSymbol: "Main.Kd",
        antiWindup: { method: "back-calculation", trackingGainSymbol: "Main.BackCalculationGain" } } },
    signals: { setpoint: binding("Main.SP", "%ML0", "degC", "read"), processVariable: binding("Main.PV", "%IL0", "degC", "read"),
      manipulatedVariable: binding("Main.MV", "%QL0", "%", "write"), additional: [
        auxiliary("requested", "Main.RequestedOutput", "%ML8"), auxiliary("integral", "Main.Integral", "%ML16"),
      ] }, limits: { outputMinimum: 0, outputMaximum: 100 }, observedSampleTiming: { nominalPeriodMs: 100 },
    parameterBinding: { loop_id: "loop:anti", task_name: "MainTask", mode: { symbol: "Main.Manual", manual_value: true }, parameters: [
      parameter("Main.Kp", "%/degC"), parameter("Main.Ki", "%/(degC*s)"), parameter("Main.Kd", "%*s/degC"), parameter("Main.BackCalculationGain", "1/s"),
    ] } };
}
function binding(symbol: string, address: string, unit: string, direction: string) { return { symbol, address, iecType: "LREAL", direction, unit, range: { minimum: -1000, maximum: 1000 } }; }
function auxiliary(id: string, symbol: string, address: string) { return { id, channel: id, role: "state", ...binding(symbol, address, "%", "read") }; }
function parameter(symbol: string, unit: string) { return { symbol, iec_type: "LREAL", unit, convention: "gain", minimum: 0, maximum: 100 }; }
