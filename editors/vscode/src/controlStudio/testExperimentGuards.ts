import type { StudyRuntimeTarget } from "./panelWorkflowSupport";
import type { RecordingOwnerPersistence, RecordingRecoveryOwner } from "./runtimeRecording";

/**
 * Guards for the Test experiments that the Test page dispatches (Specification 35, A2.7, A8.5 and A8.6). An identity is
 * journalled before anything is sent, and the recording is journalled through the Test journal. A start whose outcome is
 * unknown is reconciled by status and is never sent again. Live requests go only to the target pinned when the loop
 * opened. Reopening completes an interrupted identification without resuming its experiment writes.
 */

export const UNPINNED_RUNTIME_MESSAGE = "Control Studio is not connected to this loop's runtime. Start the runtime and reopen Control Studio to connect it.";

/** Writes the journal entry, then dispatches. A failed journal write never dispatches. */
export async function journalBeforeDispatch<T>(journal: () => Promise<void>, dispatch: () => Promise<T>): Promise<T> {
  await journal();
  return dispatch();
}

/**
 * The recording journal of a Test experiment. The owner is written with the experiment's record, which is what reopening
 * restores into the Test recorder. Clearing the owner keeps the identity until the experiment is terminal (A8.5).
 */
export function experimentRecordingJournal(ports: {
  persist(owner: RecordingRecoveryOwner): Promise<void>;
  clearOwner(): Promise<void>;
}): RecordingOwnerPersistence {
  return { persist: owner => ports.persist(owner), clear: () => ports.clearOwner() };
}

/** The arguments of a Test start: the pinned runtime target and the journal. Either one missing refuses the start before any request. */
export interface TestStartPorts {
  readonly target: StudyRuntimeTarget;
  readonly journal: RecordingOwnerPersistence;
}

export function testStartPorts(target: StudyRuntimeTarget | undefined, journal: RecordingOwnerPersistence | undefined): TestStartPorts {
  if (!target) throw new Error(UNPINNED_RUNTIME_MESSAGE);
  if (!journal) throw new Error("The Test recording needs its journal before Start is sent. No runtime request was made.");
  return { target, journal };
}

/** Experiment identities whose start may have reached the runtime. Such an identity is not sent again (A8.5). */
export class UnconfirmedTestStarts {
  private readonly identities = new Set<string>();

  markUnconfirmed(experimentId: string): void {
    this.identities.add(experimentId);
  }

  /** Status has confirmed the identity, either as running or as never having been accepted. */
  confirm(experimentId: string): void {
    this.identities.delete(experimentId);
  }

  isUnconfirmed(experimentId: string): boolean {
    return this.identities.has(experimentId);
  }

  assertSendable(experimentId: string): void {
    if (this.identities.has(experimentId)) {
      throw new Error("This Test start was not confirmed. Its identity is reconciled by status and is not sent again; review the experiment to create a new identity.");
    }
  }
}
