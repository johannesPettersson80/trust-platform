import * as assert from "assert";

suite("Control Studio loop definition", () => {
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
