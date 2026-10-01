import * as assert from "assert";

suite("Control Studio loop definition", () => {
  test("validates declared project baseline duration before joining the runtime", () => {
    const api = loopApi();
    const experiment = { execution: "project-scripted", kind: "observation", amplitude: 0, durationSeconds: 35.1, seed: 1,
      baselineSeconds: 5, activation: { symbol: "P1.Run", address: "%MX64.0", iecType: "BOOL", direction: "write" } };
    const loop = { ...completeLoop(), schemaVersion: 2, experiment };
    assert.strictEqual(api.decodeControlLoopDefinition(loop).kind, "accepted");
    for (const invalid of [
      ...[-1, NaN, Infinity, 35.1].map(baselineSeconds => ({ ...loop, experiment: { ...experiment, baselineSeconds } })),
      { ...loop, schemaVersion: 1 },
      { ...loop, experiment: { ...experiment, activation: undefined } },
      { ...loop, experiment: { ...experiment, execution: "control-studio", activation: undefined } },
    ]) assert.strictEqual(api.decodeControlLoopDefinition(invalid).kind, "rejected");
  });
  test("accepts a hold parameter only when the declared maximum can record the longest permitted hold", () => {
    const api = loopApi();
    const base = completeLoop();
    const activation = { symbol: "P1.Run", address: "%MX64.0", iecType: "BOOL", direction: "write" };
    const hold = { symbol: "P1.HoldSeconds", iec_type: "REAL", unit: "s", convention: "held step duration", minimum: 20, maximum: 40 };
    const experiment = { execution: "project-scripted", kind: "observation", amplitude: 0, durationSeconds: 45.1, seed: 1,
      baselineSeconds: 5, holdParameter: "P1.HoldSeconds", activation };
    const parameterBinding = { loop_id: base.id, task_name: "MainTask", parameters: [hold], mode: { symbol: "P1.Run", manual_value: false } };
    const loop = { ...base, schemaVersion: 2, controller: { ...base.controller, taskPeriodMs: 100 }, observedSampleTiming: { nominalPeriodMs: 100 },
      experiment, parameterPurpose: "experiment", parameterBinding };
    assert.strictEqual(api.decodeControlLoopDefinition(loop).kind, "accepted");
    for (const [label, invalid] of [
      ["a 35.1 s maximum cannot record a 40 s hold", { ...loop, experiment: { ...experiment, durationSeconds: 35.1 } }],
      ["the hold must be a declared parameter", { ...loop, experiment: { ...experiment, holdParameter: "P1.Other" } }],
      ["the hold is declared in seconds", { ...loop, parameterBinding: { ...parameterBinding, parameters: [{ ...hold, unit: "min" }] } }],
      ["the hold belongs to the experiment parameters", { ...loop, parameterPurpose: "controller" }],
      ["only a project-owned test has a hold parameter", { ...loop, experiment: { ...experiment, execution: "control-studio", activation: undefined, baselineSeconds: undefined } }],
    ] as const) assert.strictEqual(api.decodeControlLoopDefinition(invalid).kind, "rejected", label);
  });
  test("requires a distinct explicit abort policy on version2 project experiments", () => {
    const api = loopApi();
    const binding = { symbol: "Main.Run", address: "%MX64.0", iecType: "BOOL", direction: "write" };
    const abort = { ...binding, symbol: "Main.Abort", address: "%MX64.1", assertedValue: true };
    const experiment = { execution: "project-scripted", kind: "observation", amplitude: 0, durationSeconds: 30, seed: 1, activation: binding, abort };
    const valid = { ...completeLoop(), schemaVersion: 2, experiment };
    assert.strictEqual(api.decodeControlLoopDefinition(valid).kind, "accepted");
    for (const bad of [
      { ...valid, schemaVersion: 1 },
      { ...valid, experiment: { ...experiment, activation: undefined } },
      ...[{ assertedValue: undefined }, { iecType: "REAL" }, { address: binding.address }, { direction: "read" }].map(change =>
        ({ ...valid, experiment: { ...experiment, abort: { ...abort, ...change } } })),
    ]) assert.strictEqual(api.decodeControlLoopDefinition(bad).kind, "rejected");
  });

  test("requires an unambiguous compatible mapping from analysis names to ST parameters", () => {
    const api = loopApi();
    const base = completeLoop();
    const analysis = require("./control-studio-declared-analysis.test").declaredShaftAnalysis();
    const parameterBinding = { loop_id: base.id, task_name: "ControlTask",
      parameters: Object.entries(analysis.controller.parameters).map(([name, value]: [string, any]) => ({
        symbol: `P1.${name}`, iec_type: "LREAL", unit: value.unit, convention: name, minimum: value.minimum, maximum: value.maximum })),
      mode: { symbol: "P1.Enabled", manual_value: false } };
    const loop = { ...base, schemaVersion: 2, analysis, parameterBinding, analysisParameterSymbols: { Kp: "P1.Kp", Kd: "P1.Kd" } };
    assert.strictEqual(api.decodeControlLoopDefinition(loop).kind, "accepted");
    for (const mapping of [{}, { Kp: "P1.Kp" }, { Kp: "P1.Kp", Kd: "P1.Kp" }, { Kp: "P1.Unknown", Kd: "P1.Kd" }]) {
      assert.strictEqual(api.decodeControlLoopDefinition({ ...loop, analysisParameterSymbols: mapping }).kind, "rejected");
    }
    for (const changed of [{ unit: "wrong" }, { maximum: 10 }]) {
      const incompatible = { ...loop, parameterBinding: { ...parameterBinding,
        parameters: [{ ...parameterBinding.parameters[0], ...changed }, parameterBinding.parameters[1]] } };
      assert.strictEqual(api.decodeControlLoopDefinition(incompatible).kind, "rejected");
    }
  });

  test("rejects malformed generic parameter bindings before runtime access", () => {
    const api = loopApi();
    const base = completeLoop();
    const parameterBinding = { loop_id: base.id, task_name: "MainTask",
      parameters: [{ symbol: "P1.Gain", iec_type: "REAL", unit: "1", convention: "proportional gain", minimum: 0, maximum: 20 }],
      mode: { symbol: "P1.Enabled", manual_value: false } };
    const loop = { ...base, schemaVersion: 2, parameterBinding };
    assert.strictEqual(api.decodeControlLoopDefinition(loop).kind, "accepted");
    for (const invalid of [
      { ...loop, schemaVersion: 1 },
      { ...loop, parameterBinding: { ...parameterBinding, loop_id: "other" } },
      { ...loop, parameterBinding: { ...parameterBinding, unknown: true } },
      { ...loop, parameterBinding: { ...parameterBinding, parameters: [] } },
      { ...loop, parameterBinding: { ...parameterBinding, parameters: [...parameterBinding.parameters, ...parameterBinding.parameters] } },
      { ...loop, parameterBinding: { ...parameterBinding, mode: { symbol: "P1.Gain", manual_value: false } } },
      ...["DINT", "UNKNOWN"].map(iec_type => ({ ...loop, parameterBinding: { ...parameterBinding,
        parameters: [{ ...parameterBinding.parameters[0], iec_type }] } })),
      ...[Number.NaN, Number.POSITIVE_INFINITY, 21].map(minimum => ({ ...loop, parameterBinding: { ...parameterBinding,
        parameters: [{ ...parameterBinding.parameters[0], minimum }] } })),
    ]) assert.strictEqual(api.decodeControlLoopDefinition(invalid).kind, "rejected", JSON.stringify(invalid.parameterBinding));
  });

  test("accepts experiment-purpose parameters only for a project-scripted activated binding", () => {
    const api = loopApi();
    const activation = { symbol: "P1.ExperimentEnabled", address: "%MX80.0", iecType: "BOOL", direction: "write" };
    const parameterBinding = {
      loop_id: "loop:tank", task_name: "MainTask",
      parameters: [{ symbol: "P1.Kp", iec_type: "LREAL", unit: "1", convention: "gain", minimum: 0, maximum: 10 }],
      mode: { symbol: activation.symbol, manual_value: false },
    };
    const valid = { ...completeLoop(), schemaVersion: 2, parameterPurpose: "experiment", parameterBinding,
      experiment: { execution: "project-scripted", kind: "observation", amplitude: 0, durationSeconds: 30, seed: 1, activation } };
    assert.strictEqual(api.decodeControlLoopDefinition(valid).kind, "accepted");
    for (const invalid of [
      { ...valid, parameterPurpose: "tuning" },
      { ...valid, experiment: { ...valid.experiment, activation: undefined } },
      { ...valid, parameterBinding: { ...parameterBinding, mode: { symbol: activation.symbol, manual_value: true } } },
      { ...valid, experiment: { ...valid.experiment, execution: "host" } },
    ]) assert.strictEqual(api.decodeControlLoopDefinition(invalid).kind, "rejected");
  });

  test("checks provided model units against the bound input and process channels", () => {
    const api = loopApi();
    const loop = { ...completeLoop(), schemaVersion: 2, analysis: {
      plant: { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
        inputUnit: "%", outputUnit: "%", revision: "model-1", source: "provided", validation: "unvalidated" },
      controller: { numerator: [{ parameter: "Kp" }], denominator: [1],
        parameters: { Kp: { value: 1, minimum: 0, maximum: 5, unit: "1" } } },
      rootLocus: { parameter: "Kp", values: [1], selectedValue: 1 },
    } };
    const scope = { projectId: loop.projectId, runtimeId: loop.runtimeId };
    assert.strictEqual(api.validateControlLoopDefinition(loop, scope).kind, "accepted");
    const incompatible = { ...loop, analysis: { ...loop.analysis, plant: { ...loop.analysis.plant, outputUnit: "mm" } } };
    assert.deepStrictEqual(api.validateControlLoopDefinition(incompatible, scope),
      { kind: "rejected", reason: "incompatible_model_units" });
  });

  test("accepts the complete SISO Control.PID loop definition without losing engineering identity", () => {
    let api: any;
    try { api = require("../../controlStudio/loopDefinition"); } catch { api = undefined; }
    const loop = {
      schemaVersion: 1,
      id: "loop:tank",
      name: "Tank level",
      revision: "sha256:loop-1",
      projectId: "project:sha256:abc",
      runtimeId: "runtime:local",
      controller: {
        instance: "TankController",
        type: "Control.PID",
        revision: "sha256:controller-1",
        action: "direct",
        taskPeriodMs: 10,
        modeSignal: "%MX0.0",
        modeWhenTrue: "Auto",
        trackingSignal: "%ML24",
      },
      signals: {
        setpoint: { symbol: "LevelSP", address: "%ML0", iecType: "LREAL", direction: "read", unit: "%", range: { minimum: 0, maximum: 100 } },
        processVariable: { symbol: "LevelPV", address: "%IL0", iecType: "LREAL", direction: "read", unit: "%", range: { minimum: 0, maximum: 100 } },
        manipulatedVariable: { symbol: "ValveMV", address: "%QL0", iecType: "LREAL", direction: "write", unit: "%", range: { minimum: 0, maximum: 100 } },
        actuator: { symbol: "ValvePosition", address: "%IL8", iecType: "LREAL", direction: "read", unit: "%", range: { minimum: 0, maximum: 100 } },
      },
      limits: { outputMinimum: 0, outputMaximum: 100, rateUpPerS: 20, rateDownPerS: 25 },
      observedSampleTiming: { nominalPeriodMs: 10, maximumJitterMs: 1 },
      plantId: "tank",
      operatingPoint: "Normal production",
    };

    assert.deepStrictEqual(api?.decodeControlLoopDefinition?.(loop), { kind: "accepted", loop });
  });

  test("uses the same loop binding for a project-owned non-PID controller", () => {
    const api = loopApi();
    const loop = completeLoop();
    const projectOwned = {
      ...loop,
      controller: {
        ...loop.controller,
        instance: "P1.ColdStorageController",
        type: "ColdStorageOnOff",
        design: "project",
      },
    };
    assert.deepStrictEqual(api.decodeControlLoopDefinition(projectOwned), {
      kind: "accepted", loop: projectOwned,
    });
    assert.deepStrictEqual(api.validateControlLoopDefinition(projectOwned, {
      projectId: loop.projectId, runtimeId: loop.runtimeId,
    }), { kind: "accepted", loop: projectOwned });

    const falselyPidDesigned = {
      ...projectOwned,
      controller: { ...projectOwned.controller, design: "pid" },
    };
    assert.deepStrictEqual(api.validateControlLoopDefinition(falselyPidDesigned, {
      projectId: loop.projectId, runtimeId: loop.runtimeId,
    }), { kind: "rejected", reason: "incompatible_controller_design" });

    assert.deepStrictEqual(api.sharedPidDesignAvailability(projectOwned), {
      available: false,
      reason: "ColdStorageOnOff is designed by the runnable project; shared Control.PID tuning is unavailable.",
    });
    assert.deepStrictEqual(api.sharedPidDesignAvailability(loop), { available: true });
  });

  test("accepts an honest open-loop input/output binding without inventing a setpoint", () => {
    const api = loopApi();
    const closedLoop = completeLoop();
    const openLoop = {
      ...closedLoop,
      topology: "open-loop",
      controller: { ...closedLoop.controller, type: "OpenLoopExperiment", design: "project" },
      signals: {
        processVariable: closedLoop.signals.processVariable,
        manipulatedVariable: closedLoop.signals.manipulatedVariable,
      },
      experiment: {
        execution: "control-studio", kind: "mv_step", amplitude: 10,
        durationSeconds: 30, seed: 20260829,
      },
    };
    assert.deepStrictEqual(api.decodeControlLoopDefinition(openLoop), { kind: "accepted", loop: openLoop });
    assert.deepStrictEqual(api.validateControlLoopDefinition(openLoop, {
      projectId: openLoop.projectId, runtimeId: openLoop.runtimeId,
    }), { kind: "accepted", loop: openLoop });

    const falselyClosedLoopDesigned = {
      ...openLoop,
      controller: { ...closedLoop.controller, design: "pid" },
    };
    assert.deepStrictEqual(api.validateControlLoopDefinition(falselyClosedLoopDesigned, {
      projectId: openLoop.projectId, runtimeId: openLoop.runtimeId,
    }), { kind: "rejected", reason: "incompatible_topology" });

    assert.deepStrictEqual(api.decodeControlLoopDefinition({
      ...openLoop,
      signals: { ...openLoop.signals, setpoint: closedLoop.signals.setpoint },
    }), { kind: "rejected", reason: "malformed_loop" });
  });

  test("validates loop identity, numeric signal compatibility, direction, units, and runtime scope", () => {
    let api: any;
    try { api = require("../../controlStudio/loopDefinition"); } catch { api = undefined; }
    const loop = completeLoop();
    const scope = { projectId: loop.projectId, runtimeId: loop.runtimeId };

    assert.deepStrictEqual(api?.validateControlLoopDefinition?.(loop, scope), {
      kind: "accepted",
      loop,
    });
    assert.deepStrictEqual(
      api?.validateControlLoopDefinition?.({ ...loop, runtimeId: "runtime:other" }, scope),
      { kind: "rejected", reason: "cross_runtime_loop" }
    );
    assert.deepStrictEqual(
      api?.validateControlLoopDefinition?.({
        ...loop,
        signals: {
          ...loop.signals,
          setpoint: { ...loop.signals.setpoint, direction: "write" },
        },
      }, scope),
      { kind: "rejected", reason: "invalid_signal_direction", signal: "setpoint" }
    );
    assert.deepStrictEqual(
      api?.validateControlLoopDefinition?.({
        ...loop,
        signals: {
          ...loop.signals,
          processVariable: { ...loop.signals.processVariable, iecType: "BOOL" },
        },
      }, scope),
      { kind: "rejected", reason: "incompatible_signal_type", signal: "processVariable" }
    );
    assert.deepStrictEqual(
      api?.validateControlLoopDefinition?.({
        ...loop,
        signals: {
          ...loop.signals,
          processVariable: { ...loop.signals.processVariable, unit: "degC" },
        },
      }, scope),
      { kind: "rejected", reason: "incompatible_units", signals: ["setpoint", "processVariable"] }
    );
  });

  test("accepts bounded experiment defaults and rejects ambiguous scripted excitation", () => {
    const api = loopApi();
    const loop = completeLoop();
    const configured = {
      ...loop,
      experiment: {
        execution: "control-studio",
        kind: "setpoint_step",
        amplitude: 5,
        durationSeconds: 120,
        seed: 20260824,
      },
    };
    assert.deepStrictEqual(api.decodeControlLoopDefinition(configured), { kind: "accepted", loop: configured });
    assert.deepStrictEqual(api.decodeControlLoopDefinition({
      ...loop,
      experiment: { execution: "project-scripted", kind: "load_step", amplitude: 5,
        durationSeconds: 120, seed: 20260824 },
    }), { kind: "rejected", reason: "malformed_loop" });
  });

  test("accepts one typed Boolean authority for a project-owned lesson sequence", () => {
    const api = loopApi();
    const loop = completeLoop();
    const scripted = {
      ...loop,
      controller: { ...loop.controller, type: "RouthComparison", design: "project" },
      experiment: {
        execution: "project-scripted", kind: "observation", amplitude: 0,
        durationSeconds: 40, seed: 20260829,
        activation: { symbol: "P1.ExperimentEnabled", address: "%MX96.0", iecType: "BOOL", direction: "write" },
      },
    };
    assert.deepStrictEqual(api.decodeControlLoopDefinition(scripted), { kind: "accepted", loop: scripted });
    assert.deepStrictEqual(api.decodeControlLoopDefinition({
      ...scripted,
      experiment: { ...scripted.experiment, activation: { ...scripted.experiment.activation, iecType: "LREAL" } },
    }), { kind: "rejected", reason: "malformed_loop" });
  });

  test("accepts one typed auxiliary signal set for MIMO, state, disturbance, and scheduling evidence", () => {
    const api = loopApi();
    const loop = completeLoop();
    const additional = [
      auxiliary("zone-b-sp", "zone-b", "SP", "%ML32", "degC", "read"),
      auxiliary("zone-b-pv", "zone-b", "PV", "%IL32", "degC", "read"),
      auxiliary("zone-b-mv", "zone-b", "MV", "%QL32", "%", "write"),
      auxiliary("jacket-state", "reactor", "state", "%IL40", "degC", "read"),
      auxiliary("feed-flow", "reactor", "disturbance", "%QL48", "kg/s", "write"),
      auxiliary("level-schedule", "tank", "schedule", "%IL56", "m", "read"),
      { ...auxiliary("zone-b-quality", "zone-b", "quality", "%IX64.0", "1", "read"), iecType: "BOOL" },
    ];
    const extended = { ...loop, signals: { ...loop.signals, additional } };
    let result: unknown = { kind: "missing_auxiliary_binding_support" };
    try {
      result = api.validateControlLoopDefinition(extended, {
        projectId: loop.projectId, runtimeId: loop.runtimeId,
      });
    } catch {}
    assert.deepStrictEqual(result, { kind: "accepted", loop: extended });
  });

  test("accepts a Boolean auxiliary state for controller mode and availability", () => {
    const api = loopApi();
    const loop = completeLoop();
    const automaticMode = {
      ...auxiliary("automatic-mode", "controller", "state", "%MX72.0", "1", "read"),
      iecType: "BOOL",
      range: { minimum: 0, maximum: 1 },
    };
    const extended = { ...loop, signals: { ...loop.signals, additional: [automaticMode] } };

    assert.deepStrictEqual(api.validateControlLoopDefinition(extended, {
      projectId: loop.projectId, runtimeId: loop.runtimeId,
    }), { kind: "accepted", loop: extended });
  });

  test("requires an explicit requested-output and retained-integral adapter before saturation analysis", () => {
    const api = loopApi();
    const loop = completeLoop();
    const additional = [
      auxiliary("requested-power", "requested-output", "state", "%ML32", "%", "read"),
      auxiliary("stored-correction", "retained-integral", "state", "%ML40", "%", "read"),
    ];
    const configured = { ...loop,
      controller: { ...loop.controller, saturationEvidence: {
        requestedOutputId: "requested-power", integralStateId: "stored-correction",
      } },
      signals: { ...loop.signals, additional },
    };
    const scope = { projectId: loop.projectId, runtimeId: loop.runtimeId };
    assert.deepStrictEqual(api.saturationWindupEvidence(configured), {
      requestedOutputId: "requested-power", integralStateId: "stored-correction",
    });
    assert.deepStrictEqual(api.validateControlLoopDefinition(configured, scope), { kind: "accepted", loop: configured });

    assert.deepStrictEqual(api.decodeControlLoopDefinition({ ...configured,
      controller: { ...configured.controller, saturationEvidence: {
        requestedOutputId: "stored-correction", integralStateId: "stored-correction",
      } },
    }), { kind: "rejected", reason: "malformed_loop" });
    for (const changed of [
      { requestedOutputId: "unknown", integralStateId: "stored-correction" },
      { requestedOutputId: "requested-power", integralStateId: "unknown" },
    ]) assert.deepStrictEqual(api.validateControlLoopDefinition({ ...configured,
      controller: { ...configured.controller, saturationEvidence: changed },
    }, scope), { kind: "rejected", reason: "incompatible_saturation_evidence" });
  });

  test("rejects overlapping addresses and duplicate role identities inside one channel", () => {
    const api = loopApi();
    const loop = completeLoop();
    const scope = { projectId: loop.projectId, runtimeId: loop.runtimeId };
    const overlapping = { ...loop, signals: { ...loop.signals, additional: [
      auxiliary("corrupt-pv", "zone-b", "PV", "%IL1", "degC", "read"),
    ] } };
    assert.deepStrictEqual(api.validateControlLoopDefinition(overlapping, scope), {
      kind: "rejected", reason: "ambiguous_signal_address", signals: ["processVariable", "corrupt-pv"],
    });

    const duplicateRole = { ...loop, signals: { ...loop.signals, additional: [
      auxiliary("zone-b-pv-1", "zone-b", "PV", "%IL32", "degC", "read"),
      auxiliary("zone-b-pv-2", "zone-b", "PV", "%IL40", "degC", "read"),
    ] } };
    assert.deepStrictEqual(api.validateControlLoopDefinition(duplicateRole, scope), {
      kind: "rejected", reason: "duplicate_signal_role", signal: "zone-b-pv-2",
    });

    const reservedIdentity = { ...loop, signals: { ...loop.signals, additional: [
      auxiliary("processVariable", "zone-b", "PV", "%IL32", "degC", "read"),
    ] } };
    assert.deepStrictEqual(api.validateControlLoopDefinition(reservedIdentity, scope), {
      kind: "rejected", reason: "duplicate_signal_id", signal: "processVariable",
    });
  });

  test("rejects non-address bindings and unknown sample timing before monitoring", () => {
    const api = loopApi();
    const loop = completeLoop();
    assert.deepStrictEqual(api.decodeControlLoopDefinition({
      ...loop, signals: { ...loop.signals, processVariable: {
        ...loop.signals.processVariable, address: "LevelPV",
      } },
    }), { kind: "rejected", reason: "malformed_loop" });
    assert.deepStrictEqual(api.decodeControlLoopDefinition({
      ...loop, observedSampleTiming: { nominalPeriodMs: 0 },
    }), { kind: "rejected", reason: "malformed_loop" });
  });
});

function loopApi(): any {
  try { return require("../../controlStudio/loopDefinition"); } catch { return {}; }
}

function auxiliary(id: string, channel: string, role: string, address: string, unit: string, direction: string) {
  return { id, channel, role, symbol: id, address, iecType: "LREAL", direction, unit,
    range: { minimum: -1000, maximum: 1000 } };
}

function completeLoop() {
  return {
    schemaVersion: 1 as const,
    id: "loop:tank",
    name: "Tank level",
    revision: "sha256:loop-1",
    projectId: "project:sha256:abc",
    runtimeId: "runtime:local",
    controller: {
      instance: "TankController",
      type: "Control.PID" as const,
      revision: "sha256:controller-1",
      action: "direct" as const,
      taskPeriodMs: 10,
      modeSignal: "%MX0.0",
      trackingSignal: "%ML24",
    },
    signals: {
      setpoint: { symbol: "LevelSP", address: "%ML0", iecType: "LREAL", direction: "read" as const, unit: "%", range: { minimum: 0, maximum: 100 } },
      processVariable: { symbol: "LevelPV", address: "%IL0", iecType: "LREAL", direction: "read" as const, unit: "%", range: { minimum: 0, maximum: 100 } },
      manipulatedVariable: { symbol: "ValveMV", address: "%QL0", iecType: "LREAL", direction: "write" as const, unit: "%", range: { minimum: 0, maximum: 100 } },
      actuator: { symbol: "ValvePosition", address: "%IL8", iecType: "LREAL", direction: "read" as const, unit: "%", range: { minimum: 0, maximum: 100 } },
    },
    limits: { outputMinimum: 0, outputMaximum: 100, rateUpPerS: 20, rateDownPerS: 25 },
    observedSampleTiming: { nominalPeriodMs: 10, maximumJitterMs: 1 },
    plantId: "tank",
    operatingPoint: "Normal production",
  };
}
