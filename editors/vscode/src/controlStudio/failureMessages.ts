// Maps every runtime and identification enum shown to the learner into a plain-language sentence with a recovery action, so no raw wire identifier reaches the screen unmapped.

import type { ModelKind } from "./identification";

function assertNever(value: never): never {
  throw new Error(`Unhandled Control Studio enum value: ${String(value)}`);
}

const MODEL_KINDS: readonly ModelKind[] = [
  "static_gain", "integrating", "first_order", "first_order_dead_time",
  "second_order", "second_order_dead_time",
];

function isModelKind(value: string): value is ModelKind {
  return (MODEL_KINDS as readonly string[]).includes(value);
}

/** Human label for an identified model structure; an unrecognized wire value still reads as a sentence. */
export function modelKindLabel(kind: string): string {
  if (!isModelKind(kind)) return `Unrecognized model structure (${kind})`;
  switch (kind) {
    case "static_gain": return "Static gain";
    case "integrating": return "Integrating";
    case "first_order": return "First order";
    case "first_order_dead_time": return "First order with dead time";
    case "second_order": return "Second order";
    case "second_order_dead_time": return "Second order with dead time";
    default: return assertNever(kind);
  }
}

type ExperimentTerminalStatus = "completed" | "aborted" | "failed" | "failed_to_restore";
const EXPERIMENT_TERMINAL_STATUSES: readonly ExperimentTerminalStatus[] =
  ["completed", "aborted", "failed", "failed_to_restore"];

function isExperimentTerminalStatus(value: string): value is ExperimentTerminalStatus {
  return (EXPERIMENT_TERMINAL_STATUSES as readonly string[]).includes(value);
}

/** Human label for a terminal experiment status; an unrecognized runtime status still reads as a sentence. */
export function experimentStatusLabel(status: string): string {
  if (!isExperimentTerminalStatus(status)) return `Stopped (${status})`;
  switch (status) {
    case "completed": return "Completed";
    case "aborted": return "Aborted";
    case "failed": return "Failed";
    case "failed_to_restore": return "Failed to restore";
    default: return assertNever(status);
  }
}

type ExperimentRestorationStatus = "pending" | "restored" | "not_required" | "failed";
const EXPERIMENT_RESTORATION_STATUSES: readonly ExperimentRestorationStatus[] =
  ["pending", "restored", "not_required", "failed"];

function isExperimentRestorationStatus(value: string): value is ExperimentRestorationStatus {
  return (EXPERIMENT_RESTORATION_STATUSES as readonly string[]).includes(value);
}

// The exhaustive set of stop and restoration reason codes the runtime sends today
// (crates/trust-runtime/src/control/experiment_handlers.rs). An unrecognized code
// still produces a readable sentence instead of the raw snake_case text.
type ExperimentReasonCode =
  | "user_abort" | "duration_complete" | "signal_forced" | "stale_runtime_data"
  | "bad_runtime_data" | "signal_outside_limits" | "relay_feedback_forced"
  | "bad_relay_feedback" | "confirmation_timeout";
const EXPERIMENT_REASON_CODES: readonly ExperimentReasonCode[] = [
  "user_abort", "duration_complete", "signal_forced", "stale_runtime_data",
  "bad_runtime_data", "signal_outside_limits", "relay_feedback_forced",
  "bad_relay_feedback", "confirmation_timeout",
];

function isExperimentReasonCode(value: string): value is ExperimentReasonCode {
  return (EXPERIMENT_REASON_CODES as readonly string[]).includes(value);
}

/** Plain-language clause for a raw runtime stop or restoration reason code. */
export function experimentReasonSentence(rawCode: string): string {
  if (!isExperimentReasonCode(rawCode)) return `the runtime reported "${rawCode}"`;
  switch (rawCode) {
    case "user_abort": return "you selected Abort test";
    case "duration_complete": return "the reviewed maximum duration was reached";
    case "signal_forced": return "the affected signal became forced on the runtime";
    case "stale_runtime_data": return "runtime data stopped updating";
    case "bad_runtime_data": return "the runtime reported a non-numeric or invalid value";
    case "signal_outside_limits": return "the signal moved outside its reviewed limits";
    case "relay_feedback_forced": return "the relay feedback signal became forced";
    case "bad_relay_feedback": return "the relay feedback signal became invalid";
    case "confirmation_timeout": return "the runtime did not confirm the restored value in time";
    default: return assertNever(rawCode);
  }
}

/** Sentence with the required next action for a restoration outcome. */
export function experimentRestorationMessage(restoration: string, restorationReason?: string): string {
  const known: ExperimentRestorationStatus = isExperimentRestorationStatus(restoration) ? restoration : "failed";
  switch (known) {
    case "restored": return "The affected signal was confirmed restored to its original value.";
    case "not_required": return "No write was made, so no restoration was required.";
    case "pending": return "Restoration is still being confirmed against the runtime.";
    case "failed": {
      const because = restorationReason ? ` — ${experimentReasonSentence(restorationReason)}` : "";
      return `Restoration failed${because}. Check the affected output on the runtime and restore it manually before starting another test.`;
    }
    default: return assertNever(known);
  }
}

/** One sentence covering what happened and the recovery action for a finished experiment. */
export function experimentTerminalMessage(input: {
  status: string; stopReason: string; restoration: string; restorationReason?: string;
}): string {
  const statusLabel = experimentStatusLabel(input.status);
  const because = experimentReasonSentence(input.stopReason);
  const restorationClause = experimentRestorationMessage(input.restoration, input.restorationReason);
  return `${statusLabel} — ${because}. ${restorationClause}`;
}

/** Status line shown while the runtime is still confirming restoration after a stop. */
export function experimentRestoringMessage(status: string, stopReason: string): string {
  return `${experimentStatusLabel(status)} — ${experimentReasonSentence(stopReason)}. Confirming restoration with the runtime…`;
}

/**
 * Clarifies that a still-visible candidate predates a later failed attempt, without
 * discarding it — spec §14 requires an error to preserve earlier valid results, so the
 * candidate is marked stale rather than cleared.
 */
export function staleCandidateNotice(stage: "Identify" | "Tune" | "Validate" | "Automatic Tune", reason: string): string {
  return `This candidate is from an earlier attempt. The latest ${stage} attempt failed: ${reason}`;
}
