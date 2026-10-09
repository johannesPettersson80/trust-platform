import type { AutomaticIdentificationResult } from "./automaticIdentification";
import { ConnectedIdentification, type ConnectedIdentificationOptions, type ConnectedIdentificationPorts } from "./connectedIdentification";
import { ConnectedIdentificationMode } from "./connectedIdentificationMode";
import { ControlExperimentClient } from "./experiment";
import { systemClock, type CampaignClock, type RecordingFollower } from "./identificationCampaign";
import { followUntilEnded, readExperimentIdentity, releaseRecording, type ExperimentIdentity } from "./identificationReconcile";
import type { ControlLoopDefinition } from "./loopDefinition";
import { bindRuntimeConnection } from "./runtimeConnection";
import { RuntimeRecordingSession, type RecordingRecoveryOwner } from "./runtimeRecording";
import type { RuntimeRequest, RuntimeVerificationAttempt } from "./verificationClient";

export interface IdentificationRecovery {
  /** Stops the original recording and restores the original mode and commands, once no experiment is left running (E11.5). */
  cancel(): Promise<void>;
  /** Present when the journal holds a campaign whose experiment may still be running: reattaches, follows it to its settled end,
   * finishes it, returns the mode and fits (A8.5, A8.6). Its result continues the study as if the campaign had never been interrupted. */
  resume?(progress: (text: string) => Promise<void>): Promise<AutomaticIdentificationResult>;
  snapshot(): RuntimeVerificationAttempt | undefined;
  /** Lets go of this client's timers and followers. The runtime keeps its experiment, recording and journal; nothing is aborted (A8.5, A10.4). */
  release?(): void;
}

export interface IdentificationRecoveryOptions extends Pick<ConnectedIdentificationOptions, "clock" | "createMode" | "fit"> {
  /** Binds the runtime the journal names; the connected runtime by default. */
  bind?: typeof bindRuntimeConnection;
  /** Reattaches the journalled recording; the runtime's recording session by default. */
  recover?(loop: ControlLoopDefinition, owner: RecordingRecoveryOwner): Promise<RecordingFollower>;
}

const RELEASED_REASON = "The identification was released; the runtime keeps its experiment and its journal.";
const RUNNING_REASON = "The interrupted experiment is still running in the runtime. The original mode returns when it ends; nothing is started or aborted meanwhile.";
const RESTORE_REASON = "Restore the original identification mode and commands before starting another operation.";

/** Reconciles the original recording and mode owners. Recovery starts no experiment and aborts none (A8.6, E11.5). */
export function identificationRecovery(loop: ControlLoopDefinition, ports: ConnectedIdentificationPorts | undefined,
  completed: () => void, options: IdentificationRecoveryOptions = {}): IdentificationRecovery | undefined {
  const checkpoint = ports?.mode.read(), recording = ports?.recordingOwner(), campaign = ports?.campaign.read();
  if (!ports || !checkpoint && !recording && !campaign) return undefined;
  const owner = recording ?? campaign?.recording;
  const journalledPlan = campaign?.plan;
  const bind = options.bind ?? bindRuntimeConnection;
  const recover = options.recover ?? ((target: ControlLoopDefinition, recovering: RecordingRecoveryOwner) =>
    RuntimeRecordingSession.recover(target, recovering));
  const clock: CampaignClock = options.clock ?? systemClock;
  const resumable = campaign?.phase === "step" && campaign.loopRevision === loop.revision && campaign.plan !== undefined;
  let restored = false;
  let released = false;
  let following = false;
  let resuming = false;
  let failure: string | undefined;
  let resumed: ConnectedIdentification | undefined;

  const connect = async () => {
    const connection = await bind(loop.runtimeId);
    if (connection.source !== "simulation") throw new Error("Reconnect the original simulator for identification recovery.");
    return connection;
  };

  const readJournalled = (request: RuntimeRequest, experimentId: string): Promise<ExperimentIdentity> => {
    const client = new ControlExperimentClient((operation, params) => request(operation, params as Record<string, unknown>));
    return readExperimentIdentity(id => client.status(id), experimentId);
  };

  /** Keeps the original recording's samples, then releases its buffer, and returns the original mode (A6.2, E4.7). */
  const restoreOriginal = async (request: RuntimeRequest): Promise<void> => {
    if (owner) {
      const session = await recover(loop, owner);
      await ports.retain(await session.stop());
      await releaseRecording(session);
      await ports.clear();
    }
    if (checkpoint) {
      const mode = options.createMode?.(loop, request, ports.mode) ?? new ConnectedIdentificationMode(loop, request, ports.mode);
      await mode.restore();
    }
    restored = true;
    completed();
  };

  /** Follows a running experiment with status reads only, and restores the original once it has ended (E11.5, A6.2). */
  const followInBackground = (request: RuntimeRequest, experimentId: string): void => {
    if (following) return;
    following = true;
    void followUntilEnded(() => readJournalled(request, experimentId), clock, () => released)
      .then(async identity => { if (identity && !released) await restoreOriginal(request); })
      .catch(error => { failure = explanation(error); })
      .finally(() => { following = false; });
  };

  const cancel = async (): Promise<void> => {
    if (restored) return;
    // A reattached campaign is cancelled by its own client: the experiment the engineer stops is aborted, then the original returns (E11.4).
    if (resumed) {
      await resumed.cancellationOwner().cancel();
      restored = true;
      completed();
      return;
    }
    if (resuming) throw new Error("The interrupted identification is reattaching. Cancel again once it has reattached.");
    const connection = await connect();
    if (journalledPlan) {
      const identity = await readJournalled(connection.request, journalledPlan.id);
      if (identity.kind === "unresolved") throw new Error(`${identity.reason} Nothing was restored: reconnect the original simulator and try again.`);
      if (identity.kind === "running") {
        followInBackground(connection.request, journalledPlan.id);
        throw new Error(RUNNING_REASON);
      }
    }
    await restoreOriginal(connection.request);
  };

  /** Runs the reattached campaign to its end. A client that leaves nothing owned completes the recovery; anything else stays unresolved. */
  const attach = async (client: ConnectedIdentification, recorder: RecordingFollower,
    progress: (text: string) => Promise<void>): Promise<AutomaticIdentificationResult> => {
    try {
      const result = await client.resume(loop, recorder, progress);
      restored = true;
      completed();
      return result;
    } catch (error) {
      if (client.isSettled()) {
        restored = true;
        completed();
      } else {
        failure = explanation(error);
      }
      throw error;
    }
  };

  const resume = resumable && campaign ? async (progress: (text: string) => Promise<void>): Promise<AutomaticIdentificationResult> => {
    if (released) throw new Error(RELEASED_REASON);
    resuming = true;
    try {
      const connection = await connect();
      const recorder = await recover(loop, campaign.recording);
      if (released) throw new Error(RELEASED_REASON);
      resumed = new ConnectedIdentification(connection.request, ports, { clock, createMode: options.createMode, fit: options.fit });
      return await attach(resumed, recorder, progress);
    } catch (error) {
      if (!restored) failure = explanation(error);
      throw error;
    } finally {
      resuming = false;
    }
  } : undefined;

  /** Closing releases the reattached client's timers and followers without aborting, restoring or clearing anything (A8.5, A10.4). */
  const release = (): void => {
    released = true;
    resumed?.dispose();
  };

  const snapshot = (): RuntimeVerificationAttempt | undefined => restored ? undefined : {
    status: "cancellation-unconfirmed",
    jobId: checkpoint?.transfer?.owner.id ?? owner?.recordingId ?? `mode:${checkpoint!.original.program_revision}`,
    projectRevision: checkpoint?.original.program_revision ?? owner?.programRevision ?? loop.revision,
    request: { execution: "connected-runtime" },
    reason: failure ?? (following || resumed ? RUNNING_REASON : RESTORE_REASON),
  };

  return { cancel, ...(resume ? { resume } : {}), release, snapshot };
}

function explanation(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
