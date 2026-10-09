import * as assert from "assert";

import { channel, pidInitialization, pidLoop } from "./control-studio-pid-fixtures";
import { EngineerWorkflowSession } from "../../controlStudio/engineerWorkflowSession";
import { ObservationDetached, ObservationNotHeld, ObservationRequirementFailed, PermittedRangeLeft, observationInProgress, resumeApplyObservation,
  stopRecovered } from "../../controlStudio/applyObservation";
import { pidTransferBinding } from "../../controlStudio/pidTransfer";
import { RuntimeRecordingSession } from "../../controlStudio/runtimeRecording";
import { RuntimeControlError } from "../../runtimeControlClient";

/** The loop of the resumed observation: the measured output's permitted range is 0 degC..10 degC (B6.3). */
const LOOP: any = { signals: { processVariable: { symbol: "P1.RoomC", unit: "degC", range: { minimum: 0, maximum: 10 } } } };
/** The requirements of the study: 4 s of test duration, a 0.1 maximum error, settled after 2 s (B6.3). */
const TASK: any = { referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 };
/** The criteria of a response that holds the target 5, without model-derived observations (B6.3). */
const CRITERIA: any = { target: 5, durationSeconds: 4, settlingTimeSeconds: 1, maximumError: 0.1, taskPeriodMs: 500,
  processRange: { minimum: 0, maximum: 10 }, outputLimits: { minimum: -20, maximum: 20 } };
/** The journalled observation of an Apply: its recording identity and its planned duration (A8.5). */
const CHECKPOINT: any = { recording: { recordingId: "r1" }, observation: { startedAtProcessNs: 1, durationMs: 4000 } };

function singleOutputModel(): any {
  return { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0, revision: "first-order", source: "provided",
    validation: "validated", evidence: ["bench"], normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")],
    operatingPoint: { inputs: [0], outputs: [2] } };
}

/** A recorded response that holds the target: nine scans over 4 s, all in Auto and inside every bound (B6.3). */
function holdingResponse(pv: number): any[] {
  return Array.from({ length: 9 }, (_, scan) => ({ scan, elapsedSeconds: scan * 0.5, controllerMode: "Auto", sp: 5, pv, appliedMv: 2,
    quality: "good", units: { sp: "degC", pv: "degC", mv: "V" } }));
}

/** The completed recording of a stopped response: its samples and its complete evidence (A8.1). */
function completeRecording(rawSamples: any[]): any {
  return { rawSamples, samples: [], evidence: { completeness: "complete" } };
}

/** One page of a running recording, as the runtime follows it (runtimeRecording.ts). */
function page(state: string, elapsedMs: number, rawSamples: any[]): any {
  return { state, elapsedMs, sampleCount: rawSamples.length, complete: state === "stopped", reason: null, rawSamples, nextSample: rawSamples.length };
}

/** A follower that answers its pages in turn, then its stopped recording; every runtime call is recorded in order. */
function followerOf(pages: any[], stopped: any, calls: string[]): any {
  let next = 0;
  return {
    follow: async () => { calls.push("follow"); return pages[Math.min(next++, pages.length - 1)]; },
    stop: async () => { calls.push("stop"); return stopped; },
    release: async () => { calls.push("release"); },
  };
}

/** A commissioning checkpoint as the journal writes it: the observation is journalled only once it has begun (A8.5, B6.3). */
function journalled(stage: string, observed: boolean, recorded = true): any {
  const loop: any = pidLoop();
  // Auto commissioning of a PI or PID loop needs its typed initialization adapter (Specification 35, C0 Missing adapters).
  loop.controller.pidInitialization = pidInitialization();
  return { id: "checkpoint-1", loopRevision: loop.revision, stage, model: singleOutputModel(), task: TASK,
    origin: { program_revision: "program", binding_revision: "parameters", active_revision: "original", previous_revision: null, status: "applied",
      transaction_id: null, reason: null, supports_request_id: true, values: { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 }, previous_values: null },
    candidate: { "Main.Kp": 2, "Main.Ki": 0.5, "Main.Kd": 0 }, parameterOperation: { id: "apply-1", program: "program", binding: "parameters" },
    references: { "Main.SP": 2 }, binding: pidTransferBinding(loop), recoveryTracking: {}, tolerance: 0.1,
    ...(recorded ? { recording: { schemaVersion: 1, recordingId: "r1", projectId: loop.projectId, runtimeId: loop.runtimeId, loopId: loop.id,
      loopRevision: loop.revision, source: "Simulation", bindings: [] } } : {}),
    ...(observed ? { observation: { startedAtProcessNs: 1, durationMs: 4000 } } : {}) };
}

/** An engineer session that holds the journalled checkpoint; its journal writes and retained recordings are recorded. */
function sessionWith(checkpoint: unknown, writes: unknown[], retained: unknown[]): EngineerWorkflowSession {
  const loop: any = pidLoop();
  loop.controller.pidInitialization = pidInitialization();
  loop.signals.processVariable.range = { minimum: 0, maximum: 10 };
  const session = new EngineerWorkflowSession(loop, () => singleOutputModel(), () => undefined,
    async recording => { retained.push(recording); }, undefined, undefined, undefined, undefined,
    async written => { writes.push(written); });
  (session as any).state.task = TASK;
  session.restoreCommissioningCheckpoint(checkpoint as never);
  return session;
}

/** Patches the runtime's recovery of a recording by its identity to answer with the follower, and restores it afterwards. */
async function withRecovery(follower: unknown, body: () => Promise<void>): Promise<void> {
  const sessions: any = RuntimeRecordingSession;
  const saved = sessions.recover;
  sessions.recover = async () => follower;
  try { await body(); }
  finally { sessions.recover = saved; }
}

/** Waits, in small steps, until a condition the fake follower reaches holds. */
async function until(condition: () => boolean): Promise<void> {
  for (let waited = 0; !condition(); waited += 5) {
    if (waited > 2000) throw new Error("the condition did not hold in time");
    await new Promise(resolve => setTimeout(resolve, 5));
  }
}

suite("reattached Apply observation (A8.5, A8.6, B6.3, A10.4)", () => {
  suite("the resumed recording (B6.3, A8.5)", () => {
    test("an observation is in progress only while its checkpoint journals both the observation and the recording identity (A8.5)", () => {
      assert.strictEqual(observationInProgress(journalled("recording", true)), true);
      assert.strictEqual(observationInProgress(journalled("transfer", true)), true);
      assert.strictEqual(observationInProgress(journalled("transfer", false)), false, "before the observation starts, the reviewed recovery applies");
      assert.strictEqual(observationInProgress(journalled("recording", true, false)), false, "without a recording identity nothing can be reattached");
      assert.strictEqual(observationInProgress(journalled("completed", true)), false, "a completed observation is not followed again");
    });

    test("reopening follows the held recording, stores it before its buffer is released, and returns the response it assesses (B6.3, A8.5)", async () => {
      const calls: string[] = [];
      const samples = holdingResponse(5);
      const stopped = completeRecording(samples);
      const follower = followerOf([page("stopped", 4000, samples)], stopped, calls);
      const resumed = await resumeApplyObservation(LOOP, CHECKPOINT, CRITERIA, {
        open: async () => follower,
        retain: async () => { calls.push("retain"); },
      });
      assert.strictEqual(resumed.recording, stopped);
      assert.strictEqual(resumed.maximumError, 0, "the response holds the target");
      assert.deepStrictEqual(calls, ["follow", "stop", "retain", "release"]);
    });

    test("a failed requirement stores the response before it is raised, so Restore previous settings can be offered (B6.3)", async () => {
      const calls: string[] = [];
      const samples = holdingResponse(5);
      const stopped = { rawSamples: samples, samples: [], evidence: { completeness: "incomplete", reason: "fake recording" } };
      const follower = followerOf([page("stopped", 4000, samples)], stopped, calls);
      await assert.rejects(() => resumeApplyObservation(LOOP, CHECKPOINT, CRITERIA, { open: async () => follower, retain: async () => { calls.push("retain"); } }),
        (error: unknown) => error instanceof ObservationRequirementFailed && Object.is(error.recording, stopped)
          && error.failure.cause === "settings" && error.failure.nextAction === "restore");
      assert.deepStrictEqual(calls, ["follow", "stop", "retain", "release"]);
    });

    test("a departure from the permitted range stores the samples the runtime holds, then raises the departure (B6.3, B6.7)", async () => {
      const calls: string[] = [];
      const samples = [{ scan: 0, elapsedSeconds: 0, controllerMode: "Auto", sp: 5, pv: 12.5, appliedMv: 2, quality: "good" }];
      const stopped = { rawSamples: samples, samples: [], evidence: { completeness: "incomplete", reason: "fake recording" } };
      const follower = followerOf([page("recording", 0, samples)], stopped, calls);
      await assert.rejects(() => resumeApplyObservation(LOOP, CHECKPOINT, CRITERIA, { open: async () => follower, retain: async () => { calls.push("retain"); } }),
        (error: unknown) => error instanceof PermittedRangeLeft && Object.is(error.recording, stopped)
          && error.message === "P1.RoomC left its permitted range (12.5 °C outside 0 °C..10 °C)" && error.failure.nextAction === "none");
      assert.deepStrictEqual(calls, ["follow", "stop", "retain", "release"], "the samples are stored before the caller restores the settings");
    });

    test("a recording the runtime no longer holds is reported as not held, with nothing stored or released (A8.6)", async () => {
      const calls: string[] = [];
      const follower: any = {
        follow: async () => { calls.push("follow"); throw new RuntimeControlError("Recording identity is unavailable.", "recording_unavailable"); },
        stop: async () => { calls.push("stop"); return undefined; },
        release: async () => { calls.push("release"); },
      };
      await assert.rejects(() => resumeApplyObservation(LOOP, CHECKPOINT, CRITERIA, { open: async () => follower, retain: async () => { calls.push("retain"); } }),
        (error: unknown) => error instanceof ObservationNotHeld && error.failure.nextAction === "restore" && /no longer holds the recording/.test(error.message));
      assert.deepStrictEqual(calls, ["follow"]);
    });

    test("a retained recording the runtime no longer holds is stored as incomplete evidence, so the recovery goes on (A8.6)", async () => {
      const outcome = await stopRecovered({ stop: async () => { throw new RuntimeControlError("Recording identity is unavailable.", "recording_unavailable"); } } as any);
      assert.strictEqual(outcome.held, false);
      assert.strictEqual(outcome.recording.evidence.completeness, "incomplete");
      assert.deepStrictEqual(outcome.recording.samples, []);
      await assert.rejects(() => stopRecovered({ stop: async () => { throw new Error("transport lost"); } } as any), /transport lost/,
        "any other failure still fails the recovery");
    });

    test("closing the panel detaches a reattached observation: it stops following and touches nothing in the runtime (A10.4)", async () => {
      const calls: string[] = [];
      let closed = false;
      const follower: any = {
        follow: async () => { calls.push("follow"); closed = true; return page("recording", 100, []); },
        stop: async () => { calls.push("stop"); return undefined; },
        release: async () => { calls.push("release"); },
      };
      await assert.rejects(() => resumeApplyObservation(LOOP, CHECKPOINT, CRITERIA, {
        open: async () => follower, retain: async () => { calls.push("retain"); }, detached: () => closed }),
        (error: unknown) => error instanceof ObservationDetached);
      assert.deepStrictEqual(calls, ["follow"], "the runtime keeps recording: nothing is stopped, stored, released or restored");
    });

    test("the reattached observation reports the process time elapsed and remaining while its recording runs (B6.3)", async () => {
      const texts: string[] = [];
      const samples = holdingResponse(5);
      const follower = followerOf([page("recording", 1000, []), page("stopped", 4000, samples)], completeRecording(samples), []);
      await resumeApplyObservation(LOOP, CHECKPOINT, CRITERIA, { open: async () => follower, retain: async () => undefined,
        progress: text => { texts.push(text); } });
      assert.deepStrictEqual(texts, ["Recording the response: 1 s of 4 s elapsed, 3 s of process time remaining."]);
    });
  });

  suite("reopening the engineer session (A8.6, B6.3, A10.4)", () => {
    test("a journalled Apply whose observation never started is not reattached: the reviewed recovery path keeps it (A8.6)", async () => {
      const writes: unknown[] = [], retained: unknown[] = [];
      await withRecovery({ follow: async () => { throw new Error("nothing is followed before the observation starts"); } }, async () => {
        const session = sessionWith(journalled("transfer", false), writes, retained);
        assert.strictEqual(await session.recoverApplyObservation(async () => undefined), false);
        assert.strictEqual((session.restoredOwnerView() as any)?.status, "recovery-required");
        assert.strictEqual((session.restoredOwnerView() as any)?.commissioningCheckpoint, true);
      });
      assert.deepStrictEqual(writes, []);
    });

    test("a recording the runtime no longer holds reports the observation incomplete, offers Restore previous settings, and keeps the checkpoint (A8.6)", async () => {
      const writes: unknown[] = [], retained: unknown[] = [], posts: any[] = [];
      const follower: any = {
        follow: async () => { throw new RuntimeControlError("Recording identity is unavailable.", "recording_unavailable"); },
        stop: async () => undefined,
        release: async () => undefined,
      };
      await withRecovery(follower, async () => {
        const session = sessionWith(journalled("recording", true), writes, retained);
        assert.strictEqual(await session.recoverApplyObservation(async payload => { posts.push(payload); }), true);
        assert.strictEqual((session as any).state.busy, undefined, "Restore stays available once the report is made");
        assert.strictEqual((session as any).state.interruptedCheckpoint.stage, "recording", "the journal keeps the observation");
      });
      const last = posts[posts.length - 1];
      assert.strictEqual(last.status, "failed");
      assert.strictEqual(last.failure.nextAction.kind, "restore");
      assert.match(last.failure.message, /no longer holds the recording/);
      assert.strictEqual(last.applicationAvailability, "restore");
      assert.deepStrictEqual(writes, [], "nothing is written before Restore");
      assert.deepStrictEqual(retained, []);
    });

    test("a failed requirement at reopening stores the response as completed, and offers Restore previous settings (B6.3)", async () => {
      const calls: string[] = [], writes: any[] = [], retained: unknown[] = [], posts: any[] = [];
      const stopped = { rawSamples: [], samples: [], evidence: { completeness: "incomplete", reason: "fake recording" } };
      const follower = followerOf([page("stopped", 4000, [])], stopped, calls);
      await withRecovery(follower, async () => {
        const session = sessionWith(journalled("recording", true), writes, retained);
        assert.strictEqual(await session.recoverApplyObservation(async payload => { posts.push(payload); }), true);
        assert.strictEqual((session as any).state.commissioned.status, "failed");
      });
      assert.deepStrictEqual(calls, ["follow", "stop", "release"]);
      assert.strictEqual(retained.length, 1, "the response is stored");
      assert.strictEqual(writes.length, 1);
      assert.strictEqual(writes[0].stage, "completed");
      assert.ok(writes[0].recordingResult, "the journal keeps the response with the completed checkpoint");
      const last = posts[posts.length - 1];
      assert.strictEqual(last.status, "failed");
      assert.strictEqual(last.failure.nextAction.kind, "restore");
      assert.strictEqual(last.applicationAvailability, "restore");
    });

    test("closing the panel during a reattached observation detaches it: nothing is stopped, stored, released or restored, and no outcome is reported (A10.4)", async () => {
      const calls: string[] = [], writes: unknown[] = [], retained: unknown[] = [], posts: any[] = [];
      let follows = 0;
      const follower: any = {
        follow: async () => { calls.push("follow"); follows++; return page("recording", follows * 100, []); },
        stop: async () => { calls.push("stop"); return undefined; },
        release: async () => { calls.push("release"); },
      };
      await withRecovery(follower, async () => {
        const session = sessionWith(journalled("recording", true), writes, retained);
        const reattaching = session.recoverApplyObservation(async payload => { posts.push(payload); });
        await until(() => follows > 1);
        session.dispose();
        assert.strictEqual(await reattaching, true);
      });
      assert.deepStrictEqual(calls, ["follow", "follow"], "closing neither stops, stores, releases nor restores");
      assert.deepStrictEqual(writes, [], "the journal keeps the observation for the next reopening");
      assert.ok(posts.every(post => post.status === "running"), "no outcome reaches the closed panel");
    });
  });
});
