import type { ControlLoopDefinition } from "./loopDefinition";
import type { ControlExperimentPlan } from "./experiment";
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
}

/** Keeps a bounded Test alive through runtime reconnect and terminal restoration. */
export async function refreshTestExperiment(lifecycle: TestExperimentLifecycle): Promise<void> {
  const experimentId = lifecycle.experimentId();
  const loop = lifecycle.loop();
  if (!experimentId || !loop) return;
  if (!lifecycle.recordingActive()) {
    const recovery = lifecycle.recovery(loop);
    if (!recovery?.recordingOwner || !recovery.plan) {
      await lifecycle.post("failed", { reason: "Test recording recovery is unavailable because the reviewed owner or plan is missing." });
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
