import * as assert from "assert";
import { RecordingCoordinator, type RecordingSession } from "../../controlStudio/recordingCoordinator";
import { restoreRecordingRecoveryOwner } from "../../controlStudio/recordingRecoveryRestore";
import { RuntimeRecordingSession, type RecordingRecoveryOwner } from "../../controlStudio/runtimeRecording";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";

const loop: ControlLoopDefinition = {
  schemaVersion: 2, id: "loop", name: "Loop", revision: "sha256:r1", projectId: "project", runtimeId: "simulator",
  controller: { instance: "P1", type: "Main", design: "project", revision: "sha256:p1", action: "direct", taskPeriodMs: 10 },
  signals: { processVariable: { symbol: "P1.PV", address: "%IL0", iecType: "LREAL", direction: "read", unit: "%", range: { minimum: 0, maximum: 100 } },
    manipulatedVariable: { symbol: "P1.MV", address: "%QL0", iecType: "LREAL", direction: "write", unit: "%", range: { minimum: 0, maximum: 100 } } },
  limits: { outputMinimum: 0, outputMaximum: 100 }, observedSampleTiming: { nominalPeriodMs: 10 },
};

suite("Control Studio recording recovery restore", () => {
  test("adopts only a matching persisted owner before the reopened panel enables actions", async () => {
    const owner: RecordingRecoveryOwner = { schemaVersion: 1, recordingId: "record", projectId: loop.projectId, runtimeId: loop.runtimeId,
      loopId: loop.id, loopRevision: loop.revision, source: "Simulation", bindings: [{ id: "pv", address: "%IL0", iec_type: "LREAL" }] };
    const store = new Map<string, unknown>([[`trust.controlStudio.recording.${loop.projectId}.${loop.id}`, owner]]);
    const context: any = { workspaceState: { get: (key: string) => store.get(key) } };
    const coordinator = new RecordingCoordinator(async () => { throw new Error("Recovered owner must block a new Start."); });
    const recovered: RecordingSession = { loop, stop: async () => { throw new Error("not part of restore"); } };
    const original = RuntimeRecordingSession.recover;
    (RuntimeRecordingSession as any).recover = async (actualLoop: ControlLoopDefinition, actualOwner: RecordingRecoveryOwner) => {
      assert.strictEqual(actualLoop, loop); assert.deepStrictEqual(actualOwner, owner); return recovered;
    };
    const activity: boolean[] = [];
    try { await restoreRecordingRecoveryOwner(context, loop, coordinator, active => activity.push(active)); }
    finally { (RuntimeRecordingSession as any).recover = original; }
    assert.deepStrictEqual(activity, [true]);
    assert.deepStrictEqual(coordinator.currentUpdate, {
      state: "unconfirmed", reason: "Recording recovery unconfirmed — Stop the original runtime buffer to obtain terminal evidence.",
    });
    assert.strictEqual(await coordinator.execute("start", loop), undefined);
  });

  test("retries an unavailable recovered owner against the same runtime without authorizing Start", async () => {
    const owner: RecordingRecoveryOwner = { schemaVersion: 1, recordingId: "record", projectId: loop.projectId, runtimeId: loop.runtimeId,
      loopId: loop.id, loopRevision: loop.revision, source: "Simulation", bindings: [{ id: "pv", address: "%IL0", iec_type: "LREAL" }] };
    const store = new Map<string, unknown>([[`trust.controlStudio.recording.${loop.projectId}.${loop.id}`, owner]]);
    const context: any = { workspaceState: { get: (key: string) => store.get(key) } };
    const coordinator = new RecordingCoordinator(async () => { throw new Error("Retry must not authorize a replacement Start."); });
    coordinator.adoptUnavailableRecovery(loop, "Recording recovery unconfirmed — Start the matching runtime before retrying Stop.");
    let stops = 0;
    const recovered: RecordingSession = { loop, stop: async () => { stops += 1; return { evidence: { method: "runtime_scan_buffer", completeness: "complete", observedSampleCount: 1,
      missingScanRanges: [], droppedObservedSamples: 0, actualCycleTimingAvailable: true, freshnessAvailable: true, reason: "Captured." } } as any; } };
    const original = RuntimeRecordingSession.recover;
    (RuntimeRecordingSession as any).recover = async () => recovered;
    const recovery = require("../../controlStudio/recordingRecoveryRestore");
    try {
      assert.strictEqual(typeof recovery.retryRecordingRecoveryOwner, "function",
        "Stop retry must reconstruct the persisted owner when the matching runtime is available again.");
      await recovery.retryRecordingRecoveryOwner(context, loop, coordinator, () => undefined);
      assert.match(coordinator.currentUpdate?.reason ?? "", /recovery unconfirmed/i);
      assert.strictEqual(stops, 0, "Reconstructing the owner must not invent a terminal Stop result.");
      const stopped = await coordinator.execute("stop", loop);
      assert.strictEqual(stopped?.kind, "stopped");
      assert.strictEqual(stops, 1, "Stop must use the reconstructed original owner.");
    } finally { (RuntimeRecordingSession as any).recover = original; }
  });
});
