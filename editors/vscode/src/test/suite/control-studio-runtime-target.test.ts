import * as assert from "assert";
import type { RuntimeLifecycleSnapshot } from "../../runtimeLifecycle";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { ControlLoopMonitor } from "../../controlStudio/loopMonitor";
import { SymbolicLoopObservation } from "../../controlStudio/symbolicLoopObservation";
import { RuntimeRecordingSession, recordPeriodExecutions } from "../../controlStudio/runtimeRecording";
import { RuntimeTransferClient, type TransferStatus } from "../../controlStudio/runtimeTransfer";
import { connectRuntimeSearch } from "../../controlStudio/runtimeSearchConnection";
import { simulationTarget } from "../../controlStudio/simulationTarget";
import { CONNECTION_LOSS_TIMEOUT_MS, connectionLossTimeoutMs } from "../../controlStudio/controlStudioLimits";
import { PhysicalApprovalRequired, liveWritePolicy, readRuntimeTarget, requireLiveWrite, runtimeTargetFromStatus } from "../../controlStudio/runtimeTarget";
import { RuntimeControlError } from "../../runtimeControlClient";

/** A loop whose process variable and output are program symbols with no I/O address (A5.1). */
function symbolOnlyLoop(): ControlLoopDefinition {
  const signal = (symbol: string) => ({ symbol, iecType: "REAL", direction: "read", unit: "%", range: { minimum: 0, maximum: 100 } });
  return { schemaVersion: 1, id: "heater", name: "Heater", revision: "sha256:loop", projectId: "project:heater", runtimeId: "simulator",
    controller: { instance: "Main.Controller", type: "Control.PID", design: "project", revision: "sha256:controller", action: "direct", taskPeriodMs: 100 },
    signals: { processVariable: signal("Main.Controller.Measurement"), manipulatedVariable: signal("Main.Controller.Output") },
    limits: { outputMinimum: 0, outputMaximum: 100 }, observedSampleTiming: { nominalPeriodMs: 100 } } as unknown as ControlLoopDefinition;
}

/** A running, simulated target; its cycle is given only when the runtime reports one. */
function lifecycle(cycleIntervalMs?: number): RuntimeLifecycleSnapshot {
  return { sessionId: "session-1", ioState: { inputs: [], outputs: [], memory: [] }, starting: false,
    status: { running: true, inlineValuesEnabled: false, runtimeMode: "online", runtimeState: "connected", simulated: true,
      ...(cycleIntervalMs === undefined ? {} : { cycleIntervalMs }), endpoint: "unix:///receipt-clock", endpointConfigured: true,
      endpointEnabled: true, endpointReachable: true } } as unknown as RuntimeLifecycleSnapshot;
}

/** A coherent symbolic inspection response that echoes the requested bindings. */
function inspection(params: any, scan: number, capturedAtMs: number,
  values: Record<string, number> = { "Main.Controller.Measurement": 35, "Main.Controller.Output": 40 }) {
  return { available: true, loop_revision: params.loop_revision, program_revision: "program", bindings: params.bindings, scan,
    runtime_time_ns: scan * 100_000_000, captured_at_ms: capturedAtMs,
    values: params.bindings.map((binding: { symbol?: string }) => values[binding.symbol ?? ""] ?? 0),
    forced: params.bindings.map(() => false), quality: params.bindings.map(() => "good") };
}

suite("Control Studio runtime target and live-write policy", () => {
  test("a target is simulated only when simulation is enabled and every attached I/O is simulated", () => {
    assert.strictEqual(runtimeTargetFromStatus({ simulation_enabled: true, simulation_only_io: true }).simulated, true);
    assert.strictEqual(runtimeTargetFromStatus({ simulation_enabled: true, simulation_only_io: false }).simulated, false,
      "a mix of simulated and physical I/O is physical");
    assert.strictEqual(runtimeTargetFromStatus({ simulation_enabled: true }).simulated, false,
      "a runtime that does not report simulation_only_io is physical");
    assert.strictEqual(runtimeTargetFromStatus({ simulation_enabled: false, simulation_only_io: true }).simulated, false);
    assert.strictEqual(runtimeTargetFromStatus(undefined).simulated, false);
    assert.strictEqual(simulationTarget({ simulated: false }), false, "no editor setting or file makes a target simulated");
    assert.strictEqual(simulationTarget({ simulated: true }), true);
  });

  test("reads the tasks, resource cycle, session epoch and time scale that the runtime reports", () => {
    assert.deepStrictEqual(runtimeTargetFromStatus({ simulation_enabled: true, simulation_only_io: true, simulation_time_scale: 10,
      session_epoch: "epoch-7", cycle_interval_ms: 10, tasks: [{ name: "MainTask", interval_ms: 100 }] }), {
      simulated: true, timeScale: 10, sessionEpoch: "epoch-7", tasks: [{ name: "MainTask", intervalMs: 100 }], cycleIntervalMs: 10 });
  });

  test("readRuntimeTarget asks the status operation and refuses a report that is not an object", async () => {
    const operations: string[] = [];
    const target = await readRuntimeTarget(async operation => {
      operations.push(operation);
      return { simulation_enabled: true, simulation_only_io: true };
    });
    assert.deepStrictEqual(operations, ["status"]);
    assert.strictEqual(target.simulated, true);
    await assert.rejects(readRuntimeTarget(async () => "not a report"), /invalid status report/);
  });

  test("a simulated live write needs no approval; a physical one needs approval for its own operation", () => {
    assert.deepStrictEqual(liveWritePolicy({ simulated: true }), { source: "simulation", approvalRequired: false });
    assert.deepStrictEqual(liveWritePolicy({ simulated: false }), { source: "physical", approvalRequired: true });
    assert.deepStrictEqual(requireLiveWrite({ source: "simulation" }, "experiment"), { source: "simulation" });
    assert.throws(() => requireLiveWrite({ source: "physical" }, "experiment"), (error: unknown) =>
      error instanceof PhysicalApprovalRequired && /experiment/.test(error.message) && /physical approval is required/i.test(error.message));
    assert.deepStrictEqual(requireLiveWrite({ source: "physical" }, "experiment", { operation: "experiment" }),
      { source: "physical", approval: { operation: "experiment" } });
  });

  test("the connection-loss timeout is five seconds, or three resource cycles when that is longer", () => {
    assert.strictEqual(CONNECTION_LOSS_TIMEOUT_MS, 5_000);
    assert.strictEqual(connectionLossTimeoutMs(), 5_000);
    assert.strictEqual(connectionLossTimeoutMs(10), 5_000);
    assert.strictEqual(connectionLossTimeoutMs(3_000), 9_000);
  });

  test("staleness runs on the receiving host's clock: a far capture time is fresh while scans arrive, and 5 s without one is stale", async function () {
    this.timeout(5_000);
    const connection = require("../../controlStudio/runtimeConnection");
    const original = connection.bindRuntimeConnection;
    let scan = 10;
    connection.bindRuntimeConnection = async () => ({ source: "simulation", endpoint: "unix:///receipt-clock",
      request: async (_operation: string, params: any) => inspection(params, scan, 5) });
    const clock = { now: 1_000_000 };
    const watch = new SymbolicLoopObservation(() => clock.now);
    const monitor = new ControlLoopMonitor({ staleAfterMs: 1_500, recordingCapacity: 10 });
    try {
      assert.strictEqual((await watch.observe(monitor, symbolOnlyLoop(), lifecycle(), clock.now)).availability, "available",
        "a capture time nearly 1000 s from this clock is not a reason to refuse the scan");
      clock.now += 4_000; scan = 11;
      assert.strictEqual((await watch.observe(monitor, symbolOnlyLoop(), lifecycle(), clock.now)).availability, "available");
      clock.now += 4_000; scan = 12;
      assert.strictEqual((await watch.observe(monitor, symbolOnlyLoop(), lifecycle(), clock.now)).availability, "available");
      clock.now += 4_999;
      assert.strictEqual((await watch.observe(monitor, symbolOnlyLoop(), lifecycle(), clock.now)).availability, "available",
        "under 5 s without a newer scan is not stale");
      clock.now += 2;
      const stale = await watch.observe(monitor, symbolOnlyLoop(), lifecycle(), clock.now);
      assert.strictEqual(stale.availability, "unavailable");
      assert.strictEqual(stale.availability === "unavailable" && stale.reason, "Runtime scan is stale");
    } finally { connection.bindRuntimeConnection = original; }
  });

  test("a long resource cycle lengthens the staleness limit to three cycles", async function () {
    this.timeout(5_000);
    const connection = require("../../controlStudio/runtimeConnection");
    const original = connection.bindRuntimeConnection;
    connection.bindRuntimeConnection = async () => ({ source: "simulation", endpoint: "unix:///slow-cycle",
      request: async (_operation: string, params: any) => inspection(params, 1, 1) });
    const clock = { now: 50_000 };
    const watch = new SymbolicLoopObservation(() => clock.now);
    const monitor = new ControlLoopMonitor({ staleAfterMs: 1_500, recordingCapacity: 10 });
    try {
      assert.strictEqual((await watch.observe(monitor, symbolOnlyLoop(), lifecycle(3_000), clock.now)).availability, "available");
      clock.now += 8_000;
      assert.strictEqual((await watch.observe(monitor, symbolOnlyLoop(), lifecycle(3_000), clock.now)).availability, "available",
        "8 s is inside three 3 s cycles");
      clock.now += 1_001;
      const stale = await watch.observe(monitor, symbolOnlyLoop(), lifecycle(3_000), clock.now);
      assert.strictEqual(stale.availability === "unavailable" && stale.reason, "Runtime scan is stale");
    } finally { connection.bindRuntimeConnection = original; }
  });

  test("a signal that has a symbol and no address is read by symbol and shown on the live chart", async function () {
    this.timeout(5_000);
    const connection = require("../../controlStudio/runtimeConnection");
    const original = connection.bindRuntimeConnection;
    connection.bindRuntimeConnection = async () => ({ source: "simulation", endpoint: "unix:///receipt-clock",
      request: async (_operation: string, params: any) => inspection(params, 7, 1_000,
        { "Main.Controller.Measurement": 42.5, "Main.Controller.Output": 12 }) });
    try {
      const observed = await new SymbolicLoopObservation(() => 2_000).observe(
        new ControlLoopMonitor({ staleAfterMs: 1_500, recordingCapacity: 10 }), symbolOnlyLoop(), lifecycle(), 2_000);
      assert.strictEqual(observed.availability, "available", JSON.stringify(observed));
      if (observed.availability !== "available") return;
      assert.strictEqual(observed.processVariable.value, 42.5);
      assert.strictEqual(observed.processVariable.symbol, "Main.Controller.Measurement");
      assert.strictEqual(observed.processVariable.address, undefined);
      assert.strictEqual(observed.manipulatedVariable.value, 12);
    } finally { connection.bindRuntimeConnection = original; }
  });

  test("a recording's record period keeps the planned duration within the sample limit", () => {
    assert.strictEqual(recordPeriodExecutions(6 * 3_600_000, 100), 3, "216,000 executions need a record period of 3");
    assert.strictEqual(recordPeriodExecutions(600_000, 100), 1, "6,000 executions need no averaging");
    assert.strictEqual(recordPeriodExecutions(10_000_000, 100), 1, "exactly 100,000 executions fit at k = 1");
    assert.strictEqual(recordPeriodExecutions(10_000_001, 100), 2);
  });

  test("a recording sends its record period, planned duration and sample limit to the runtime", async function () {
    this.timeout(5_000);
    const runtime = require("../../runtimeLifecycle").runtimeLifecycleService;
    const selected = require("../../selectedRuntime");
    const control = require("../../runtimeControlClient");
    const original = { snapshot: runtime.snapshot, token: runtime.controlAuthToken, selected: selected.getSelectedRuntimeId, send: control.sendRuntimeControlRequest };
    const starts: any[] = [];
    runtime.snapshot = async () => lifecycle();
    runtime.controlAuthToken = async () => "bound-token";
    selected.getSelectedRuntimeId = () => "simulator";
    control.sendRuntimeControlRequest = async (_endpoint: string, _token: string, operation: string, params: any) => {
      if (operation === "control.signals.inspect") return inspection(params, 1, 1_000);
      if (operation === "control.recording.start") { starts.push(params); return { status: "recording" }; }
      throw new Error(`unexpected ${operation}`);
    };
    try {
      await RuntimeRecordingSession.start(symbolOnlyLoop(), undefined, 6 * 3_600_000);
      assert.strictEqual(starts.length, 1);
      assert.strictEqual(starts[0].record_every, 3);
      assert.strictEqual(starts[0].maximum_duration_ms, 6 * 3_600_000);
      assert.strictEqual(starts[0].maximum_samples, 100_000);
    } finally {
      runtime.snapshot = original.snapshot; runtime.controlAuthToken = original.token;
      selected.getSelectedRuntimeId = original.selected; control.sendRuntimeControlRequest = original.send;
    }
  });

  test("a transfer sends the caller's source and approval, and a physical transfer without approval is never dispatched", async () => {
    const sent: { operation: string; params: any }[] = [];
    const client = new RuntimeTransferClient({} as any, "program", async (operation: string, params: any) => {
      sent.push({ operation, params });
      throw new RuntimeControlError("transfer refused before dispatch", "invalid_transfer_request");
    });
    const reviewed = { program_revision: "program", binding_revision: "binding", values: {}, status: "idle", reason: null, transaction: null } as unknown as TransferStatus;
    await assert.rejects(client.apply({}, true, 0, undefined, reviewed, { source: "physical", approval: { operation: "controller transfer" } }), /transfer refused/);
    assert.strictEqual(sent[0].operation, "control.transfer.apply");
    assert.strictEqual(sent[0].params.source, "physical");
    assert.strictEqual(sent[0].params.physical_approved, true);
    await assert.rejects(client.apply({}, true, 0, undefined, reviewed, { source: "simulation" }), /transfer refused/);
    assert.strictEqual(sent[1].params.source, "simulation");
    assert.strictEqual(sent[1].params.physical_approved, false);
    await assert.rejects(client.apply({}, true, 0, undefined, reviewed, { source: "physical" }),
      (error: unknown) => error instanceof PhysicalApprovalRequired);
    assert.strictEqual(sent.length, 2, "a physical write without approval never reaches the runtime");
  });

  test("an isolated search never runs on a physical target's host, whatever approval exists", async () => {
    const connection = require("../../controlStudio/runtimeConnection");
    const original = connection.bindRuntimeConnection;
    connection.bindRuntimeConnection = async () => ({ source: "physical", endpoint: "unix:///physical", request: async () => ({}) });
    try {
      await assert.rejects(connectRuntimeSearch(symbolOnlyLoop()), /A2\.6/);
    } finally { connection.bindRuntimeConnection = original; }
  });
});
