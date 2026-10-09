import * as assert from "assert";

suite("Control Studio Test recording owner", () => {
  test("starts and stops one runtime-owned Test recording without using the manual owner", async () => {
    const api = experimentRecordingApi();
    assert.strictEqual(typeof api.ExperimentRecordingOwner, "function",
      "Test needs a dedicated runtime recording owner");
    const loop = testLoop();
    const events: string[] = [];
    const recording = completeRecording();
    const owner = new api.ExperimentRecordingOwner(async (requestedLoop: any) => {
      events.push(`start:${requestedLoop.id}`);
      return { loop: requestedLoop, stop: async () => { events.push(`stop:${requestedLoop.id}`); return recording; } };
    });

    const start = await owner.start(loop);
    assert.deepStrictEqual(start, { kind: "started" });
    assert.strictEqual(owner.recording, true);
    assert.strictEqual(owner.loop, loop);
    const stop = await owner.stop(loop);
    assert.strictEqual(stop.kind, "stopped");
    assert.strictEqual(stop.recording, recording);
    assert.strictEqual(owner.recording, false);
    assert.deepStrictEqual(events, ["start:loop:test", "stop:loop:test"]);
  });

  test("does not turn an uncertain Start acknowledgement into Test authority", async () => {
    const api = experimentRecordingApi();
    assert.strictEqual(typeof api.ExperimentRecordingOwner, "function");
    const loop = testLoop();
    const owner = new api.ExperimentRecordingOwner(async () => {
      throw new Error("socket closed before the Start acknowledgement");
    });
    await assert.rejects(() => owner.start(loop), /socket closed/);
    assert.strictEqual(owner.recording, false);
  });

  test("starts runtime acquisition before excitation and stops it when excitation start fails", async () => {
    const api = experimentRecordingApi();
    assert.strictEqual(typeof api.startExperimentWithRecording, "function",
      "Test must acquire runtime evidence before it writes an excitation");
    const loop = testLoop();
    const events: string[] = [];
    const owner = new api.ExperimentRecordingOwner(async (requestedLoop: any) => {
      events.push("recording-start");
      return { loop: requestedLoop, stop: async () => { events.push("recording-stop"); return completeRecording(); } };
    });
    const journal = { persist: async () => undefined, clear: async () => undefined };
    await assert.rejects(() => api.startExperimentWithRecording(owner, loop, async () => {
      events.push("experiment-start");
      throw new Error("runtime rejected the reviewed test");
    }, journal), /runtime rejected/);
    assert.deepStrictEqual(events, ["recording-start", "experiment-start", "recording-stop"]);
    assert.strictEqual(owner.recording, false);
  });

  test("persists the acknowledged Test recording owner before excitation dispatch", async () => {
    const api = experimentRecordingApi();
    const loop = testLoop();
    const events: string[] = [];
    const recoveryOwner = { schemaVersion: 1, recordingId: "test-recording", projectId: loop.projectId,
      runtimeId: loop.runtimeId, loopId: loop.id, loopRevision: loop.revision, source: "Simulation",
      bindings: [{ id: "pv", address: "%IL0", iec_type: "LREAL" }] };
    const owner = new api.ExperimentRecordingOwner(async (requestedLoop: any) => {
      events.push("recording-start");
      return { loop: requestedLoop, recoveryOwner: () => recoveryOwner,
        stop: async () => { events.push("recording-stop"); return completeRecording(); } };
    });
    await api.startExperimentWithRecording(owner, loop, async () => { events.push("experiment-start"); return "started"; },
      { persist: async () => undefined, clear: async () => undefined },
      async (persisted: any) => { events.push(`persist:${persisted.recordingId}`); });
    assert.deepStrictEqual(events, ["recording-start", "persist:test-recording", "experiment-start"]);
  });

  test("adopts a recovered Test owner for original-buffer Stop only", async () => {
    const api = experimentRecordingApi();
    const loop = testLoop();
    const events: string[] = [];
    const owner = new api.ExperimentRecordingOwner(async () => { throw new Error("replacement Start must not run"); });
    const recording = completeRecording();
    owner.adoptRecoveredOwner({ loop, stop: async () => { events.push("stop:original"); return recording; } },
      "Test recording recovery unconfirmed — Stop the original buffer.");
    assert.strictEqual(owner.recording, true);
    await assert.rejects(() => owner.start(loop), /already active/i);
    const stopped = await owner.stop(loop);
    assert.strictEqual(stopped.recording, recording);
    assert.deepStrictEqual(events, ["stop:original"]);
  });

  test("reconstructs the persisted Test recording through a stop-only recovery callback", async () => {
    const api = experimentRecordingApi();
    assert.strictEqual(typeof api.restoreExperimentRecordingOwner, "function");
    const loop = testLoop();
    const recovered = { schemaVersion: 1, recordingId: "persisted-test-recording", projectId: loop.projectId,
      runtimeId: loop.runtimeId, loopId: loop.id, loopRevision: loop.revision, source: "Simulation",
      bindings: [{ id: "pv", address: "%IL0", iec_type: "LREAL" }] };
    const events: string[] = [];
    const owner = new api.ExperimentRecordingOwner(async () => { throw new Error("replacement Start must not run"); });
    await api.restoreExperimentRecordingOwner(owner, loop, recovered, async (exactLoop: any, exactOwner: any) => {
      events.push(`recover:${exactOwner.recordingId}`);
      assert.strictEqual(exactLoop, loop);
      return { loop, stop: async () => { events.push("stop:original"); return completeRecording(); } };
    });
    assert.strictEqual(owner.recording, true);
    await owner.stop(loop);
    assert.deepStrictEqual(events, ["recover:persisted-test-recording", "stop:original"]);
  });
});

function experimentRecordingApi(): any {
  try { return require("../../controlStudio/experimentRecording"); } catch { return {}; }
}

function testLoop(): any {
  return { projectId: "project:test", runtimeId: "simulation", id: "loop:test", revision: "sha256:test" };
}

function completeRecording(): any {
  return { samples: [], rawSamples: [], evidence: { method: "runtime_scan_buffer", completeness: "complete",
    observedSampleCount: 2, missingScanRanges: [], droppedObservedSamples: 0, actualCycleTimingAvailable: true,
    freshnessAvailable: true, reason: "Complete recording." } };
}
