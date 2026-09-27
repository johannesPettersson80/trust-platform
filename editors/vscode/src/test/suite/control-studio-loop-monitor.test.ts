import * as assert from "assert";

suite("Control Studio live loop monitor", () => {
  test("resolves controller mode by its declared auxiliary BOOL symbol", () => {
    const api = monitorApi();
    const configured: any = loop();
    configured.controller.modeSignal = "P1.Manual";
    configured.controller.modeWhenTrue = "Manual";
    configured.signals.additional = [{ id: "manual", channel: "manual", role: "state", symbol: "P1.Manual", address: "%MX0.0",
      iecType: "BOOL", direction: "read", unit: "", range: { minimum: 0, maximum: 1 } }];
    const monitor = new api.ControlLoopMonitor({ staleAfterMs: 500, recordingCapacity: 3 });
    assert.strictEqual(monitor.observe(configured, lifecycle(41, "50", "21.5", "36"), 1000).controller.mode, "Manual");
  });

  test("projects coherent readings and symbolic mode with the shared freshness and recording rules", () => {
    const api = monitorApi();
    const configured: any = loop();
    configured.controller.modeSignal = "Main.Manual";
    const monitor = new api.ControlLoopMonitor({ staleAfterMs: 500, recordingCapacity: 3 });
    assert.strictEqual(typeof monitor.observeReadings, "function", "Coherent symbolic readings need the same monitor projection as addressed scans");
    const readings = { running: true, scan: 88, controllerMode: "Manual",
      setpoint: signal("LevelSP", "%ML0", 50, "%", "good"),
      processVariable: signal("LevelPV", "%IL0", 21.5, "%", "good"),
      manipulatedVariable: signal("ValveMV", "%QL0", 36, "%", "good"), additional: [] };
    monitor.startRecording();
    const sample = monitor.observeReadings(configured, "Simulation", readings, 1000);
    assert.strictEqual(sample.controller.mode, "Manual");
    assert.strictEqual(sample.scan, 88);
    assert.strictEqual(monitor.observeReadings(configured, "Simulation", readings, 1100).availability, "available");
    assert.strictEqual(monitor.observeReadings(configured, "Simulation", readings, 1600).availability, "unavailable");
    assert.strictEqual(monitor.observeReadings(configured, "Simulation", { ...readings, scan: 89, controllerMode: "Auto" }, 1700).controller.mode, "Auto");
    assert.deepStrictEqual(monitor.stopRecording().samples.map((s: any) => s.scan), [88, 89]);
  });

  test("derives freshness from the declared loop sample period", () => {
    const api = monitorApi();
    assert.strictEqual(api.controlLoopStaleAfterMs({ observedSampleTiming: { nominalPeriodMs: 100 } }), 1_500);
    assert.strictEqual(api.controlLoopStaleAfterMs({ observedSampleTiming: { nominalPeriodMs: 60_000 } }), 300_000);
  });

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
    assert.deepStrictEqual(monitor.stopRecording().samples, [sample]);
  });

  test("projects a real open-loop input/output sample without a synthetic setpoint", () => {
    const api = monitorApi();
    const monitor = new api.ControlLoopMonitor({ staleAfterMs: 500, recordingCapacity: 3 });
    const openLoop: any = loop();
    openLoop.topology = "open-loop";
    openLoop.controller = { ...openLoop.controller, type: "OpenLoopExperiment", design: "project" };
    delete openLoop.signals.setpoint;

    const sample = monitor.observe(openLoop, lifecycle(41, "unused", "21.5", "36"), 1_000);

    assert.strictEqual(sample.availability, "available");
    assert.strictEqual(sample.setpoint, undefined);
    assert.deepStrictEqual(sample.processVariable, signal("LevelPV", "%IL0", 21.5, "%", "good"));
    assert.deepStrictEqual(sample.manipulatedVariable, signal("ValveMV", "%QL0", 36, "%", "good"));
    assert.deepStrictEqual(api.summarizeLoopHealth([sample]), {
      sampleCount: 1,
      meanAbsoluteError: null,
      saturationSamples: 0,
      timing: "One sample is insufficient for oscillation or noise diagnostics.",
    });
  });

  test("honors the declared meaning of a true controller-mode signal", () => {
    const api = monitorApi();
    const monitor = new api.ControlLoopMonitor({ staleAfterMs: 500, recordingCapacity: 3 });
    const manualModeLoop: any = loop();
    manualModeLoop.controller.modeWhenTrue = "Manual";

    const manual = monitor.observe(manualModeLoop, lifecycle(42, "50", "21.5", "36"), 1_000);
    assert.strictEqual(manual.availability, "available");
    assert.strictEqual(manual.controller.mode, "Manual");

    const automaticState = lifecycle(43, "50", "21.5", "36");
    automaticState.ioState.memory[1].value = "FALSE";
    const automatic = monitor.observe(manualModeLoop, automaticState, 1_010);
    assert.strictEqual(automatic.availability, "available");
    assert.strictEqual(automatic.controller.mode, "Auto");
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
    const recording = monitor.stopRecording().samples;
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
    assert.deepStrictEqual(monitor.stopRecording().samples.map((sample: any) => sample.scan), [7]);
  });

  test("reports exact missing scans when a 100 ms UI observes a 20 ms runtime", () => {
    const monitor = new (monitorApi().ControlLoopMonitor)({ staleAfterMs: 500, recordingCapacity: 20 });
    monitor.startRecording();
    monitor.observe(loop(), lifecycle(0, "50", "40", "30"), 1_000);
    monitor.observe(loop(), lifecycle(5, "50", "45", "35"), 1_100);
    monitor.observe(loop(), lifecycle(10, "50", "48", "38"), 1_200);

    const recording = monitor.stopRecording();
    assert.deepStrictEqual(recording.samples.map((sample: any) => sample.scan), [0, 5, 10]);
    assert.deepStrictEqual(recording.evidence, {
      method: "ui_observed_snapshots",
      completeness: "incomplete",
      observedSampleCount: 3,
      scanRange: { first: 0, last: 10 },
      missingScanRanges: [{ first: 1, last: 4 }, { first: 6, last: 9 }],
      droppedObservedSamples: 0,
      actualCycleTimingAvailable: false,
      freshnessAvailable: false,
      reason: "Incomplete recording — UI observed 3 of 11 scans; scans 1–4 and 6–9 were not acquired.",
    });
  });

  test("deduplicates a faster UI but never upgrades latest-snapshot fallback to complete", () => {
    const monitor = new (monitorApi().ControlLoopMonitor)({ staleAfterMs: 500, recordingCapacity: 20 });
    monitor.startRecording();
    for (const observedAtMs of [1_000, 1_020, 1_040, 1_060, 1_080]) {
      monitor.observe(loop(), lifecycle(0, "50", "40", "30"), observedAtMs);
    }
    monitor.observe(loop(), lifecycle(1, "50", "41", "31"), 1_100);
    for (const observedAtMs of [1_120, 1_140, 1_160, 1_180]) {
      monitor.observe(loop(), lifecycle(1, "50", "41", "31"), observedAtMs);
    }
    monitor.observe(loop(), lifecycle(2, "50", "42", "32"), 1_200);

    const recording = monitor.stopRecording();
    assert.deepStrictEqual(recording.samples.map((sample: any) => sample.scan), [0, 1, 2]);
    assert.deepStrictEqual(recording.evidence.missingScanRanges, []);
    assert.strictEqual(recording.evidence.completeness, "incomplete");
    assert.strictEqual(recording.evidence.actualCycleTimingAvailable, false);
    assert.strictEqual(recording.evidence.freshnessAvailable, false);
    assert.match(recording.evidence.reason, /UI-observed snapshots do not prove every runtime scan was acquired/);
  });

  test("bounds gap metadata to retained samples and reports gaps with capacity loss", () => {
    const monitor = new (monitorApi().ControlLoopMonitor)({ staleAfterMs: 500, recordingCapacity: 3 });
    monitor.startRecording();
    for (const [scan, observedAtMs] of [[0, 1_000], [2, 1_020], [4, 1_040], [6, 1_060], [8, 1_080]]) {
      monitor.observe(loop(), lifecycle(scan, "50", "40", "30"), observedAtMs);
    }

    const recording = monitor.stopRecording();
    assert.deepStrictEqual(recording.samples.map((sample: any) => sample.scan), [4, 6, 8]);
    assert.deepStrictEqual(recording.evidence.scanRange, { first: 4, last: 8 });
    assert.strictEqual(recording.evidence.observedSampleCount, 3);
    assert.deepStrictEqual(recording.evidence.missingScanRanges, [
      { first: 5, last: 5 },
      { first: 7, last: 7 },
    ]);
    assert.strictEqual(recording.evidence.droppedObservedSamples, 2);
    assert.match(recording.evidence.reason, /scans 5 and 7 were not acquired/);
    assert.match(recording.evidence.reason, /2 UI-observed samples were dropped by the bounded editor buffer/);
  });

  test("rejects an out-of-order runtime scan instead of corrupting the recording", () => {
    const monitor = new (monitorApi().ControlLoopMonitor)({ staleAfterMs: 500, recordingCapacity: 4 });
    monitor.startRecording();
    monitor.observe(loop(), lifecycle(10, "50", "48", "32"), 1_000);
    assert.deepStrictEqual(monitor.observe(loop(), lifecycle(9, "50", "47", "31"), 1_020), {
      availability: "unavailable",
      source: "Simulation",
      loopId: "loop:tank",
      reason: "Runtime scan moved backwards from 10 to 9",
    });
    monitor.observe(loop(), lifecycle(11, "50", "49", "33"), 1_040);
    assert.deepStrictEqual(monitor.stopRecording().samples.map((sample: any) => sample.scan), [10, 11]);
  });

  test("retains runtime point quality instead of silently promoting bad data", () => {
    const monitor = new (monitorApi().ControlLoopMonitor)({ staleAfterMs: 500, recordingCapacity: 2 });
    const state = lifecycle(8, "50", "48", "32");
    state.ioState.inputs[0] = { ...state.ioState.inputs[0], quality: "bad" } as any;
    const sample = monitor.observe(loop(), state, 1_000);
    assert.strictEqual(sample.availability, "available");
    assert.strictEqual(sample.processVariable.quality, "bad");
    const messages = require("../../controlStudio/messages") as any;
    const scope = { projectId: "project:sha256:abc", editorSessionId: "session-quality",
      loopId: "loop:tank", loopRevision: "sha256:loop-1" };
    const envelope = messages.loopMonitorEnvelope(scope, "quality-message", sample);
    assert.deepStrictEqual(messages.decodeLoopMonitorEnvelope(envelope, scope), envelope);
  });

  test("projects declared auxiliary signals from the same synchronized runtime scan", () => {
    const monitor = new (monitorApi().ControlLoopMonitor)({ staleAfterMs: 500, recordingCapacity: 2 });
    const configuredLoop: any = loop();
    configuredLoop.signals.additional = [
      { id: "feed-flow", channel: "reactor", role: "disturbance", symbol: "FeedFlow",
        address: "%QL8", iecType: "LREAL", direction: "write", unit: "%",
        range: { minimum: 0, maximum: 100 } },
      { id: "feed-quality", channel: "reactor", role: "quality", symbol: "FeedQuality",
        address: "%MX1.0", iecType: "BOOL", direction: "read", unit: "state" },
    ];
    const state = lifecycle(9, "50", "48", "32");
    state.ioState.outputs.push({ name: "FeedFlow", address: "%QL8", valueType: "LREAL", value: "27.5" });
    state.ioState.memory.push({ name: "FeedQuality", address: "%MX1.0", valueType: "BOOL", value: "1" });

    const sample = monitor.observe(configuredLoop, state, 1_000);

    assert.strictEqual(sample.availability, "available");
    assert.deepStrictEqual(sample.additional, [
      { id: "feed-flow", channel: "reactor", role: "disturbance", symbol: "FeedFlow", displayName: "Feed flow",
        address: "%QL8", value: 27.5, unit: "%", quality: "good" },
      { id: "feed-quality", channel: "reactor", role: "quality", symbol: "FeedQuality", displayName: "Feed quality",
        address: "%MX1.0", value: 1, unit: "state", quality: "good" },
    ]);
    const messages = require("../../controlStudio/messages") as any;
    const scope = { projectId: "project:sha256:abc", editorSessionId: "session-aux",
      loopId: "loop:tank", loopRevision: "sha256:loop-1" };
    const envelope = messages.loopMonitorEnvelope(scope, "aux-message", sample);
    assert.deepStrictEqual(messages.decodeLoopMonitorEnvelope(envelope, scope), envelope);
    assert.strictEqual(messages.decodeLoopMonitorEnvelope({
      ...envelope,
      payload: { ...sample, additional: [{ ...sample.additional[0], role: "invented" }] },
    }, scope), undefined, "auxiliary monitor roles must be decoded strictly");
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

  test("humanizes a lesson signal id for display while keeping the ST symbol available", () => {
    const api = monitorApi();
    assert.strictEqual(api.humanizeSignalIdentifier("door-open-indication"), "Door open indication");
    assert.strictEqual(api.humanizeSignalIdentifier("effective-surrounding-temperature"), "Effective surrounding temperature");
    assert.strictEqual(api.humanizeSignalIdentifier("feed_flow_rate"), "Feed flow rate");
    assert.strictEqual(api.humanizeSignalIdentifier("SETPOINT"), "Setpoint");
    assert.strictEqual(api.humanizeSignalIdentifier("gust"), "Gust");
    assert.strictEqual(api.humanizeSignalIdentifier(""), "");
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
