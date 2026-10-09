import * as assert from "assert";
import { ConnectedIdentification } from "../../controlStudio/connectedIdentification";
import { ExperimentRecordingOwner, startExperimentWithRecording } from "../../controlStudio/experimentRecording";
import { RuntimeRecordingSession } from "../../controlStudio/runtimeRecording";
import { assertSelectedRuntimeIsTarget } from "../../controlStudio/panelWorkflowSupport";
import { MainWorkflowError, classifyMainWorkflowFailure } from "../../controlStudio/mainWorkflowFailure";
import { RuntimeControlError } from "../../runtimeControlClient";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";

/** A loop with a 10 ms task and a 10 min maximum experiment: the fields a resumed campaign reads. */
function campaignLoop(): ControlLoopDefinition {
  return { id: "tank", name: "Tank", revision: "rev-1", projectId: "project", runtimeId: "simulator",
    experiment: { durationSeconds: 600 }, controller: { taskPeriodMs: 10, instance: "P1" },
    signals: { processVariable: { symbol: "P1.PV", unit: "%", range: { minimum: 0, maximum: 100 } },
      manipulatedVariable: { symbol: "P1.MV", unit: "%", range: { minimum: 0, maximum: 100 } }, additional: [] },
    limits: { outputMinimum: 0, outputMaximum: 100 } } as unknown as ControlLoopDefinition;
}

/** A journalled step whose experiment was dispatched: the state a reopened host finds. */
function journalledStep(owner: unknown) {
  return { schemaVersion: 1, loopRevision: "rev-1", phase: "step", recording: owner,
    plan: { id: "experiment-1", kind: "setpoint_step", trajectory: [{ atMs: 0, value: 1 }], durationMs: 600_000 },
    baseline: { mean: 0, noise: 0.1, halfRange: 0.1, spacingSeconds: 0.01, windowSeconds: 1 },
    stepStartSeconds: 0, channelsRemaining: 0 };
}

/** The host's ports and the mode double, with counters for every write that could reach the runtime or the journal. */
function hostPorts(journal: unknown, owner: unknown, counters: { cleared: number; modeRestored: number }) {
  const emptyPage = { state: "recording", elapsedMs: 0, sampleCount: 0, complete: true, reason: null, rawSamples: [], nextSample: 0 };
  const recorder = { follow: async () => emptyPage, stop: async () => ({ samples: [], rawSamples: [], evidence: {} }),
    recoveryOwner: () => owner };
  const ports = { sample: async () => ({}), recordingOwner: () => owner, persist: async () => undefined,
    retain: async () => undefined, clear: async () => { counters.cleared += 1; },
    campaign: { read: () => journal, write: async () => undefined },
    mode: { read: () => undefined, write: async () => undefined } };
  const options = { clock: { now: () => 0, wait: async () => undefined },
    createMode: () => ({ enter: async () => undefined, processNoise: 0,
      restore: async () => { counters.modeRestored += 1; } }) };
  return { ports, options, recorder };
}

suite("Control Studio client lifecycle", () => {
  test("closing releases the client without aborting the experiment or restoring the loop, and keeps the journal", async () => {
    const owner = { schemaVersion: 1, recordingId: "recording-1", projectId: "project", runtimeId: "simulator", loopId: "tank",
      loopRevision: "rev-1", source: "Simulation", bindings: [] };
    const journal = journalledStep(owner);
    const counters = { cleared: 0, modeRestored: 0 };
    const { ports, options, recorder } = hostPorts(journal, owner, counters);
    const operations: string[] = [];
    let identification: ConnectedIdentification | undefined;
    const request = async (operation: string) => {
      operations.push(operation);
      if (operation === "control.experiment.status") {
        identification?.dispose(); // the host closes while the experiment still runs
        return { status: "running", restoration: "pending" };
      }
      return {};
    };
    identification = new ConnectedIdentification(request as any, ports as any, options as any);
    await assert.rejects(identification.resume(campaignLoop(), recorder as any, async () => undefined), /follower was released/);
    assert.deepStrictEqual(operations.filter(operation => operation !== "control.experiment.status"), [],
      "closing sends no abort, finish or other write to the runtime");
    assert.ok(ports.campaign.read(), "the journal is kept for reopening");
    assert.strictEqual(counters.cleared, 0, "nothing clears the journal");
    assert.strictEqual(counters.modeRestored, 0, "the loop is left to the runtime and the journaled operation");
  });

  test("reopening a running campaign reattaches to its experiment and starts or changes nothing", async () => {
    const owner = { schemaVersion: 1, recordingId: "recording-1", projectId: "project", runtimeId: "simulator", loopId: "tank",
      loopRevision: "rev-1", source: "Simulation", bindings: [] };
    const counters = { cleared: 0, modeRestored: 0 };
    const { ports, options, recorder } = hostPorts(journalledStep(owner), owner, counters);
    const operations: string[] = [];
    const request = async (operation: string) => {
      operations.push(operation);
      return operation === "control.experiment.status" ? { status: "completed", restoration: "restored", stop_reason: "duration_reached" } : {};
    };
    const identification = new ConnectedIdentification(request as any, ports as any, options as any);
    await identification.resume(campaignLoop(), recorder as any, async () => undefined).catch(() => undefined);
    assert.ok(operations.includes("control.experiment.status"), "the journaled experiment is read by its identity");
    assert.deepStrictEqual(operations.filter(operation => operation !== "control.experiment.status"), [],
      "no experiment, transfer, parameter or recording write is sent on reopen");
    assert.strictEqual(counters.modeRestored, 1, "the mode the interrupted identification changed is returned once");
  });

  test("a start whose outcome is unknown is never replayed on reopen", async () => {
    const owner = { schemaVersion: 1, recordingId: "recording-1", projectId: "project", runtimeId: "simulator", loopId: "tank",
      loopRevision: "rev-1", source: "Simulation", bindings: [] };
    const counters = { cleared: 0, modeRestored: 0 };
    const { ports, options, recorder } = hostPorts(journalledStep(owner), owner, counters);
    const operations: string[] = [];
    const request = async (operation: string) => {
      operations.push(operation);
      if (operation === "control.experiment.status") throw new RuntimeControlError("No experiment holds this identifier.", "experiment_not_found");
      return {};
    };
    const identification = new ConnectedIdentification(request as any, ports as any, options as any);
    await assert.rejects(identification.resume(campaignLoop(), recorder as any, async () => undefined), /No experiment holds/);
    assert.strictEqual(operations.includes("control.experiment.start"), false, "reconciling by identity never sends a second start");
  });

  test("the owner receives the journal for the recording Start, and the experiment starts after its record is kept", async () => {
    const loop = campaignLoop();
    const events: string[] = [];
    const journal = { persist: async () => { events.push("journal"); }, clear: async () => undefined };
    const recordingOwner = { schemaVersion: 1, recordingId: "recording-1", projectId: "project", runtimeId: "simulator", loopId: "tank",
      loopRevision: "rev-1", source: "Simulation", bindings: [] };
    const owner = new ExperimentRecordingOwner(async (requested: any, persistence: any) => {
      events.push("recording-start");
      assert.strictEqual(persistence, journal, "the owner receives the journal it must write before Start");
      return { loop: requested, recoveryOwner: () => recordingOwner,
        stop: async () => ({ samples: [], rawSamples: [], evidence: {} }) } as any;
    });
    await startExperimentWithRecording(owner, loop, async () => { events.push("experiment-start"); return "started"; }, journal,
      async () => { events.push("experiment-record"); });
    assert.deepStrictEqual(events, ["recording-start", "experiment-record", "experiment-start"]);
  });

  test("no journal means no runtime request: a Test recording refuses before any Start is dispatched", async () => {
    const events: string[] = [];
    const owner = new ExperimentRecordingOwner(async () => { events.push("recording-start"); throw new Error("unreachable"); });
    await assert.rejects(() => startExperimentWithRecording(owner, campaignLoop(), async () => { events.push("experiment-start"); return 1; },
      undefined), /journal before Start/);
    assert.deepStrictEqual(events, []);
  });

  test("live actions refuse with connect-runtime while another runtime is selected, and never switch the study", () => {
    assert.doesNotThrow(() => assertSelectedRuntimeIsTarget("runtime-a", "runtime-a"));
    let refusal: unknown;
    try { assertSelectedRuntimeIsTarget("runtime-a", "runtime-b"); } catch (error) { refusal = error; }
    assert.ok(refusal instanceof MainWorkflowError, "the refusal is a typed workflow error");
    assert.strictEqual((refusal as Error).message,
      "Control Studio is connected to runtime-a; the selected runtime is runtime-b. Reconnect to continue.");
    assert.strictEqual(classifyMainWorkflowFailure(refusal).nextAction, "connect-runtime");
  });

  test("a stopped recording is kept before its runtime buffer is released", async () => {
    const signal = { id: "valid", address: "%MX16.0", iec_type: "BOOL" };
    const bindings = [{ id: "setpoint", address: "%ML0", iec_type: "LREAL" }, { id: "processVariable", address: "%IL0", iec_type: "LREAL" },
      { id: "manipulatedVariable", address: "%QL0", iec_type: "LREAL" }, signal];
    const sample = (scan: number) => ({ scan, runtime_time_ns: scan * 10_000_000, captured_at_ms: 1000 + scan,
      values: [1, 0.5, 2, 1], forced: [false, false, false, false] });
    const raw = { recording_id: "record", loop_revision: "rev-1", bindings: bindings.map(binding => ({ ...binding })), stop_reason: "stopped",
      samples: [sample(1), sample(2)], acquisition: { method: "runtime_scan_buffer", completeness: "complete", observedSampleCount: 2,
        scanRange: { first: 1, last: 2 }, missingScanRanges: [], droppedObservedSamples: 0, actualCycleTimingAvailable: true,
        freshnessAvailable: false, reason: "Captured every scan." } };
    const operations: string[] = [];
    const send = async (operation: string) => {
      operations.push(operation);
      return operation === "control.recording.stop" ? raw : { status: "released" };
    };
    const loop = campaignLoop();
    const Constructor = RuntimeRecordingSession as unknown as new (...args: unknown[]) => RuntimeRecordingSession;
    const session = new Constructor(loop, "record", bindings, "Simulation", send, "", undefined);
    const stopped = await session.stop();
    assert.strictEqual(stopped.rawSamples?.length, 2);
    assert.strictEqual((await session.stop()).rawSamples?.length, 2, "a repeated stop returns the same samples");
    assert.deepStrictEqual(operations, ["control.recording.stop"], "stop alone never frees the buffer");
    await session.release();
    assert.deepStrictEqual(operations, ["control.recording.stop", "control.recording.release"], "release follows the caller's store");
  });

  test("a recording at record period k is complete while every sample steps by k, and a missed execution makes it incomplete", async () => {
    const bindings = [{ id: "setpoint", address: "%ML0", iec_type: "LREAL" }, { id: "processVariable", address: "%IL0", iec_type: "LREAL" },
      { id: "manipulatedVariable", address: "%QL0", iec_type: "LREAL" }];
    const stopWith = async (sequences: number[]) => {
      const raw = { recording_id: "record", loop_revision: "rev-1", bindings: bindings.map(binding => ({ ...binding })), stop_reason: "stopped",
        samples: sequences.map(scan => ({ scan, runtime_time_ns: scan * 10_000_000, captured_at_ms: 1000 + scan, values: [1, 0.5, 2],
          forced: [false, false, false] })),
        acquisition: { method: "runtime_scan_buffer", completeness: "complete", observedSampleCount: sequences.length,
          scanRange: { first: sequences[0], last: sequences[sequences.length - 1] }, missingScanRanges: [], droppedObservedSamples: 0,
          actualCycleTimingAvailable: true, freshnessAvailable: false, reason: "Captured every execution." } };
      const send = async (operation: string) => operation === "control.recording.stop" ? raw : { status: "released" };
      const Constructor = RuntimeRecordingSession as unknown as new (...args: unknown[]) => RuntimeRecordingSession;
      return new Constructor(campaignLoop(), "record", bindings, "Simulation", send, "", undefined).stop();
    };
    assert.strictEqual((await stopWith([10, 20, 30, 40])).evidence.completeness, "complete", "every sample covers ten executions");
    assert.strictEqual((await stopWith([10, 20, 30, 35])).evidence.completeness, "complete", "the final sample may cover fewer executions");
    const missed = await stopWith([10, 20, 35, 40]);
    assert.strictEqual(missed.evidence.completeness, "incomplete", "a missed execution inside the record period is a gap");
    assert.match(missed.evidence.reason, /record period/);
  });

  test("release refuses a recording that has not been stopped", async () => {
    const send = async () => ({ status: "released" });
    const Constructor = RuntimeRecordingSession as unknown as new (...args: unknown[]) => RuntimeRecordingSession;
    const session = new Constructor(campaignLoop(), "record", [], "Simulation", send, "", undefined);
    await assert.rejects(() => session.release(), /Stop the recording before releasing it/);
  });

  test("runtime codes map to the typed action of the annex, by code and never by text", () => {
    const kind = (code: string) => classifyMainWorkflowFailure(new RuntimeControlError("A runtime message.", code)).nextAction;
    for (const code of ["experiment_active", "transaction_pending", "controller_write_pending", "experiment_state_unavailable", "stale_scan"]) {
      assert.strictEqual(kind(code), "wait", code);
    }
    for (const code of ["invalid_target", "invalid_address", "incompatible_binding", "invalid_experiment", "controller_unavailable"]) {
      assert.strictEqual(kind(code), "open-loop-setup", code);
    }
    assert.strictEqual(kind("outside_limits"), "open-limits");
    assert.strictEqual(kind("physical_approval_required"), "approve");
    for (const code of ["job_active", "out_of_limits", "io_unavailable"]) assert.strictEqual(kind(code), "none", code);
  });
});
