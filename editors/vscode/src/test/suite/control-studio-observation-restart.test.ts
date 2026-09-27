import * as assert from "assert";
import { ControlLoopMonitor } from "../../controlStudio/loopMonitor";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { SymbolicLoopObservation } from "../../controlStudio/symbolicLoopObservation";
import { RuntimeRecordingSession } from "../../controlStudio/runtimeRecording";
import type { RuntimeLifecycleSnapshot } from "../../runtimeLifecycle";

const connection = require("../../controlStudio/runtimeConnection");
const signal = (symbol: string, address: string, unit: string) => ({ symbol, address, unit, iecType: "LREAL" as const,
  direction: "read" as const, range: { minimum: 0, maximum: 100 } });
const loop: ControlLoopDefinition = {
  schemaVersion: 1, id: "heater", name: "Heater", revision: "sha256:loop", projectId: "project:heater", runtimeId: "simulator",
  controller: { instance: "P1.Controller", type: "Control.PID", design: "project", revision: "sha256:controller",
    action: "direct", taskPeriodMs: 100, modeSignal: "P1.AutoMode", modeWhenTrue: "Auto" },
  signals: { setpoint: signal("P1.SetpointC", "%ML0", "degC"), processVariable: signal("P1.TemperatureC", "%IL0", "degC"),
    manipulatedVariable: signal("P1.AppliedPowerPct", "%IL8", "%") },
  limits: { outputMinimum: 0, outputMaximum: 100 }, observedSampleTiming: { nominalPeriodMs: 100 },
};
function lifecycle(sessionId: string): RuntimeLifecycleSnapshot & { sessionId: string } {
  return { sessionId, status: { running: true, inlineValuesEnabled: false, runtimeMode: "simulate", runtimeState: "running",
    endpoint: "unix:///same-reused-path", endpointConfigured: true, endpointEnabled: true, endpointReachable: true },
    ioState: { inputs: [], outputs: [], memory: [] }, starting: false };
}
function response(params: any, revision: string, scan: number, pv = 35) {
  return { available: true, loop_revision: params.loop_revision, program_revision: revision, bindings: params.bindings,
    scan, runtime_time_ns: scan * 100_000_000, captured_at_ms: Date.now(),
    values: params.bindings.map((binding: { address?: string; iec_type: string }) => binding.address === "%IL0" ? pv
      : binding.iec_type === "BOOL" ? 0 : binding.address === "%ML0" ? 35 : 75),
    forced: params.bindings.map(() => false), quality: params.bindings.map(() => "good") };
}

suite("Control Studio observation after runtime restart", () => {
  test("starts one recording while live monitoring continues on the native single-watch selection", async function () {
    this.timeout(10000);
    const runtime = require("../../runtimeLifecycle").runtimeLifecycleService;
    const selected = require("../../selectedRuntime");
    const control = require("../../runtimeControlClient");
    const original = { bind: connection.bindRuntimeConnection, snapshot: runtime.snapshot,
      token: runtime.controlAuthToken, selected: selected.getSelectedRuntimeId, send: control.sendRuntimeControlRequest };
    const watchedLoop: ControlLoopDefinition = { ...loop, signals: { ...loop.signals,
      actuator: signal("P1.Command", "%QL0", "%"), additional: [
        { ...signal("P1.PreviousRequest", "%ML96", "%"), id: "previousRequest", channel: "primary", role: "state" },
      ] } };
    const snapshot = lifecycle("current-session");
    const watch = new SymbolicLoopObservation(), monitor = new ControlLoopMonitor({ staleAfterMs: 1500, recordingCapacity: 100 });
    let selection = "", scan = 0, starts = 0, liveReads = 0;
    const selectionKey = (bindings: any[]) => JSON.stringify(bindings.map(binding =>
      [binding.id, binding.address ?? null, binding.symbol ?? null, binding.iec_type]));
    const inspect = async (_operation: string, params: any) => {
      assert.strictEqual(params.loop_revision, loop.revision);
      if (params.expected_program_revision) assert.strictEqual(params.expected_program_revision, "program");
      const requested = selectionKey(params.bindings);
      if (selection !== requested) {
        selection = requested;
        return { available: false, program_revision: "program", loop_revision: params.loop_revision,
          bindings: params.bindings, reason: "Waiting for the next coherent runtime scan." };
      }
      // The next scan captures the currently selected watch, as SignalRegistry does.
      return response(params, "program", ++scan);
    };
    connection.bindRuntimeConnection = async () => ({ endpoint: snapshot.status.endpoint, request: inspect });
    runtime.snapshot = async () => snapshot;
    runtime.controlAuthToken = async () => "bound-token";
    selected.getSelectedRuntimeId = () => loop.runtimeId;
    control.sendRuntimeControlRequest = async (endpoint: string, token: string, operation: string, params: any) => {
      assert.strictEqual(endpoint, snapshot.status.endpoint); assert.strictEqual(token, "bound-token");
      // A normal panel refresh can occur between any two recording requests.
      const live = await watch.observe(monitor, watchedLoop, snapshot, Date.now());
      assert.strictEqual(live.availability, "available", JSON.stringify(live)); liveReads++;
      if (operation === "control.signals.inspect") return inspect(operation, params);
      assert.strictEqual(operation, "control.recording.start"); starts++;
      assert.strictEqual(params.program_revision, "program");
      assert.strictEqual(selectionKey(params.bindings), selection, "Start must use the currently captured selection");
      return { status: "recording" };
    };
    try {
      assert.strictEqual((await watch.observe(monitor, watchedLoop, snapshot, Date.now())).availability, "available");
      let failure: unknown;
      const session = await RuntimeRecordingSession.start(watchedLoop).catch(error => { failure = error; return undefined; });
      assert.ok(session, `Live monitoring must permit recording startup: ${String(failure)}`);
      assert.strictEqual(starts, 1, "monitor refreshes must not cause duplicate recording writes");
      assert.ok(liveReads >= 2);
      assert.strictEqual(session.recoveryOwner().programRevision, "program");
    } finally {
      connection.bindRuntimeConnection = original.bind; runtime.snapshot = original.snapshot;
      runtime.controlAuthToken = original.token; selected.getSelectedRuntimeId = original.selected;
      control.sendRuntimeControlRequest = original.send;
    }
  });
  test("waits for the first coherent watch capture before completing the initial observation", async () => {
    const previous = connection.bindRuntimeConnection;
    let reads = 0;
    connection.bindRuntimeConnection = async () => ({ source: "simulation", endpoint: "unix:///same-reused-path",
      request: async (_operation: string, params: any) => {
        if (++reads === 1) return { available: false, program_revision: "new", loop_revision: params.loop_revision,
          bindings: params.bindings, reason: "The first coherent watch capture is pending." };
        assert.strictEqual(params.expected_program_revision, "new");
        return response(params, "new", 1);
      } });
    try {
      const watch = new SymbolicLoopObservation(), monitor = new ControlLoopMonitor({ staleAfterMs: 1500, recordingCapacity: 100 });
      const observed = await watch.observe(monitor, loop, lifecycle("fresh-session"), Date.now());
      assert.strictEqual(observed.availability, "available", JSON.stringify(observed));
      assert.strictEqual(reads, 2);
    } finally { connection.bindRuntimeConnection = previous; }
  });
  test("a new confirmed debug session refreshes a cached program and scan baseline on the same endpoint", async () => {
    const previous = connection.bindRuntimeConnection;
    let revision = "old", scan = 1000, binds = 0;
    connection.bindRuntimeConnection = async () => {
      binds++;
      return { source: "simulation", endpoint: "unix:///same-reused-path", request: async (_operation: string, params: any) => {
        if (params.expected_program_revision && params.expected_program_revision !== revision) throw new Error("signals_unavailable: program revision changed");
        return response(params, revision, scan);
      } };
    };
    try {
      const watch = new SymbolicLoopObservation(), monitor = new ControlLoopMonitor({ staleAfterMs: 1500, recordingCapacity: 100 });
      assert.strictEqual((await watch.observe(monitor, loop, lifecycle("old-session"), Date.now())).availability, "available");
      revision = "new"; scan = 1;
      const fresh = await watch.observe(monitor, loop, lifecycle("new-session"), Date.now());
      assert.strictEqual(fresh.availability, "available", JSON.stringify(fresh));
      assert.ok(binds >= 2, "the new session must own a newly bound read connection");
      if (fresh.availability === "available") assert.strictEqual(fresh.scan, 1);
      scan = 0;
      assert.strictEqual((await watch.observe(monitor, loop, lifecycle("new-session"), Date.now())).availability, "unavailable",
        "a backward scan in the same session remains invalid");
    } finally { connection.bindRuntimeConnection = previous; }
  });

  test("a late old-session inspection cannot replace or poison the newer observation", async () => {
    const previous = connection.bindRuntimeConnection;
    let release!: () => void, requested!: () => void, bind = 0;
    const started = new Promise<void>(resolve => { requested = resolve; });
    connection.bindRuntimeConnection = async () => {
      const old = bind++ === 0;
      return { source: "simulation", endpoint: "unix:///same-reused-path", request: async (_operation: string, params: any) => {
        if (old) { requested(); await new Promise<void>(resolve => { release = resolve; }); }
        return response(params, old ? "old" : "new", old ? 1000 : 1, old ? 30 : 35);
      } };
    };
    try {
      const watch = new SymbolicLoopObservation(), monitor = new ControlLoopMonitor({ staleAfterMs: 1500, recordingCapacity: 100 });
      const old = watch.observe(monitor, loop, lifecycle("old-session"), Date.now());
      await started;
      const fresh = watch.observe(monitor, loop, lifecycle("new-session"), Date.now());
      release();
      const [oldResult, freshResult] = await Promise.all([old, fresh]);
      assert.strictEqual(oldResult.availability, "unavailable", "late old-session data must be discarded");
      assert.strictEqual(freshResult.availability, "available");
      if (freshResult.availability === "available") assert.strictEqual(freshResult.processVariable.value, 35);
      const next = await watch.observe(monitor, loop, lifecycle("new-session"), Date.now());
      assert.strictEqual(next.availability, "available", "late completion must not restore the old cached program");
    } finally { connection.bindRuntimeConnection = previous; }
  });
});
