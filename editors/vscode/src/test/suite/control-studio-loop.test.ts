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
});

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
