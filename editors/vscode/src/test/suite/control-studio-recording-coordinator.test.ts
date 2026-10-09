import * as assert from "assert";
import { RecordingCoordinator, type RecordingSession } from "../../controlStudio/recordingCoordinator";
import { RecordingStartUnconfirmed } from "../../controlStudio/recordingStartFailure";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import type { LoopRecording } from "../../controlStudio/loopMonitor";

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

  test("keeps a stopped recording before its runtime buffer is released", async () => {
    const order: string[] = [];
    const session: RecordingSession = { loop, stop: async () => { order.push("stop"); return recording; },
      release: async () => { order.push("release"); } };
    const coordinator = new RecordingCoordinator(async () => session);
    await coordinator.execute("start", loop);
    const stopped = await coordinator.execute("stop", loop, undefined, undefined, async () => { order.push("retain"); });
    assert.strictEqual(stopped?.kind, "stopped");
    assert.deepStrictEqual(order, ["stop", "retain", "release"]);
    assert.strictEqual(coordinator.recording, false);
  });

  test("a failed keep leaves the owner and its buffer in place, and the next Stop keeps and then releases", async () => {
    const order: string[] = [];
    const session: RecordingSession = { loop, stop: async () => { order.push("stop"); return recording; },
      release: async () => { order.push("release"); } };
    const coordinator = new RecordingCoordinator(async () => session);
    await coordinator.execute("start", loop);
    await assert.rejects(coordinator.execute("stop", loop, undefined, undefined, async () => { throw new Error("study store unavailable"); }),
      /study store unavailable/);
    assert.strictEqual(coordinator.recording, true, "the owner is kept for a retry");
    assert.deepStrictEqual(order, ["stop"], "nothing is released before the samples are kept");
    const retried = await coordinator.execute("stop", loop, undefined, undefined, async () => { order.push("retain"); });
    assert.strictEqual(retried?.kind, "stopped");
    assert.deepStrictEqual(order, ["stop", "stop", "retain", "release"]);
    assert.strictEqual(coordinator.recording, false);
  });
});
