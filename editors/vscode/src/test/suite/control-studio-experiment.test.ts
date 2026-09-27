import * as assert from "assert";

suite("Control Studio bounded experiment", () => {
  test("records the declared project baseline before enabling the same Boolean authority", async () => {
    const api = experimentApi();
    const configured = { ...loop(), experiment: { execution: "project-scripted", kind: "observation", amplitude: 0,
      durationSeconds: 35.1, seed: 1, baselineSeconds: 5,
      activation: { symbol: "P1.Run", address: "%MX64.0", iecType: "BOOL", direction: "write" } } };
    const result = api.buildControlExperimentPlan({ loop: configured, sample: sample(), kind: "observation", amplitude: 0, durationSeconds: 35.1 });
    assert.strictEqual(result.kind, "accepted");
    assert.deepStrictEqual(result.plan.trajectory, [{ atMs: 0, value: 0 }, { atMs: 5000, value: 1 }],
      "the recorder must observe five actual seconds with the project disabled before activation");
    let sent: any;
    const client = new api.ControlExperimentClient(async (_operation: string, params: any) => { sent = params; return {}; });
    await client.start(result.plan, false);
    assert.deepStrictEqual(sent.trajectory, [{ at_ms: 0, value: false }, { at_ms: 5000, value: true }]);
    assert.strictEqual(sent.restoration_value, false);
    assert.strictEqual(sent.duration_ms, 35100, "the reviewed maximum includes baseline and active intervals");
    for (const durationSeconds of [1, 5]) {
      assert.strictEqual(api.buildControlExperimentPlan({ loop: configured, sample: sample(), kind: "observation", amplitude: 0, durationSeconds }).kind, "rejected");
    }
    for (const baselineSeconds of [-1, NaN, Infinity, 35.1]) {
      assert.strictEqual(api.buildControlExperimentPlan({ loop: { ...configured, experiment: { ...configured.experiment, baselineSeconds } }, sample: sample(), kind: "observation", amplitude: 0, durationSeconds: 35.1 }).kind, "rejected");
    }
  });
  test("project-owned excitation rejects forced or bad primary signals while retaining auxiliary diagnostics", () => {
    const api = experimentApi();
    const configured = { ...loop(), experiment: { execution: "project-scripted", kind: "observation", amplitude: 0,
      durationSeconds: 30, seed: 1, activation: { symbol: "P1.Run", address: "%MX64.0", iecType: "BOOL", direction: "write" } } };
    for (const key of ["setpoint", "processVariable", "manipulatedVariable"] as const) {
      for (const quality of ["forced", "bad"] as const) {
        const observed = { ...sample(), [key]: { ...sample()[key], quality } };
        assert.strictEqual(api.buildControlExperimentPlan({ loop: configured, sample: observed, kind: "observation", amplitude: 0, durationSeconds: 30 }).kind,
          "rejected", `${key} ${quality} must block Boolean activation`);
      }
    }
    const observed = { ...sample(), additional: [{ id: "diagnostic", value: 1, quality: "forced" }] };
    assert.strictEqual(api.buildControlExperimentPlan({ loop: configured, sample: observed, kind: "observation", amplitude: 0, durationSeconds: 30 }).kind, "accepted");
  });
  test("reviews and transmits a distinct latched project abort assertion", async () => {
    const api = experimentApi();
    const configured = { ...loop(), schemaVersion: 2, experiment: {
      execution: "project-scripted", kind: "observation", amplitude: 0, durationSeconds: 30, seed: 1,
      activation: { symbol: "P1.Run", address: "%MX64.0", iecType: "BOOL", direction: "write" },
      abort: { symbol: "P1.Abort", address: "%MX64.1", iecType: "BOOL", direction: "write", assertedValue: true },
    } };
    const result = api.buildControlExperimentPlan({ loop: configured, sample: sample(), kind: "observation", amplitude: 0, durationSeconds: 30 });
    assert.deepStrictEqual(result.plan.abortBinding, { symbol: "P1.Abort", address: "%MX64.1", valueType: "BOOL", assertedValue: true });
    assert.ok(result.plan.stopConditions.some((reason: string) => reason.includes("P1.Abort") && reason.includes("true")));
    let sent: any;
    const client = new api.ControlExperimentClient(async (_operation: string, params: any) => { sent = params; return {}; });
    await client.start(result.plan, false);
    assert.deepStrictEqual(sent.abort_binding, { address: "%MX64.1", value_type: "BOOL", asserted_value: true });
    assert.strictEqual(sent.restoration_value, false);
  });

  test("rejects an invalid project duration before presenting approval", () => {
    const api = experimentApi();
    const configured = { ...loop(), experiment: { execution: "project-scripted", kind: "observation", amplitude: 0,
      durationSeconds: 30, seed: 1, activation: { symbol: "P1.Run", address: "%MX64.0", iecType: "BOOL", direction: "write" } } };
    for (const durationSeconds of [0, Number.NaN, Number.POSITIVE_INFINITY, 3601]) {
      assert.strictEqual(api.buildControlExperimentPlan({ loop: configured, sample: sample(), kind: "observation", amplitude: 0, durationSeconds }).kind, "rejected");
    }
  });

  test("offers every supported experiment without exposing the legacy wire alias", () => {
    assert.deepStrictEqual(experimentApi().controlExperimentChoices(), [
      { value: "automatic", label: "Automatic bounded excitation" },
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
      execution: "control-studio", kind: "automatic", amplitude: 10,
      durationSeconds: 30, seed: 1,
    });
  });

  test("selects a bounded excitation automatically while retaining manual observation", () => {
    const api = experimentApi();
    const configured = {
      ...loop(),
      experiment: { execution: "control-studio", kind: "observation", amplitude: 0, durationSeconds: 30, seed: 7 },
    };
    const defaults = api.controlExperimentDefaults(configured);
    assert.strictEqual(defaults.kind, "automatic",
      "a zero-amplitude configured observation is an automatic identification request, not an uninformative default Test");
    assert.strictEqual(defaults.amplitude, 10, "automatic amplitude starts at ten percent of the declared signal span");
    const shortAutomatic = api.controlExperimentDefaults({
      ...configured,
      experiment: { ...configured.experiment, durationSeconds: 2 },
    });
    assert.strictEqual(shortAutomatic.durationSeconds, 10,
      "automatic identification reserves a ten-second bounded observation window unless the configured duration is longer");

    const manual = { ...sample(), controller: { ...sample().controller, mode: "Manual" } };
    const automatic = api.buildControlExperimentPlan({
      loop: configured, sample: manual, kind: "automatic", amplitude: defaults.amplitude, durationSeconds: defaults.durationSeconds,
    });
    assert.strictEqual(automatic.kind, "accepted");
    assert.strictEqual(automatic.plan.kind, "mv_step", "a Manual loop receives a bounded output step for identification");
    assert.deepStrictEqual(automatic.plan.trajectory, [
      { atMs: 0, value: 32 },
      { atMs: 3_000, value: 42 },
      { atMs: 15_000, value: 32 },
    ]);

    const automaticAuto = api.buildControlExperimentPlan({
      loop: configured, sample: sample(), kind: "automatic", amplitude: defaults.amplitude, durationSeconds: defaults.durationSeconds,
    });
    assert.strictEqual(automaticAuto.kind, "accepted");
    assert.strictEqual(automaticAuto.plan.kind, "setpoint_step", "an Auto loop uses the declared setpoint before disturbing output directly");
    assert.strictEqual(automaticAuto.plan.to, 60);
    assert.deepStrictEqual(automaticAuto.plan.trajectory, [
      { atMs: 0, value: 50 },
      { atMs: 3_000, value: 60 },
      { atMs: 15_000, value: 50 },
    ]);

    const observation = api.buildControlExperimentPlan({
      loop: configured, sample: sample(), kind: "observation", amplitude: 0, durationSeconds: defaults.durationSeconds,
    });
    assert.strictEqual(observation.kind, "accepted");
    assert.strictEqual(observation.plan.kind, "observation", "manual observation remains non-exciting");
    assert.deepStrictEqual(observation.plan.trajectory, []);

    const tooShort = api.buildControlExperimentPlan({
      loop: { ...configured, controller: { ...configured.controller, taskPeriodMs: 500 },
        observedSampleTiming: { nominalPeriodMs: 500 } }, sample: manual,
      kind: "automatic", amplitude: defaults.amplitude, durationSeconds: 1,
    });
    assert.deepStrictEqual(tooShort, {
      kind: "rejected",
      reason: "Automatic Test duration is too short to capture baseline, held step and restoration.",
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

  test("plans an MV pulse as a bounded rise and return while holding an MV step", async () => {
    const api = experimentApi();
    const manualSample = { ...sample(), controller: { ...sample().controller, mode: "Manual" } };
    const pulse = api.buildControlExperimentPlan({
      loop: loop(), sample: manualSample, kind: "mv_pulse", amplitude: 5, durationSeconds: 30,
    });
    const step = api.buildControlExperimentPlan({
      loop: loop(), sample: manualSample, kind: "mv_step", amplitude: 5, durationSeconds: 30,
    });

    assert.strictEqual(pulse.kind, "accepted");
    assert.deepStrictEqual(pulse.plan.trajectory, [
      { atMs: 0, value: 37 },
      { atMs: 15_000, value: 32 },
    ]);
    assert.strictEqual(step.kind, "accepted");
    assert.deepStrictEqual(step.plan.trajectory, [{ atMs: 0, value: 37 }]);

    let request: any;
    const client = new api.ControlExperimentClient(async (_operation: string, params: any) => {
      request = params;
      return {};
    });
    await client.start(pulse.plan, false);
    assert.deepStrictEqual(request.trajectory, [
      { at_ms: 0, value: 37 },
      { at_ms: 15_000, value: 32 },
    ]);
  });

  test("writes the declared Manual command rather than an observed controller output", () => {
    const api = experimentApi();
    const configured = {
      ...loop(),
      controller: {
        ...loop().controller,
        manualOutputSignal: "TankController.ManualCommand",
        manualOutputAddress: "%ML8",
        manualOutputType: "LREAL",
      },
    };
    const manual = { ...sample(), controller: { ...sample().controller, mode: "Manual" } };
    const pulse = api.buildControlExperimentPlan({
      loop: configured, sample: manual, kind: "mv_pulse", amplitude: 5, durationSeconds: 30,
    });
    assert.strictEqual(pulse.kind, "accepted");
    assert.strictEqual(pulse.plan.affectedSignal, "TankController.ManualCommand");
    assert.strictEqual(pulse.plan.address, "%ML8");
    assert.strictEqual(pulse.plan.valueType, "LREAL");
    assert.strictEqual(pulse.plan.from, 32, "use the measured actual MV as the Manual-command baseline");
    assert.deepStrictEqual(pulse.plan.trajectory, [
      { atMs: 0, value: 37 },
      { atMs: 15_000, value: 32 },
    ]);
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
      kind: "rejected", reason: "Duration must be between 1 and 3600 seconds.",
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
    assert.deepStrictEqual(messages.decodeControlExperimentCommand(command(
      "control-studio.relay.calculate", {}
    ), expected), { action: "calculate-relay", messageId: "message-1" },
    "the explicit relay calculation command must be bound to the current loop and carry no mutable candidate settings");
    assert.strictEqual(messages.decodeControlExperimentCommand(command(
      "control-studio.relay.calculate", { proportionalGain: 99 }
    ), expected), undefined, "the webview must never supply relay tuning values to the host");
    assert.deepStrictEqual(messages.decodeAntiWindupCommand(command(
      "control-studio.anti-windup.review", { trackingTimeSeconds: 4 }
    ), expected), { action: "review", messageId: "message-1", trackingTimeSeconds: 4 });
    assert.deepStrictEqual(messages.decodeAntiWindupCommand(command(
      "control-studio.anti-windup.stage", {}
    ), expected), { action: "stage", messageId: "message-1" });
    assert.deepStrictEqual(messages.decodeAntiWindupCommand(command(
      "control-studio.anti-windup.verify", {}
    ), expected), { action: "verify", messageId: "message-1" });
    assert.deepStrictEqual(messages.decodeAntiWindupCommand(command(
      "control-studio.anti-windup.rollback", {}
    ), expected), { action: "rollback", messageId: "message-1" });
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

  test("recovers a Test recording only with its exact reviewed plan and recording owner", () => {
    const api = experimentApi();
    const configured = loop();
    const scope = { projectId: configured.projectId, loopId: configured.id, loopRevision: configured.revision };
    const plan = api.buildControlExperimentPlan({
      loop: configured, sample: sample(), kind: "setpoint_step", amplitude: 5, durationSeconds: 30,
    }).plan;
    const recordingOwner = {
      schemaVersion: 1, recordingId: "recording-test-1", projectId: configured.projectId, runtimeId: configured.runtimeId,
      loopId: configured.id, loopRevision: configured.revision, source: "Simulation", programRevision: "program-test-1",
      bindings: [
        { id: "setpoint", address: "%ML0", iec_type: "LREAL" },
        { id: "processVariable", address: "%IL0", iec_type: "LREAL" },
        { id: "manipulatedVariable", address: "%QL0", iec_type: "LREAL" },
      ],
    };
    const stored = { schemaVersion: 2, experimentId: plan.id, ...scope, plan, recordingOwner };
    const recovered = api.decodeControlExperimentRecovery(stored, scope);
    assert.strictEqual(recovered?.experimentId, plan.id);
    assert.deepStrictEqual(recovered?.plan, plan);
    assert.deepStrictEqual(recovered?.recordingOwner, recordingOwner);
    assert.strictEqual(api.decodeControlExperimentRecovery({ ...stored, experimentId: "other" }, scope), undefined);
    assert.strictEqual(api.decodeControlExperimentRecovery({ ...stored, plan: { ...plan, id: "other" } }, scope), undefined);
    assert.strictEqual(api.decodeControlExperimentRecovery({ ...stored, recordingOwner: { ...recordingOwner, loopRevision: "sha256:stale" } }, scope), undefined);
  });

  test("stores and restores the combined Test recording recovery transaction", async () => {
    const api = experimentApi();
    const persistence = experimentPersistenceApi();
    const configured = loop();
    const plan = api.buildControlExperimentPlan({
      loop: configured, sample: sample(), kind: "setpoint_step", amplitude: 5, durationSeconds: 30,
    }).plan;
    const recordingOwner = { schemaVersion: 1, recordingId: "recording-test-2", projectId: configured.projectId,
      runtimeId: configured.runtimeId, loopId: configured.id, loopRevision: configured.revision, source: "Simulation",
      bindings: [{ id: "pv", address: "%IL0", iec_type: "LREAL" }] };
    const values = new Map<string, unknown>();
    const context = { workspaceState: {
      get: (key: string) => values.get(key),
      update: async (key: string, value: unknown) => { values.set(key, value); },
    } };
    await persistence.storeExperimentRecovery(context, configured, plan, recordingOwner);
    const recovered = persistence.restoreExperimentRecovery(context, configured);
    assert.ok(recovered, `combined recovery was not retained: ${JSON.stringify([...values.entries()])}`);
    assert.strictEqual(recovered.experimentId, plan.id);
    assert.deepStrictEqual(recovered.plan, plan);
    assert.deepStrictEqual(recovered.recordingOwner, recordingOwner);
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

  test("hands immutable completed and failed Test records to Identify with explicit roles and fresh retries", () => {
    const api = experimentApi();
    assert.strictEqual(typeof api.ExperimentIdentificationLedger, "function",
      "terminal Test results need an immutable Identify ledger");
    const plan = api.buildControlExperimentPlan({
      loop: loop(), sample: sample(), kind: "setpoint_step", amplitude: 5, durationSeconds: 30,
    }).plan;
    const ledger = new api.ExperimentIdentificationLedger();
    const failed = ledger.record(plan, {
      experiment_id: plan.id, status: "failed", stop_reason: "stale_runtime_data", restoration: "restored",
    }, [{ timeSeconds: 0, input: 50, output: 48, quality: "good" }]);
    assert.strictEqual(failed.role, "estimation");
    assert.strictEqual(failed.outcome, "failed");
    assert.strictEqual(Object.isFrozen(failed), true);
    assert.strictEqual(Object.isFrozen(failed.samples), true);
    assert.strictEqual(ledger.records.length, 1);
    assert.throws(() => ledger.retry(failed.experimentId, plan), /fresh reviewed plan/i);

    const retryPlan = { ...plan, id: "experiment:loop:tank:42:setpoint_step", expectedScan: 42 };
    const retry = ledger.retry(failed.experimentId, retryPlan);
    assert.notStrictEqual(retry, plan);
    assert.strictEqual(retry.id, retryPlan.id);
    const estimation = ledger.record(retry, {
      experiment_id: retry.id, status: "completed", stop_reason: "duration_elapsed", restoration: "restored",
    }, [{ timeSeconds: 0, input: 50, output: 48, quality: "good" }]);
    assert.strictEqual(estimation.role, "estimation");
    assert.deepStrictEqual(ledger.identificationRecords.map((record: any) => record.experimentId), [retry.id]);

    const validationPlan = { ...retryPlan, id: "experiment:loop:tank:43:setpoint_step", expectedScan: 43, to: 57 };
    const validation = ledger.record(validationPlan, {
      experiment_id: validationPlan.id, status: "completed", stop_reason: "duration_elapsed", restoration: "restored",
    }, [{ timeSeconds: 0, input: 50, output: 48, quality: "good" }]);
    assert.strictEqual(validation.role, "validation");
  });

  test("uses immutable Test ledger roles for Identify before falling back to manual recording", () => {
    const api = experimentApi();
    assert.strictEqual(typeof api.identificationRecordForRole, "function",
      "Identify must select completed immutable Test evidence by role");
    const ledger = new api.ExperimentIdentificationLedger();
    const plan = api.buildControlExperimentPlan({
      loop: loop(), sample: sample(), kind: "setpoint_step", amplitude: 5, durationSeconds: 30,
    }).plan;
    const failed = ledger.record(plan, {
      experiment_id: plan.id, status: "failed", restoration: "restored",
    }, identificationSamples(5));
    const estimationPlan = { ...plan, id: "experiment:loop:tank:42:setpoint_step", expectedScan: 42 };
    const estimation = ledger.record(estimationPlan, {
      experiment_id: estimationPlan.id, status: "completed", restoration: "restored",
    }, identificationSamples(5));
    const validationPlan = { ...estimationPlan, id: "experiment:loop:tank:43:setpoint_step", expectedScan: 43, to: 57 };
    const validation = ledger.record(validationPlan, {
      experiment_id: validationPlan.id, status: "completed", restoration: "restored",
    }, identificationSamples(7));
    assert.strictEqual(api.identificationRecordForRole(ledger.records, "estimation")?.experimentId, estimation.experimentId);
    assert.strictEqual(api.identificationRecordForRole(ledger.records, "validation")?.experimentId, validation.experimentId);
    assert.notStrictEqual(api.identificationRecordForRole(ledger.records, "estimation")?.experimentId, failed.experimentId,
      "failed Test records must not reach Identify");

    const { IdentificationWorkflow } = require("../../controlStudio/identification") as any;
    const workflow = new IdentificationWorkflow();
    const estimationQualification = workflow.qualify(
      api.identificationRecordForRole(ledger.records, workflow.nextFitRole).samples,
      [], "Simulation", "automatic"
    );
    assert.strictEqual(estimationQualification.suitable, true);
    assert.strictEqual(workflow.fit().stage, "validation_required");
    const validationQualification = workflow.qualify(
      api.identificationRecordForRole(ledger.records, workflow.nextFitRole).samples,
      [], "Simulation", "automatic"
    );
    assert.strictEqual(validationQualification.suitable, true);
    assert.strictEqual(workflow.fit().stage, "acceptance_required");
  });

  test("admits only complete runtime-owned Test recording samples to Identify", () => {
    const api = experimentApi();
    assert.strictEqual(typeof api.identificationSamplesFromRuntimeRecording, "function",
      "Test-to-Identify must reject editor-observed snapshots and consume runtime acquisition");
    const complete = runtimeOwnedRecording();
    assert.deepStrictEqual(api.identificationSamplesFromRuntimeRecording({ kind: "mv_step" }, complete), [
      { timeSeconds: 0, input: 32, output: 48, quality: "good", inputUnit: "%", outputUnit: "°C" },
      { timeSeconds: 0.02, input: 35, output: 48.5, quality: "bad", inputUnit: "%", outputUnit: "°C", saturated: true },
      { timeSeconds: 0.04, input: 35, output: 49, quality: "good", inputUnit: "%", outputUnit: "°C", saturated: true },
    ]);
    assert.deepStrictEqual(api.identificationSamplesFromRuntimeRecording({ kind: "setpoint_step" }, complete)
      .map((sample: any) => [sample.input, sample.inputUnit]), [[50, "°C"], [55, "°C"], [55, "°C"]]);
    assert.throws(() => api.identificationSamplesFromRuntimeRecording({ kind: "mv_step" }, {
      ...complete, evidence: { ...complete.evidence, method: "ui_observed_snapshots", completeness: "incomplete",
        actualCycleTimingAvailable: false },
    }), /complete runtime-owned recording/i);
    assert.throws(() => api.identificationSamplesFromRuntimeRecording({ kind: "mv_step" }, {
      ...complete, rawSamples: [complete.rawSamples[0], { ...complete.rawSamples[2], scan: 14 }],
    }), /contiguous runtime scans/i);
  });

  test("retains an incomplete aborted Test prefix while keeping it out of Identify", () => {
    const api = experimentApi();
    assert.strictEqual(typeof api.retainedIdentificationSamplesFromRuntimeRecording, "function",
      "incomplete Test records need a retained-prefix projection");
    const manualSample = { ...sample(), controller: { ...sample().controller, mode: "Manual" } };
    const plan = api.buildControlExperimentPlan({
      loop: loop(), sample: manualSample, kind: "mv_step", amplitude: 5, durationSeconds: 30,
    }).plan;
    const recording = runtimeOwnedRecording();
    recording.evidence = { ...recording.evidence, completeness: "incomplete", actualCycleTimingAvailable: false,
      missingScanRanges: [{ first: 12, last: 12 }], reason: "Runtime acquisition stopped after abort; scan 12 is unavailable." };
    const ledger = new api.ExperimentIdentificationLedger();
    const retained = api.retainedIdentificationSamplesFromRuntimeRecording(plan, recording);
    const failed = ledger.record(plan, {
      experiment_id: plan.id, status: "aborted", stop_reason: "user_abort", restoration: "restored",
    }, retained, recording.evidence);
    assert.strictEqual(failed.outcome, "failed");
    assert.strictEqual(failed.qualification, "not_qualified");
    assert.strictEqual(failed.acquisition.completeness, "incomplete");
    assert.deepStrictEqual(failed.samples.map((sample: any) => sample.input), [32, 35, 35]);
    assert.strictEqual(api.identificationRecordForRole(ledger.records, "estimation"), undefined);
    assert.throws(() => ledger.retry(failed.experimentId, plan), /fresh reviewed plan/i);
  });
});

function experimentApi(): any {
  try { return require("../../controlStudio/experiment"); } catch { return {}; }
}

function experimentPersistenceApi(): any {
  try { return require("../../controlStudio/panelPersistence"); } catch { return {}; }
}

function runtimeOwnedRecording(): any {
  const sample = (scan: number, elapsedSeconds: number, appliedMv: number, pv: number, quality: "good" | "bad", sp: number, saturation = "none") => ({
    elapsedSeconds, observedAtMs: 1_000 + elapsedSeconds * 1_000, actualCycleSeconds: elapsedSeconds === 0 ? null : 0.02,
    scan, source: "Simulation", loopRevision: "sha256:loop", sp, pv, requestedMv: null, appliedMv,
    units: { sp: "°C", pv: "°C", mv: "%" }, quality, freshnessMs: null, controllerMode: "Auto",
    outputLimits: { minimum: 0, maximum: 35 }, rateLimits: {}, safetyState: "normal", events: [], saturation,
  });
  return {
    samples: [],
    rawSamples: [sample(11, 0, 32, 48, "good", 50), sample(12, 0.02, 35, 48.5, "bad", 55, "high"), sample(13, 0.04, 35, 49, "good", 55)],
    evidence: { method: "runtime_scan_buffer", completeness: "complete", observedSampleCount: 3,
      scanRange: { first: 11, last: 13 }, missingScanRanges: [], droppedObservedSamples: 0,
      actualCycleTimingAvailable: true, freshnessAvailable: true, reason: "Complete recording — runtime captured 3 scans." },
  };
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

function identificationSamples(amplitude: number) {
  return Array.from({ length: 30 }, (_, index) => ({
    timeSeconds: index,
    input: index < 5 ? 0 : amplitude,
    output: index < 5 ? 0 : amplitude * 0.2 * (1 - Math.exp(-(index - 5) / 4)),
    quality: "good" as const,
    inputUnit: "%",
    outputUnit: "%",
  }));
}
