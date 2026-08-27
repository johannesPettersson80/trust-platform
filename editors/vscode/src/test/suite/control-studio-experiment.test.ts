import * as assert from "assert";

suite("Control Studio bounded experiment", () => {
  test("builds a concise bounded setpoint plan from current loop evidence", () => {
    const api = experimentApi();
    assert.deepStrictEqual(api.buildControlExperimentPlan({
      loop: loop(), sample: sample(), kind: "setpoint_step", amplitude: 5, durationSeconds: 30,
    }), {
      kind: "accepted",
      plan: {
        id: "experiment:loop:tank:41:setpoint_step",
        source: "simulation", kind: "setpoint_step", affectedSignal: "LevelSP", address: "%ML0",
        valueType: "LREAL",
        from: 50, to: 55, unit: "%", minimum: 0, maximum: 100, durationMs: 30_000,
        expectedScan: 41, stopOnStaleMs: 500,
        stopConditions: ["Abort", "30 s timeout", "Stale runtime data", "Signal force", "Runtime fault"],
        restorationValue: 50, requiresPhysicalApproval: false,
      },
    });
  });

  test("blocks unsafe, stale, forced, out-of-limit, and unapproved physical plans", () => {
    const api = experimentApi();
    const base = { loop: loop(), sample: sample(), kind: "setpoint_step", amplitude: 5, durationSeconds: 30 };
    assert.deepStrictEqual(api.buildControlExperimentPlan({ ...base, sample: { availability: "unavailable", reason: "stale" } }), {
      kind: "rejected", reason: "Fresh synchronized SP, PV, and MV are required.",
    });
    assert.deepStrictEqual(api.buildControlExperimentPlan({ ...base, sample: {
      ...sample(), setpoint: { ...sample().setpoint, quality: "forced" },
    } }), { kind: "rejected", reason: "The affected signal is forced." });
    assert.deepStrictEqual(api.buildControlExperimentPlan({ ...base, amplitude: 60 }), {
      kind: "rejected", reason: "The requested value 110 % is outside 0…100 %.",
    });
    assert.deepStrictEqual(api.buildControlExperimentPlan({ ...base, kind: "output_step" }), {
      kind: "rejected", reason: "Output steps require confirmed Manual controller mode.",
    });
    assert.deepStrictEqual(api.buildControlExperimentPlan({ ...base, sample: { ...sample(), source: "Physical" } }), {
      kind: "rejected", reason: "A reviewed physical topology is required before approval can be requested.",
    });
  });

  test("maps start, status, and abort to the existing runtime control request shape", async () => {
    const api = experimentApi();
    const calls: any[] = [];
    const client = new api.ControlExperimentClient(async (operation: string, params: unknown) => {
      calls.push({ operation, params });
      return { status: operation.endsWith("abort") ? "aborted" : "running", experiment_id: "experiment-1" };
    });
    const plan = { ...api.buildControlExperimentPlan({
      loop: loop(), sample: sample(), kind: "setpoint_step", amplitude: 5, durationSeconds: 30,
    }).plan, id: "experiment-1" };
    await client.start(plan, false);
    await client.status("experiment-1");
    await client.abort("experiment-1");
    assert.deepStrictEqual(calls.map((call) => call.operation), [
      "control.experiment.start", "control.experiment.status", "control.experiment.abort",
    ]);
    assert.deepStrictEqual(calls[0].params, {
      experiment_id: "experiment-1", source: "simulation", kind: "setpoint_step", address: "%ML0",
      value: 55, restoration_value: 50, value_type: "LREAL", minimum: 0, maximum: 100, duration_ms: 30_000,
      expected_scan: 41, stop_on_stale_ms: 500, controller_mode: "Auto",
    });
  });

  test("accepts only strict loop-scoped review, approval, and abort messages", () => {
    const messages = require("../../controlStudio/messages") as any;
    const expected = {
      projectId: "project:fixture", editorSessionId: "session-1",
      loopId: "loop:tank", loopRevision: "sha256:loop-1",
    };
    const command = (kind: string, payload: unknown) => ({
      schemaVersion: 1, kind, messageId: "message-1",
      projectId: expected.projectId, editorSessionId: expected.editorSessionId,
      loopId: expected.loopId, expectedLoopRevision: expected.loopRevision, payload,
    });
    assert.deepStrictEqual(messages.decodeControlExperimentCommand(command(
      "control-studio.experiment.review",
      { kind: "setpoint_step", amplitude: 5, durationSeconds: 30, physicalTopologyReviewed: true }
    ), expected), {
      action: "review", messageId: "message-1", kind: "setpoint_step", amplitude: 5, durationSeconds: 30,
      physicalTopologyReviewed: true,
    });
    assert.deepStrictEqual(messages.decodeControlExperimentCommand(command(
      "control-studio.experiment.approve", { planId: "experiment-1", physicalApproved: false }
    ), expected), {
      action: "approve", messageId: "message-1", planId: "experiment-1", physicalApproved: false,
    });
    assert.deepStrictEqual(messages.decodeControlExperimentCommand(command(
      "control-studio.experiment.abort", { experimentId: "experiment-1" }
    ), expected), { action: "abort", messageId: "message-1", experimentId: "experiment-1" });
    assert.strictEqual(messages.decodeControlExperimentCommand({
      ...command("control-studio.experiment.approve", { planId: "experiment-1", physicalApproved: false }),
      expectedLoopRevision: "sha256:stale",
    }, expected), undefined);
  });

  test("recovers only the exact loop-scoped running experiment", () => {
    const api = experimentApi();
    assert.strictEqual(typeof api.decodeControlExperimentRecovery, "function",
      "Control Studio must expose strict experiment recovery decoding");
    const scope = { projectId: "project:fixture", loopId: "loop:tank", loopRevision: "sha256:loop-1" };
    const stored = { schemaVersion: 1, experimentId: "experiment-1", ...scope };
    assert.deepStrictEqual(api.decodeControlExperimentRecovery(stored, scope), { experimentId: "experiment-1" });
    assert.strictEqual(api.decodeControlExperimentRecovery({ ...stored, loopRevision: "sha256:stale" }, scope), undefined);
    assert.strictEqual(api.decodeControlExperimentRecovery({ ...stored, extra: true }, scope), undefined);
  });
});

function experimentApi(): any {
  try { return require("../../controlStudio/experiment"); } catch { return {}; }
}

function loop() {
  return {
    schemaVersion: 1, id: "loop:tank", name: "Tank level", revision: "sha256:loop-1",
    projectId: "project:fixture", runtimeId: "runtime:local",
    controller: { instance: "TankController", type: "Control.PID", revision: "sha256:controller-1", action: "direct", taskPeriodMs: 10 },
    signals: {
      setpoint: { symbol: "LevelSP", address: "%ML0", iecType: "LREAL", direction: "read", unit: "%", range: { minimum: 0, maximum: 100 } },
      processVariable: { symbol: "LevelPV", address: "%IL0", iecType: "LREAL", direction: "read", unit: "%", range: { minimum: 0, maximum: 100 } },
      manipulatedVariable: { symbol: "ValveMV", address: "%QL0", iecType: "LREAL", direction: "write", unit: "%", range: { minimum: 0, maximum: 100 } },
    },
    limits: { outputMinimum: 0, outputMaximum: 100 }, observedSampleTiming: { nominalPeriodMs: 10 },
  };
}

function sample() {
  return {
    availability: "available", source: "Simulation", loopId: "loop:tank", loopRevision: "sha256:loop-1",
    controller: { instance: "TankController", revision: "sha256:controller-1", mode: "Auto" },
    scan: 41, observedAtMs: 1_000,
    setpoint: { symbol: "LevelSP", address: "%ML0", value: 50, unit: "%", quality: "good" },
    processVariable: { symbol: "LevelPV", address: "%IL0", value: 48, unit: "%", quality: "good" },
    manipulatedVariable: { symbol: "ValveMV", address: "%QL0", value: 32, unit: "%", quality: "good" },
    outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, saturation: "none", recording: false,
  };
}
