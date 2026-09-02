import * as assert from "assert";

suite("Control Studio bounded experiment", () => {
  test("offers every supported experiment without exposing the legacy wire alias", () => {
    assert.deepStrictEqual(experimentApi().controlExperimentChoices(), [
      { value: "observation", label: "Observe without excitation" },
      { value: "setpoint_step", label: "Setpoint step" },
      { value: "load_step", label: "Load disturbance step" },
      { value: "mv_step", label: "Manual output step" },
      { value: "mv_pulse", label: "Manual output pulse" },
      { value: "relay", label: "Relay feedback" },
      { value: "prbs", label: "PRBS" },
      { value: "multisine", label: "Multisine" },
    ]);
  });

  test("opens Test with the exact defaults owned by the configured lesson loop", () => {
    const api = experimentApi();
    assert.deepStrictEqual(api.controlExperimentDefaults({
      ...loop(),
      experiment: {
        execution: "project-scripted", kind: "observation", amplitude: 0,
        durationSeconds: 120, seed: 20260824,
      },
    }), {
      execution: "project-scripted", kind: "observation", amplitude: 0,
      durationSeconds: 120, seed: 20260824,
    });
    assert.deepStrictEqual(api.controlExperimentDefaults(loop()), {
      execution: "control-studio", kind: "setpoint_step", amplitude: 5,
      durationSeconds: 30, seed: 1,
    });
  });

  test("starts and restores a project-owned lesson through its single Boolean authority", () => {
    const api = experimentApi();
    const projectLoop = {
      ...loop(),
      controller: { ...loop().controller, type: "RouthComparison", design: "project" },
      experiment: {
        execution: "project-scripted", kind: "observation", amplitude: 0,
        durationSeconds: 40, seed: 20260829,
        activation: { symbol: "P1.ExperimentEnabled", address: "%MX96.0", iecType: "BOOL", direction: "write" },
      },
    };
    assert.deepStrictEqual(api.buildControlExperimentPlan({
      loop: projectLoop, sample: sample(), kind: "observation", amplitude: 0, durationSeconds: 40,
    }), {
      kind: "accepted",
      plan: {
        id: "experiment:loop:tank:41:project_sequence",
        source: "simulation", kind: "project_sequence", affectedSignal: "P1.ExperimentEnabled",
        address: "%MX96.0", valueType: "BOOL", from: false, to: true, unit: "authority",
        minimum: 0, maximum: 1, durationMs: 40_000, expectedScan: 41,
        stopOnStaleMs: 500,
        stopConditions: ["Abort", "40 s timeout", "Stale runtime data", "Signal force", "Runtime fault"],
        restorationValue: false, requiresPhysicalApproval: false, trajectory: [],
      },
    });
    assert.deepStrictEqual(api.buildControlExperimentPlan({
      loop: projectLoop,
      sample: { ...sample(), source: "Physical" },
      kind: "observation", amplitude: 0, durationSeconds: 40,
    }), {
      kind: "rejected",
      reason: "A reviewed physical topology is required before approval can be requested.",
    });
  });

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
        trajectory: [{ atMs: 0, value: 55 }],
      },
    });
  });

  test("accepts bounded slow-process lessons up to one hour", () => {
    const api = experimentApi();
    const accepted = api.buildControlExperimentPlan({
      loop: loop(), sample: sample(), kind: "setpoint_step", amplitude: 5, durationSeconds: 1_200,
    });
    assert.strictEqual(accepted.kind, "accepted");
    assert.strictEqual(accepted.plan.durationMs, 1_200_000);
    assert.deepStrictEqual(api.buildControlExperimentPlan({
      loop: loop(), sample: sample(), kind: "setpoint_step", amplitude: 5, durationSeconds: 3_601,
    }), {
      kind: "rejected", reason: "Amplitude must be non-zero and duration must be between 1 and 3600 seconds.",
    });
  });

  test("builds every declared bounded experiment from the same loop and safety envelope", () => {
    const api = experimentApi();
    const base = { loop: loopWithDisturbance(), sample: sampleWithDisturbance(), amplitude: 5, durationSeconds: 10 };
    const plans = ["observation", "setpoint_step", "load_step", "mv_step", "mv_pulse", "relay", "prbs", "multisine"]
      .map((kind) => api.buildControlExperimentPlan({ ...base, kind, seed: 7,
        sample: ["mv_step", "mv_pulse", "relay", "prbs", "multisine"].includes(kind)
          ? { ...sample(), controller: { ...sample().controller, mode: "Manual" } }
          : base.sample }));
    assert.deepStrictEqual(plans.map((result: any) => result.kind), Array(8).fill("accepted"));
    assert.deepStrictEqual(plans.map((result: any) => result.plan.kind),
      ["observation", "setpoint_step", "load_step", "mv_step", "mv_pulse", "relay", "prbs", "multisine"]);
    assert.deepStrictEqual(plans[0].plan?.trajectory ?? "missing_trajectory", []);
    assert.strictEqual(plans[2].plan.address, "%QL8");
    assert.deepStrictEqual(plans[5].plan.trajectory, []);
    assert.deepStrictEqual(plans[5].plan.relayFeedback, {
      processVariable: "LevelPV", address: "%IL0", valueType: "LREAL", unit: "%",
      center: 48, hysteresis: 0.5, lowerThreshold: 47.5, upperThreshold: 48.5,
      lowOutput: 27, highOutput: 37,
    });
    assert.strictEqual(plans[6].plan.seed, 7);
    assert.deepStrictEqual(plans[7].plan.frequenciesHz, [0.1, 0.2, 0.3]);
    for (const result of plans.slice(1).filter((result: any) => result.plan.kind !== "relay")) {
      assert.ok(result.plan.trajectory.length > 0);
      assert.ok(result.plan.trajectory.every((point: any) => point.value >= result.plan.minimum && point.value <= result.plan.maximum));
      assert.strictEqual(result.plan.restorationValue, result.plan.from);
    }
    assert.strictEqual(plans[5].plan.restorationValue, plans[5].plan.from);
  });

  test("builds relay as measured feedback instead of a timed square wave", () => {
    const result = experimentApi().buildControlExperimentPlan({
      loop: loop(),
      sample: { ...sample(), controller: { ...sample().controller, mode: "Manual" } },
      kind: "relay", amplitude: 5, durationSeconds: 30,
    });
    assert.strictEqual(result.kind, "accepted");
    assert.deepStrictEqual(result.plan.trajectory, []);
    assert.strictEqual(result.plan.to, 37);
    assert.deepStrictEqual(result.plan.relayFeedback, {
      processVariable: "LevelPV", address: "%IL0", valueType: "LREAL", unit: "%",
      center: 48, hysteresis: 0.5, lowerThreshold: 47.5, upperThreshold: 48.5,
      lowOutput: 27, highOutput: 37,
    });
  });

  test("allows a reviewed open-loop output step without inventing a controller mode", () => {
    const openLoop = {
      ...loop(),
      topology: "open-loop",
      controller: { ...loop().controller, type: "OpenLoopExperiment", design: "project" },
      signals: { ...loop().signals, setpoint: undefined },
    };
    const openLoopSample = {
      ...sample(),
      setpoint: undefined,
      controller: { ...sample().controller, mode: "Unavailable" },
    };

    const result = experimentApi().buildControlExperimentPlan({
      loop: openLoop,
      sample: openLoopSample,
      kind: "mv_step",
      amplitude: 5,
      durationSeconds: 30,
    });

    assert.strictEqual(result.kind, "accepted");
    assert.strictEqual(result.plan.address, "%QL0");
  });

  test("allows a reviewed open-loop disturbance step without inventing a controller mode", () => {
    const disturbance = {
      id: "gust", channel: "airframe", role: "disturbance", symbol: "GustLoad",
      address: "%ML32", iecType: "LREAL", direction: "write", unit: "g",
      range: { minimum: -1, maximum: 1 },
    };
    const openLoop = {
      ...loop(), topology: "open-loop",
      controller: { ...loop().controller, type: "RobustComparison", design: "project" },
      signals: { ...loop().signals, setpoint: undefined, additional: [disturbance] },
    };
    const openLoopSample = {
      ...sample(), setpoint: undefined,
      controller: { ...sample().controller, mode: "Unavailable" },
      additional: [{ ...disturbance, value: 0, quality: "good" }],
    };

    const result = experimentApi().buildControlExperimentPlan({
      loop: openLoop, sample: openLoopSample, kind: "load_step", amplitude: 0.1,
      durationSeconds: 30,
    });

    assert.strictEqual(result.kind, "accepted");
    assert.strictEqual(result.plan.address, "%ML32");
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
    assert.deepStrictEqual(api.buildControlExperimentPlan({ ...base, sample: {
      ...sample(), processVariable: { ...sample().processVariable, quality: "bad" },
    } }), { kind: "rejected", reason: "Fresh good-quality SP, PV, and MV are required." });
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
      trajectory: [{ at_ms: 0, value: 55 }],
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
    for (const kind of ["observation", "load_step", "mv_step", "mv_pulse", "relay", "prbs", "multisine"] as const) {
      assert.strictEqual(messages.decodeControlExperimentCommand(command(
        "control-studio.experiment.review",
        { kind, amplitude: 5, durationSeconds: 30, physicalTopologyReviewed: false }
      ), expected)?.kind, kind, `review messages must preserve ${kind}`);
    }
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

  test("keeps polling terminal experiments until restoration is confirmed or fails", () => {
    const classify = experimentApi().classifyControlExperimentStatus;
    assert.strictEqual(classify({ status: "running", restoration: "pending" }), "running");
    assert.strictEqual(classify({ status: "aborted", restoration: "pending" }), "awaiting_restoration");
    assert.strictEqual(classify({ status: "failed", restoration: "pending" }), "awaiting_restoration");
    assert.strictEqual(classify({ status: "completed", restoration: "restored" }), "terminal");
    assert.strictEqual(classify({ status: "aborted", restoration: "restored" }), "terminal");
    assert.strictEqual(classify({ status: "failed_to_restore", restoration: "failed" }), "terminal");
    assert.strictEqual(classify({ status: "aborted", restoration: "failed" }), "invalid");
    assert.strictEqual(classify({ status: "mystery", restoration: "pending" }), "invalid");
  });
});

function experimentApi(): any {
  try { return require("../../controlStudio/experiment"); } catch { return {}; }
}

function sampleWithDisturbance() {
  return { ...sample(), additional: [{ id: "feed-flow", channel: "reactor", role: "disturbance",
    symbol: "FeedFlow", address: "%QL8", value: 32, unit: "%", quality: "good" }] };
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

function loopWithDisturbance() {
  const value = loop();
  return { ...value, signals: { ...value.signals, additional: [{
    id: "feed-flow", channel: "reactor", role: "disturbance", symbol: "FeedFlow",
    address: "%QL8", iecType: "LREAL", direction: "write", unit: "%",
    range: { minimum: 0, maximum: 100 },
  }] } };
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
