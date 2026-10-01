import * as assert from "assert";

suite("Control Studio loop signal directions", () => {
  test("accepts a project-scripted loop that only reads every primary and auxiliary signal behind its single activation authority", () => {
    const api = loopApi();
    const loop = projectScriptedLoop();
    const scope = { projectId: loop.projectId, runtimeId: loop.runtimeId };
    assert.deepStrictEqual(api.validateControlLoopDefinition(loop, scope), { kind: "accepted", loop });
  });

  test("rejects a project-scripted manipulated variable declared as a Control Studio write", () => {
    const api = loopApi();
    const loop = projectScriptedLoop();
    const scope = { projectId: loop.projectId, runtimeId: loop.runtimeId };
    const mvWrite = {
      ...loop,
      signals: {
        ...loop.signals,
        manipulatedVariable: { ...loop.signals.manipulatedVariable, direction: "write" as const },
      },
    };
    assert.deepStrictEqual(api.validateControlLoopDefinition(mvWrite, scope), {
      kind: "rejected", reason: "invalid_signal_direction", signal: "manipulatedVariable",
    });
  });

  test("rejects a project-scripted auxiliary signal declared as a write", () => {
    const api = loopApi();
    const loop = projectScriptedLoop();
    const scope = { projectId: loop.projectId, runtimeId: loop.runtimeId };
    const auxWrite = {
      ...loop,
      signals: {
        ...loop.signals,
        additional: [
          { ...loop.signals.additional[0], direction: "write" as const },
          loop.signals.additional[1],
        ],
      },
    };
    assert.deepStrictEqual(api.validateControlLoopDefinition(auxWrite, scope), {
      kind: "rejected", reason: "invalid_signal_direction", signal: "additional",
    });
  });

  test("accepts an observation-only project-scripted experiment without an activation authority", () => {
    const api = loopApi();
    const loop = projectScriptedLoop();
    const withoutActivation = {
      ...loop,
      experiment: {
        execution: "project-scripted" as const, kind: "observation" as const, amplitude: 0,
        durationSeconds: 40, seed: 20260829,
      },
    };
    assert.deepStrictEqual(api.decodeControlLoopDefinition(withoutActivation), { kind: "accepted", loop: withoutActivation });
  });

  test("treats a signal declared without a unit as dimensionless so recordings and requirements accept it", () => {
    const api = loopApi();
    const loop = projectScriptedLoop();
    const scope = { projectId: loop.projectId, runtimeId: loop.runtimeId };
    const unitless = { ...loop, signals: { ...loop.signals,
      additional: [loop.signals.additional[0], { ...loop.signals.additional[1], unit: "" }] } };
    const result = api.validateControlLoopDefinition(unitless, scope);
    assert.strictEqual(result.kind, "accepted");
    assert.strictEqual(result.loop.signals.additional[1].unit, "1", "A unitless Boolean or ratio signal is dimensionless");
  });

  test("keeps requiring a Control Studio-driven manipulated variable to stay writable", () => {
    const api = loopApi();
    const loop = controlStudioLoop();
    const scope = { projectId: loop.projectId, runtimeId: loop.runtimeId };
    const mvRead = {
      ...loop,
      signals: {
        ...loop.signals,
        manipulatedVariable: { ...loop.signals.manipulatedVariable, direction: "read" as const },
      },
    };
    assert.deepStrictEqual(api.validateControlLoopDefinition(mvRead, scope), {
      kind: "rejected", reason: "invalid_signal_direction", signal: "manipulatedVariable",
    });
    assert.deepStrictEqual(api.validateControlLoopDefinition(loop, scope), { kind: "accepted", loop });
  });
});

function loopApi(): any {
  try { return require("../../controlStudio/loopDefinition"); } catch { return {}; }
}

function projectScriptedLoop() {
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
    },
    signals: {
      setpoint: { symbol: "LevelSP", address: "%ML0", iecType: "LREAL", direction: "read" as const, unit: "%", range: { minimum: 0, maximum: 100 } },
      processVariable: { symbol: "LevelPV", address: "%IL0", iecType: "LREAL", direction: "read" as const, unit: "%", range: { minimum: 0, maximum: 100 } },
      manipulatedVariable: { symbol: "ValveMV", address: "%QL0", iecType: "LREAL", direction: "read" as const, unit: "%", range: { minimum: 0, maximum: 100 } },
      actuator: { symbol: "ValvePosition", address: "%IL8", iecType: "LREAL", direction: "read" as const, unit: "%", range: { minimum: 0, maximum: 100 } },
      additional: [
        { id: "zone-b-mv", channel: "zone-b", role: "MV" as const, symbol: "ZoneBMV", address: "%QL32", iecType: "LREAL", direction: "read" as const, unit: "%", range: { minimum: 0, maximum: 100 } },
        { id: "automatic-mode", channel: "controller", role: "state" as const, symbol: "AutomaticMode", address: "%IX80.0", iecType: "BOOL", direction: "read" as const, unit: "1", range: { minimum: 0, maximum: 1 } },
      ],
    },
    limits: { outputMinimum: 0, outputMaximum: 100 },
    observedSampleTiming: { nominalPeriodMs: 10 },
    experiment: {
      execution: "project-scripted" as const,
      kind: "observation" as const,
      amplitude: 0,
      durationSeconds: 40,
      seed: 20260829,
      activation: {
        symbol: "P1.ExperimentEnabled", address: "%MX72.0", iecType: "BOOL" as const, direction: "write" as const,
      },
    },
  };
}

function controlStudioLoop() {
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
    },
    signals: {
      setpoint: { symbol: "LevelSP", address: "%ML0", iecType: "LREAL", direction: "read" as const, unit: "%", range: { minimum: 0, maximum: 100 } },
      processVariable: { symbol: "LevelPV", address: "%IL0", iecType: "LREAL", direction: "read" as const, unit: "%", range: { minimum: 0, maximum: 100 } },
      manipulatedVariable: { symbol: "ValveMV", address: "%QL0", iecType: "LREAL", direction: "write" as const, unit: "%", range: { minimum: 0, maximum: 100 } },
    },
    limits: { outputMinimum: 0, outputMaximum: 100 },
    observedSampleTiming: { nominalPeriodMs: 10 },
  };
}
