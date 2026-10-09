import type { ControlLoopDefinition } from "./loopDefinition";
import type { ControlExperimentPlan } from "./experiment";
import { readExperimentIdentity } from "./identificationReconcile";
import type { RecordingRecoveryOwner } from "./runtimeRecording";

export interface TestExperimentRecovery {
  readonly plan?: ControlExperimentPlan;
  readonly recordingOwner?: RecordingRecoveryOwner;
}

export interface TestExperimentLifecycle {
  experimentId(): string | undefined;
  loop(): ControlLoopDefinition | undefined;
  recovery(loop: ControlLoopDefinition): TestExperimentRecovery | undefined;
  recordingActive(): boolean;
  recoverOwner(loop: ControlLoopDefinition, owner: RecordingRecoveryOwner): Promise<void>;
  recovered(plan: ControlExperimentPlan): void;
  status(experimentId: string): Promise<unknown>;
  classify(value: unknown): "running" | "awaiting_restoration" | "terminal" | "invalid";
  terminal(value: unknown): Promise<unknown>;
  terminalObserved(value: unknown): void;
  clear(): Promise<void>;
  post(phase: string, payload: unknown): Promise<void>;
  schedule(): void;
  /** Whether this identity's start is still landing: journalled, and not yet answered or failed (A8.5). */
  startLanding?(experimentId: string): boolean;
}

/** Keeps a bounded Test alive through runtime reconnect and terminal restoration. */
export async function refreshTestExperiment(lifecycle: TestExperimentLifecycle): Promise<void> {
  const experimentId = lifecycle.experimentId();
  const loop = lifecycle.loop();
  if (!experimentId || !loop) return;
  if (!lifecycle.recordingActive()) {
    const recovery = lifecycle.recovery(loop);
    // Journalled with its identity only, before any recording owner exists: the runtime's status decides (A8.5, E11.4).
    if (!recovery?.recordingOwner) {
      await reconcileByIdentity(lifecycle, experimentId);
      return;
    }
    if (!recovery.plan) {
      await lifecycle.post("failed", { reason: "Test recording recovery is unavailable because the reviewed plan is missing." });
      return;
    }
    try {
      await lifecycle.recoverOwner(loop, recovery.recordingOwner);
      lifecycle.recovered(recovery.plan);
    } catch (error) {
      await lifecycle.post("failed", {
        reason: `Test recording recovery remains unconfirmed: ${error instanceof Error ? error.message : String(error)}`,
      });
      return;
    }
  }
  try {
    const value = await lifecycle.status(experimentId);
    const phase = lifecycle.classify(value);
    if (phase === "invalid") throw new Error("The runtime returned an invalid experiment state.");
    if (phase === "terminal") lifecycle.terminalObserved(value);
    const payload = phase === "terminal" ? await lifecycle.terminal(value) : value;
    if (phase === "terminal" && !lifecycle.recordingActive()) await lifecycle.clear();
    else lifecycle.schedule();
    await lifecycle.post(phase === "awaiting_restoration" ? "restoring" : phase, payload);
  } catch (error) {
    await lifecycle.post("failed", { reason: error instanceof Error ? error.message : String(error) });
  }
}

/**
 * A Test journalled with its identity only is reconciled by its status, and nothing is started or sent again: a running experiment
 * is attached and followed, an ended one is concluded and cleared, and an identity the runtime does not hold is cleared, since
 * nothing was applied, unless its start is still landing: then the journal is kept until the start is answered or has failed.
 * An unreadable status keeps the journal for the next reconciliation.
 */
async function reconcileByIdentity(lifecycle: TestExperimentLifecycle, experimentId: string): Promise<void> {
  const identity = await readExperimentIdentity(id => lifecycle.status(id), experimentId);
  if (identity.kind === "running") {
    lifecycle.schedule();
    await lifecycle.post(lifecycle.classify(identity.state) === "awaiting_restoration" ? "restoring" : "running", identity.state);
    return;
  }
  if (identity.kind === "terminal") {
    lifecycle.terminalObserved(identity.state);
    const payload = await lifecycle.terminal(identity.state);
    await lifecycle.clear();
    await lifecycle.post("terminal", payload);
    return;
  }
  if (identity.kind === "absent") {
    // A start still landing has not been answered, so "absent" does not yet mean it never started: its journal stays.
    if (lifecycle.startLanding?.(experimentId)) return;
    await lifecycle.clear();
    await lifecycle.post("failed", { reason: `The runtime holds no experiment with this identity, so the Test never started and nothing was applied. ${identity.reason}` });
    return;
  }
  await lifecycle.post("failed", { reason: `${identity.reason} The start is not sent again; reopening reconciles it.` });
}
