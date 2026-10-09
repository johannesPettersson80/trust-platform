import { RuntimeControlError } from "../runtimeControlClient";
import { classifyControlExperimentStatus } from "./experiment";
import type { CampaignClock } from "./identificationCampaign";

/**
 * One experiment, read by its identifier alone (E5.6, E11.4, E11.5). Reading never writes: a start whose outcome is unknown is
 * reconciled here, and never sent again.
 */
export type ExperimentIdentity =
  /** Running, or ended with its restoration still pending: the runtime still owns it. */
  | { readonly kind: "running"; readonly state: unknown }
  /** Ended, with its restoration confirmed or failed: nothing is left running. */
  | { readonly kind: "terminal"; readonly state: unknown }
  /** The runtime holds no experiment with this identifier, so it never started. */
  | { readonly kind: "absent"; readonly reason: string }
  /** No terminal status can be read, so the operation stays unresolved (E11.5). */
  | { readonly kind: "unresolved"; readonly reason: string };

/** Reads one experiment by its identifier. An unknown identifier is absent; a transport failure or an invalid reply is unresolved. */
export async function readExperimentIdentity(status: (experimentId: string) => Promise<unknown>,
  experimentId: string): Promise<ExperimentIdentity> {
  let value: unknown;
  try {
    value = await status(experimentId);
  } catch (error) {
    if (error instanceof RuntimeControlError && error.code === "experiment_not_found") {
      return { kind: "absent", reason: explanation(error) };
    }
    return { kind: "unresolved", reason: `The status of experiment ${experimentId} could not be read: ${explanation(error)}.` };
  }
  const phase = classifyControlExperimentStatus(value);
  if (phase === "invalid") return { kind: "unresolved", reason: "The runtime returned an invalid experiment status." };
  return phase === "terminal" ? { kind: "terminal", state: value } : { kind: "running", state: value };
}

/**
 * Polls one experiment until it has ended or is absent. Only status reads are sent: nothing is aborted, finished or restored
 * here (E11.5). Resolves undefined once the caller has released its follow.
 */
export async function followUntilEnded(read: () => Promise<ExperimentIdentity>, clock: CampaignClock,
  released: () => boolean, intervalMs = 500): Promise<ExperimentIdentity | undefined> {
  for (;;) {
    await clock.wait(intervalMs);
    if (released()) return undefined;
    const identity = await read();
    if (identity.kind === "terminal" || identity.kind === "absent") return identity;
  }
}

/** Frees a stopped recording's runtime buffer. Call it only after the recording's samples are kept (E4.7, E11.6). */
export async function releaseRecording(recording: object): Promise<void> {
  const release = (recording as { release?: unknown }).release;
  if (typeof release === "function") await (release as () => Promise<void>).call(recording);
}

function explanation(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
