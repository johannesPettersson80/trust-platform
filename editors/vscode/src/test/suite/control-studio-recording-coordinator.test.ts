import * as assert from "assert";
import { executeRecordingCommand, RecordingCoordinator, type RecordingSession, type RecordingUpdate } from "../../controlStudio/recordingCoordinator";
import { RecordingStartUnconfirmed } from "../../controlStudio/recordingStartFailure";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import type { LoopRecording } from "../../controlStudio/loopMonitor";
import type { RecordingRecoveryOwner } from "../../controlStudio/runtimeRecording";

const signal = { symbol: "P1.Value", address: "%IL0", iecType: "LREAL", direction: "read" as const,
  unit: "%", range: { minimum: 0, maximum: 100 } };
const loop: ControlLoopDefinition = {
  schemaVersion: 2, id: "loop", name: "Loop", revision: "sha256:r1", projectId: "project", runtimeId: "simulator",
  controller: { instance: "P1", type: "Main", design: "project", revision: "sha256:p1", action: "direct", taskPeriodMs: 10 },
  signals: { setpoint: { ...signal, address: "%ML0" }, processVariable: signal, manipulatedVariable: { ...signal, address: "%QL0" } },
  limits: { outputMinimum: 0, outputMaximum: 100 }, observedSampleTiming: { nominalPeriodMs: 10 },
};
const recording: LoopRecording = { samples: [], rawSamples: [], evidence: {
  method: "runtime_scan_buffer", completeness: "incomplete", observedSampleCount: 0, missingScanRanges: [],
  droppedObservedSamples: 0, actualCycleTimingAvailable: false, freshnessAvailable: false, reason: "Stopped before data arrived.",
} };
const retainedRecording: LoopRecording = { samples: [], rawSamples: [], evidence: {
  method: "runtime_scan_buffer", completeness: "complete", observedSampleCount: 3, missingScanRanges: [],
  droppedObservedSamples: 0, actualCycleTimingAvailable: true, freshnessAvailable: false,
  reason: "Start acknowledgement unconfirmed; stopped data retained. Complete recording — runtime captured 3 scans.",
} };

function hooks() {
  const changes: boolean[] = []; const stopped: LoopRecording[] = []; const evidence: LoopRecording["evidence"][] = [];
  const updates: RecordingUpdate[] = [];
  let started = 0; let refreshed = 0;
  return {
    changes, stopped, evidence, updates,
    get started() { return started; }, get refreshed() { return refreshed; },
    value: {
      recordingChanged: (value: boolean) => { changes.push(value); },
      started: () => { started += 1; },
      stopped: (value: LoopRecording) => { stopped.push(value); },
      publish: async (value: RecordingUpdate) => {
        updates.push(value);
        if (!("state" in value)) evidence.push(value);
      },
      refresh: () => { refreshed += 1; },
    },
  };
}

suite("Control Studio recording coordination", () => {
  test("honors Stop while a recording start acknowledgement is pending", async () => {
    let acknowledge!: (session: RecordingSession) => void;
    let stops = 0;
    const coordinator = new RecordingCoordinator(() => new Promise(resolve => { acknowledge = resolve; }));
    const starting = coordinator.execute("start", loop);
    const stopping = coordinator.execute("stop", loop).catch(error => ({ kind: "lost-stop", reason: String(error) }));
    acknowledge({ loop, stop: async () => { stops += 1; return recording; } });
    await starting;
    assert.strictEqual((await stopping)?.kind, "stopped", "the user's Stop must not be lost during the start handshake");
    assert.strictEqual(stops, 1);
    assert.strictEqual(coordinator.recording, false);
  });

  test("reports the retained owner state when its panel is reopened", async () => {
    let acknowledge!: (session: RecordingSession) => void;
    const coordinator = new RecordingCoordinator(() => new Promise(resolve => { acknowledge = resolve; }));
    const starting = coordinator.execute("start", loop);
    assert.deepStrictEqual((coordinator as any).currentUpdate, {
      state: "starting", reason: "Starting recording…",
    });
    acknowledge({ loop, stop: async () => recording });
    await starting;
    assert.deepStrictEqual((coordinator as any).currentUpdate, {
      state: "recording", reason: "Recording at runtime — Start acknowledged; waiting for Stop to retain runtime scans.",
    });
  });

  test("rejects a stop for another runtime even if revision strings match", async () => {
    let stops = 0;
    const coordinator = new RecordingCoordinator(async () => ({ loop, stop: async () => { stops += 1; return recording; } }));
    await coordinator.execute("start", loop);
    await assert.rejects(() => coordinator.execute("stop", { ...loop, runtimeId: "another-device" }), /matching|scope/i);
    assert.strictEqual(stops, 0);
    assert.strictEqual(coordinator.recording, true);
  });

  test("retains actual stopped samples when Start acknowledgement is uncertain", async () => {
    let starts = 0; let retriedStops = 0;
    const session: RecordingSession = { loop, stop: async () => { retriedStops += 1; return recording; } };
    const uncertain = new RecordingStartUnconfirmed(session, retainedRecording,
      new Error("Start reply lost"));
    const coordinator = new RecordingCoordinator(async () => { starts += 1; throw uncertain; });
    const events = hooks();

    await executeRecordingCommand(coordinator, "start", loop, events.value);

    assert.strictEqual(starts, 1);
    assert.strictEqual(events.started, 0, "An uncertain acknowledgement is not a successful Start");
    assert.deepStrictEqual(events.stopped, [retainedRecording], "Keep the actual Stop result rather than a placeholder");
    assert.deepStrictEqual(events.evidence, [retainedRecording.evidence], "Publish the native sample count and completeness unchanged");
    assert.strictEqual(events.evidence[0].observedSampleCount, 3, "Never replace returned samples with zero evidence");
    assert.strictEqual(retriedStops, 0, "A Stop that already returned data must not be replayed");
    assert.strictEqual(coordinator.recording, false);
  });

  test("honors a pending Stop after an uncertain Start whose first Stop failed", async () => {
    let starts = 0; let retryStops = 0;
    const session: RecordingSession = { loop, stop: async () => { retryStops += 1; return retainedRecording; } };
    const uncertain = new RecordingStartUnconfirmed(session, undefined,
      new Error("Start reply lost"), new Error("first Stop reply lost"));
    const coordinator = new RecordingCoordinator(async () => { starts += 1; throw uncertain; });
    const startEvents = hooks(); const stopEvents = hooks();

    await Promise.all([
      executeRecordingCommand(coordinator, "start", loop, startEvents.value),
      executeRecordingCommand(coordinator, "stop", loop, stopEvents.value),
    ]);

    assert.strictEqual(starts, 1, "The pending Stop must not cause another Start");
    assert.strictEqual(startEvents.started, 0, "The lost acknowledgement must not be presented as a Start");
    assert.deepStrictEqual(startEvents.updates, [
      { state: "starting", reason: "Starting recording…" },
      { state: "unconfirmed", reason: uncertain.message },
    ], "A failed automatic Stop remains explicit without inventing a zero-sample record");
    assert.strictEqual(retryStops, 1, "The pending Stop retries the original owned session");
    assert.deepStrictEqual(stopEvents.stopped, [retainedRecording]);
    assert.deepStrictEqual(stopEvents.evidence, [retainedRecording.evidence]);
    assert.strictEqual(stopEvents.evidence[0].observedSampleCount, 3);
    assert.strictEqual(coordinator.recording, false, "Successful retry releases only the original owner");
  });

  test("publishes Starting then keeps one lost-acknowledgement owner through failed and incomplete Stop retry", async () => {
    const originalRecordingId = "recording-start-reply-lost";
    const incompleteRecording: LoopRecording = { samples: [], rawSamples: [], evidence: {
      method: "runtime_scan_buffer", completeness: "incomplete", observedSampleCount: 2,
      missingScanRanges: [{ first: 4, last: 4 }], droppedObservedSamples: 1,
      actualCycleTimingAvailable: true, freshnessAvailable: false,
      reason: "Runtime scan 4 was not retained.",
    } };
    let starts = 0;
    const stopIds = [originalRecordingId]; // The lost Start acknowledgement already attempted this owner's Stop once.
    let successfulStop = false;
    const session: RecordingSession = { loop, stop: async () => {
      stopIds.push(originalRecordingId);
      if (!successfulStop) throw new Error("first Stop reply lost");
      return incompleteRecording;
    } };
    const uncertain = new RecordingStartUnconfirmed(session, undefined,
      new Error("Start reply lost"), new Error("first Stop reply lost"));
    const coordinator = new RecordingCoordinator(async () => { starts += 1; throw uncertain; });
    const startEvents = hooks(); const failedStopEvents = hooks(); const completedStopEvents = hooks();

    await executeRecordingCommand(coordinator, "start", loop, startEvents.value);

    assert.deepStrictEqual(startEvents.updates, [
      { state: "starting", reason: "Starting recording…" },
      { state: "unconfirmed", reason: uncertain.message },
    ], "Manual recording must expose its pending Start before the lost acknowledgement recovery state.");
    assert.strictEqual(starts, 1);
    assert.strictEqual(coordinator.recording, true, "The original owner remains available for Stop.");
    assert.deepStrictEqual(stopIds, [originalRecordingId], "Do not replace or replay the uncertain Start.");

    await executeRecordingCommand(coordinator, "stop", loop, failedStopEvents.value);

    assert.strictEqual(coordinator.recording, true, "A failed Stop retains the original owner.");
    assert.deepStrictEqual(stopIds, [originalRecordingId, originalRecordingId]);
    assert.strictEqual(starts, 1, "Stop recovery must never issue a replacement Start.");

    successfulStop = true;
    await executeRecordingCommand(coordinator, "stop", loop, completedStopEvents.value);

    assert.deepStrictEqual(stopIds, [originalRecordingId, originalRecordingId, originalRecordingId]);
    assert.strictEqual(starts, 1);
    assert.deepStrictEqual(completedStopEvents.stopped, [incompleteRecording]);
    assert.deepStrictEqual(completedStopEvents.evidence, [incompleteRecording.evidence],
      "The successful Stop must publish the runtime's actual incomplete evidence.");
    assert.strictEqual(coordinator.recording, false);
  });

  test("does not let a completed unconfirmed record replace a later recording Stop", async () => {
    let starts = 0; let laterStops = 0;
    const staleSession: RecordingSession = { loop, stop: async () => recording };
    const laterRecording = { ...retainedRecording, evidence: { ...retainedRecording.evidence, observedSampleCount: 4 } };
    const laterSession: RecordingSession = { loop, stop: async () => { laterStops += 1; return laterRecording; } };
    const coordinator = new RecordingCoordinator(async () => {
      starts += 1;
      if (starts === 1) throw new RecordingStartUnconfirmed(staleSession, retainedRecording, new Error("first Start reply lost"));
      return laterSession;
    });

    const first = await coordinator.execute("start", loop);
    const second = await coordinator.execute("start", loop);
    const stopped = await coordinator.execute("stop", loop);

    assert.strictEqual(first?.kind, "start-unconfirmed");
    assert.strictEqual(second?.kind, "started");
    assert.strictEqual(stopped?.kind, "stopped");
    assert.strictEqual((stopped as Extract<typeof stopped, { kind: "stopped" }>).recording, laterRecording,
      "The next Stop must belong to the newly acknowledged session");
    assert.strictEqual(laterStops, 1);
  });

  test("persists an owner before Start and clears it only after terminal Stop evidence", async () => {
    const owner: RecordingRecoveryOwner = { schemaVersion: 1, recordingId: "owned-recording", projectId: loop.projectId,
      runtimeId: loop.runtimeId, loopId: loop.id, loopRevision: loop.revision, source: "Simulation",
      bindings: [{ id: "processVariable", address: "%IL0", iec_type: "LREAL" }] };
    const order: string[] = [];
    const coordinator = new RecordingCoordinator((async (_loop: ControlLoopDefinition, lifecycle: any) => {
      assert.strictEqual(typeof lifecycle?.persist, "function", "Coordinator must provide persistence before Start can dispatch.");
      await lifecycle.persist(owner); order.push("start");
      return { loop, stop: async () => { order.push("stop"); return recording; } };
    }) as any);
    const events = hooks();
    const lifecycle = {
      ...events.value,
      persistOwner: async (value: RecordingRecoveryOwner) => { assert.deepStrictEqual(value, owner); order.push("persist"); },
      clearOwner: async () => { order.push("clear"); },
    };
    await executeRecordingCommand(coordinator, "start", loop, lifecycle);
    assert.deepStrictEqual(order, ["persist", "start"]);
    await executeRecordingCommand(coordinator, "stop", loop, lifecycle);
    assert.deepStrictEqual(order, ["persist", "start", "stop", "clear"]);
  });

  test("adopts a recovered owner as unconfirmed and stops only that owner", async () => {
    let recoveredStops = 0; let freshStarts = 0;
    const recovered: RecordingSession = { loop, stop: async () => { recoveredStops += 1; return recording; } };
    const coordinator = new RecordingCoordinator(async () => { freshStarts += 1; return { loop, stop: async () => recording }; });
    assert.strictEqual(typeof (coordinator as any).adoptRecoveredOwner, "function",
      "A new extension host must adopt the persisted owner before it permits any recording action.");
    (coordinator as any).adoptRecoveredOwner(recovered, "Recording recovery unconfirmed — Stop the original runtime buffer to obtain terminal evidence.");
    assert.deepStrictEqual((coordinator as any).currentUpdate, {
      state: "unconfirmed", reason: "Recording recovery unconfirmed — Stop the original runtime buffer to obtain terminal evidence.",
    });
    assert.strictEqual(await coordinator.execute("start", loop), undefined, "Recovery blocks a replacement Start.");
    assert.strictEqual(freshStarts, 0);
    assert.strictEqual((await coordinator.execute("stop", loop))?.kind, "stopped");
    assert.strictEqual(recoveredStops, 1);
  });

  test("keeps an unavailable recovered owner unconfirmed and blocks a replacement Start", async () => {
    let starts = 0;
    const coordinator = new RecordingCoordinator(async () => { starts += 1; return { loop, stop: async () => recording }; });
    coordinator.adoptUnavailableRecovery(loop, "Recording recovery unconfirmed — Start the matching runtime before retrying Stop.");
    const events = hooks();
    await executeRecordingCommand(coordinator, "stop", loop, events.value);
    assert.deepStrictEqual(events.updates, [{ state: "unconfirmed", reason: "Recording recovery unconfirmed — Start the matching runtime before retrying Stop." }]);
    assert.strictEqual(await coordinator.execute("start", loop), undefined);
    assert.strictEqual(starts, 0);
  });

  test("reconstructs the same unavailable owner when its runtime becomes available", async () => {
    let recoveredStops = 0;
    const coordinator = new RecordingCoordinator(async () => {
      throw new Error("Recovery must not create a replacement Start.");
    });
    coordinator.adoptUnavailableRecovery(loop, "Recording recovery unconfirmed — Start the matching runtime before retrying Stop.");
    const recovered: RecordingSession = { loop, stop: async () => { recoveredStops += 1; return recording; } };

    assert.doesNotThrow(() => coordinator.adoptRecoveredOwner(recovered,
      "Recording recovery unconfirmed — Stop the original runtime buffer to obtain terminal evidence."),
    "Once the same runtime is available, retry must reconstruct only the retained owner.");
    assert.strictEqual((await coordinator.execute("stop", loop))?.kind, "stopped");
    assert.strictEqual(recoveredStops, 1);
    assert.strictEqual(coordinator.recording, false);
  });

  test("retains a recovered owner when terminal program identity evidence is rejected", async () => {
    let replacementStarts = 0;
    const coordinator = new RecordingCoordinator(async () => {
      replacementStarts += 1;
      throw new Error("Recovery must not authorize a replacement Start.");
    });
    coordinator.adoptRecoveredOwner({ loop, stop: async () => {
      throw new Error("The recording program identity does not match its bound program.");
    } }, "Recording recovery unconfirmed — Stop the original runtime buffer to obtain terminal evidence.");
    const events = hooks();
    let clears = 0;

    await executeRecordingCommand(coordinator, "stop", loop, {
      ...events.value,
      clearOwner: async () => { clears += 1; },
    });

    assert.strictEqual(clears, 0, "Rejected terminal identity evidence must retain the persisted owner.");
    assert.deepStrictEqual(events.updates, [{ state: "unconfirmed",
      reason: "Recording recovery unconfirmed — The recording program identity does not match its bound program." }]);
    assert.strictEqual(await coordinator.execute("start", loop), undefined, "Recovery must continue blocking a replacement Start.");
    assert.strictEqual(replacementStarts, 0);
    assert.strictEqual(coordinator.recording, true);
  });
});
