import * as assert from "assert";

suite("Control Studio live loop monitor", () => {
  test("projects synchronized SP, PV, and MV from the existing runtime lifecycle snapshot", () => {
    const api = monitorApi();
    const monitor = new api.ControlLoopMonitor({ staleAfterMs: 500, recordingCapacity: 3 });
    monitor.startRecording();
    const sample = monitor.observe(loop(), lifecycle(41, "50", "21.5", "36"), 1_000);

    assert.deepStrictEqual(sample, {
      availability: "available",
      source: "Simulation",
      loopId: "loop:tank",
      loopRevision: "sha256:loop-1",
      controller: {
        instance: "TankController",
        revision: "sha256:controller-1",
        mode: "Auto",
      },
      scan: 41,
      observedAtMs: 1_000,
      setpoint: signal("LevelSP", "%ML0", 50, "%", "good"),
      processVariable: signal("LevelPV", "%IL0", 21.5, "%", "good"),
      manipulatedVariable: signal("ValveMV", "%QL0", 36, "%", "good"),
      outputLimits: { minimum: 0, maximum: 100 },
      rateLimits: { upPerSecond: 20, downPerSecond: 25 },
      saturation: "none",
      recording: true,
    });
    assert.deepStrictEqual(monitor.stopRecording(), [sample]);
  });

  test("clears current values for missing and stale scans and resumes on a new scan", () => {
    const api = monitorApi();
    const monitor = new api.ControlLoopMonitor({ staleAfterMs: 500, recordingCapacity: 3 });
    assert.strictEqual(monitor.observe(loop(), lifecycle(10, "50", "20", "30"), 1_000).availability, "available");
    assert.deepStrictEqual(
      monitor.observe(loop(), lifecycle(10, "50", "20", "30"), 1_501),
      {
        availability: "unavailable",
        source: "Simulation",
        loopId: "loop:tank",
        reason: "Runtime scan is stale",
      }
    );
    assert.strictEqual(monitor.observe(loop(), lifecycle(11, "50", "20.5", "31"), 1_510).availability, "available");

    const missing = lifecycle(12, "50", "20.5", "31");
    missing.ioState.inputs = [];
    assert.deepStrictEqual(monitor.observe(loop(), missing, 1_520), {
      availability: "unavailable",
      source: "Simulation",
      loopId: "loop:tank",
      reason: "Process variable %IL0 is missing from runtime scan 12",
    });
  });

  test("keeps bounded immutable recording samples and reports evidence-based health only", () => {
    const api = monitorApi();
    const monitor = new api.ControlLoopMonitor({ staleAfterMs: 500, recordingCapacity: 2 });
    monitor.startRecording();
    monitor.observe(loop(), lifecycle(1, "50", "45", "30"), 100);
    monitor.observe(loop(), lifecycle(2, "50", "48", "32"), 110);
    monitor.observe(loop(), lifecycle(3, "50", "49", "33"), 120);
    const recording = monitor.stopRecording();
    assert.deepStrictEqual(recording.map((sample: any) => sample.scan), [2, 3]);
    assert.deepStrictEqual(api.summarizeLoopHealth(recording), {
      sampleCount: 2,
      meanAbsoluteError: 1.5,
      saturationSamples: 0,
      timing: "Two samples are insufficient for oscillation or noise diagnostics.",
    });
    assert.ok(!JSON.stringify(api.summarizeLoopHealth(recording)).toLowerCase().includes("stiction"));
  });

  test("records each runtime scan once when refreshes repeat the same snapshot", () => {
    const monitor = new (monitorApi().ControlLoopMonitor)({ staleAfterMs: 500, recordingCapacity: 3 });
    monitor.startRecording();
    monitor.observe(loop(), lifecycle(7, "50", "48", "32"), 1_000);
    monitor.observe(loop(), lifecycle(7, "50", "48", "32"), 1_050);
    assert.deepStrictEqual(monitor.stopRecording().map((sample: any) => sample.scan), [7]);
  });

  test("uses strict session-scoped monitor and recording messages", () => {
    const messages = require("../../controlStudio/messages") as any;
    const sample = new (monitorApi().ControlLoopMonitor)({ staleAfterMs: 500, recordingCapacity: 2 })
      .observe(loop(), lifecycle(7, "50", "48", "32"), 1_000);
    const expected = {
      projectId: "project:sha256:abc",
      editorSessionId: "session-1",
      loopId: "loop:tank",
      loopRevision: "sha256:loop-1",
    };
    const envelope = messages.loopMonitorEnvelope(expected, "message-1", sample);
    assert.deepStrictEqual(messages.decodeLoopMonitorEnvelope(envelope, expected), envelope);
    assert.strictEqual(messages.decodeLoopMonitorEnvelope({ ...envelope, loopId: "loop:other" }, expected), undefined);

    const command = (kind: string) => ({
      schemaVersion: 1,
      kind,
      messageId: "command-1",
      projectId: expected.projectId,
      editorSessionId: expected.editorSessionId,
      loopId: expected.loopId,
      expectedLoopRevision: expected.loopRevision,
      payload: {},
    });
    assert.deepStrictEqual(
      messages.decodeLoopRecordingCommand(command("control-studio.recording.start"), expected),
      { action: "start", messageId: "command-1" }
    );
    assert.deepStrictEqual(
      messages.decodeLoopRecordingCommand(command("control-studio.recording.stop"), expected),
      { action: "stop", messageId: "command-1" }
    );
    assert.strictEqual(messages.decodeLoopRecordingCommand({ ...command("control-studio.recording.start"), extra: true }, expected), undefined);
  });
});

function monitorApi(): any {
  try { return require("../../controlStudio/loopMonitor"); } catch { return {}; }
}

function signal(symbol: string, address: string, value: number, unit: string, quality: string) {
  return { symbol, address, value, unit, quality };
}

function loop() {
  return {
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
    },
    signals: {
      setpoint: { symbol: "LevelSP", address: "%ML0", iecType: "LREAL", direction: "read", unit: "%", range: { minimum: 0, maximum: 100 } },
      processVariable: { symbol: "LevelPV", address: "%IL0", iecType: "LREAL", direction: "read", unit: "%", range: { minimum: 0, maximum: 100 } },
      manipulatedVariable: { symbol: "ValveMV", address: "%QL0", iecType: "LREAL", direction: "write", unit: "%", range: { minimum: 0, maximum: 100 } },
    },
    limits: { outputMinimum: 0, outputMaximum: 100, rateUpPerS: 20, rateDownPerS: 25 },
    observedSampleTiming: { nominalPeriodMs: 10 },
    plantId: "tank",
  };
}

function lifecycle(scan: number, sp: string, pv: string, mv: string) {
  return {
    status: {
      running: true,
      inlineValuesEnabled: true,
      runtimeMode: "simulate",
      runtimeState: "running",
      endpoint: "tcp://127.0.0.1:9900",
      endpointConfigured: true,
      endpointEnabled: true,
      endpointReachable: true,
    },
    ioState: {
      scan,
      inputs: [{ name: "LevelPV", address: "%IL0", valueType: "LREAL", value: pv }],
      outputs: [{ name: "ValveMV", address: "%QL0", valueType: "LREAL", value: mv }],
      memory: [
        { name: "LevelSP", address: "%ML0", valueType: "LREAL", value: sp },
        { name: "ModeAuto", address: "%MX0.0", valueType: "BOOL", value: "TRUE" },
      ],
    },
    starting: false,
  };
}
