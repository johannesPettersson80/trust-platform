import { RuntimeVerificationExecutionError } from "./verificationClient";
import { displayEngineeringUnit, formatEngineeringNumber } from "./chartContract";
import type { RunRequirementResult } from "./runValidation";
import { PhysicalApprovalRequired } from "./runtimeTarget";
import { AcquisitionLimitsRequired } from "./acquisitionSettings";
import { RuntimeControlError, RuntimeTransportError } from "../runtimeControlClient";

/**
 * A failure is classified by the type of the error that carries it, never by its message text (A11.2). The next
 * action is one typed kind; the engineer sees the kind's label on one button, and the message says what differs.
 */
export type MainWorkflowFailureCause =
  | "model-or-data"
  | "settings"
  | "controller-capability"
  | "task-infeasible"
  | "implementation"
  | "runtime"
  | "approval"
  | "unclassified";

/** The typed next actions of A11.2. `wait` and `none` offer no button. */
export type MainWorkflowNextActionKind =
  | "connect-runtime"
  | "open-loop-setup"
  | "open-limits"
  | "open-requirements"
  | "open-channels"
  | "choose-method"
  | "identify"
  | "tune"
  | "validate"
  | "approve"
  | "restore"
  | "wait"
  | "none";

/** Values that other modules still throw. `nextActionKind` maps each one to its kind, so those modules keep working. */
export type LegacyMainWorkflowNextAction =
  | "configure-acquisition"
  | "configure-scenarios"
  | "find-model"
  | "retune"
  | "choose-controller"
  | "revise-requirements"
  | "review-implementation"
  | "choose-application-mode"
  | "reapply";

export type MainWorkflowNextAction = MainWorkflowNextActionKind | LegacyMainWorkflowNextAction;

/** A failure as the engineer's session holds it: its kind is always a typed kind, never a legacy value. */
export interface MainWorkflowFailure {
  cause: MainWorkflowFailureCause;
  nextAction: MainWorkflowNextActionKind;
}

export interface MainWorkflowFailureInput {
  cause: MainWorkflowFailureCause;
  nextAction: MainWorkflowNextAction;
}

/** A classified failure. `summary` replaces the error's first sentence when the error's own text is not plain words. */
export interface MainWorkflowClassification extends MainWorkflowFailure {
  summary?: string;
}

/** The failure as the page receives it in `engineering.result` (A11.1, A11.2). */
export interface MainWorkflowFailurePresentation {
  message: string;
  detail?: string;
  nextAction: { kind: MainWorkflowNextActionKind; label: string };
}

export class MainWorkflowError extends Error {
  readonly failure: MainWorkflowFailure;
  constructor(message: string, failure: MainWorkflowFailureInput) {
    super(message);
    this.name = "MainWorkflowError";
    this.failure = { cause: failure.cause, nextAction: nextActionKind(failure.nextAction) };
  }
}

// The runtime's own codes (control-studio-runtime-protocol E0.4). A code not listed here has no typed action.
// stale_scan: a reloaded program has not completed its first scan yet (A6.7); it clears by itself.
const WAIT_CODES: readonly string[] = ["experiment_active", "transaction_pending", "controller_write_pending", "experiment_state_unavailable", "stale_scan"];
const OPEN_LOOP_SETUP_CODES: readonly string[] = ["invalid_target", "invalid_address", "incompatible_binding", "invalid_experiment", "controller_unavailable"];
const OPEN_LIMITS_CODES: readonly string[] = ["outside_limits"];

/** Maps a value that a module still throws to the typed kind of A11.2. */
export function nextActionKind(value: MainWorkflowNextAction): MainWorkflowNextActionKind {
  switch (value) {
    case "configure-acquisition": return "open-limits";
    case "configure-scenarios": return "open-requirements";
    case "find-model": return "identify";
    case "retune": return "tune";
    case "choose-controller": return "choose-method";
    case "revise-requirements": return "open-requirements";
    case "review-implementation": return "open-loop-setup";
    case "choose-application-mode": return "none";
    case "reapply": return "validate";
    default: return value;
  }
}

export function nextActionLabel(kind: MainWorkflowNextActionKind): string {
  switch (kind) {
    case "connect-runtime": return "Connect runtime…";
    case "open-loop-setup": return "Loop setup…";
    case "open-limits": return "Limits…";
    case "open-requirements": return "Requirements…";
    case "open-channels": return "Channels…";
    case "choose-method": return "Choose another method";
    case "identify": return "Identify plant";
    case "tune": return "Tune controller";
    case "validate": return "Validate controller";
    case "approve": return "Approve…";
    case "restore": return "Restore previous settings";
    case "wait":
    case "none": return "";
  }
}

/** Classifies a failure by the type of its error. Errors this module cannot import without a cycle are classified by their caller. */
export function classifyMainWorkflowFailure(error: unknown): MainWorkflowClassification {
  if (error instanceof MainWorkflowError) return error.failure;
  if (error instanceof PhysicalApprovalRequired) return { cause: "approval", nextAction: "approve", summary: firstSentence(error.message) };
  if (error instanceof AcquisitionLimitsRequired) return { cause: "settings", nextAction: "open-limits", summary: firstSentence(error.message) };
  // B5.5: an isolated run that failed keeps its evidence; the reason states what happened and no action is offered.
  if (error instanceof RuntimeVerificationExecutionError) return { cause: "implementation", nextAction: "none", summary: firstSentence(error.message) };
  if (error instanceof RuntimeControlError) return runtimeControlFailure(error);
  // The runtime was not reached or stopped answering: the original text stays as the detail.
  if (error instanceof RuntimeTransportError) {
    return { cause: "runtime", nextAction: "connect-runtime", summary: "Control Studio cannot reach the runtime." };
  }
  return { cause: "unclassified", nextAction: "none" };
}

/** The runtime's own code decides the action; its message is kept whole as the detail. */
function runtimeControlFailure(error: RuntimeControlError): MainWorkflowClassification {
  const code = error.code ?? "";
  const runtimeText = code && error.message.startsWith(`${code}: `) ? error.message.slice(code.length + 2) : error.message;
  const summary = firstSentence(withoutNextSuffix(runtimeText));
  if (WAIT_CODES.includes(code)) return { cause: "runtime", nextAction: "wait", summary: "Wait for the running operation to finish." };
  if (code === "physical_approval_required") return { cause: "approval", nextAction: "approve", summary };
  if (OPEN_LOOP_SETUP_CODES.includes(code)) return { cause: "runtime", nextAction: "open-loop-setup", summary };
  if (OPEN_LIMITS_CODES.includes(code)) return { cause: "runtime", nextAction: "open-limits", summary };
  return { cause: "runtime", nextAction: "none", summary };
}

/**
 * The failure's presentation: `message` is the error's first sentence without any "Next: …" suffix; `detail` is the full
 * error text, present only when it says more than the message.
 */
export function presentMainWorkflowFailure(text: string, failure: MainWorkflowClassification): MainWorkflowFailurePresentation {
  const full = withoutNextSuffix(text) || "Control Studio could not complete this step.";
  const message = failure.summary ?? firstSentence(full);
  return {
    message,
    ...(full !== message ? { detail: full } : {}),
    nextAction: { kind: failure.nextAction, label: nextActionLabel(failure.nextAction) },
  };
}

/**
 * Binding the runtime to a loop fails when no matching runtime is connected (A11.2): the fix is to connect one. A
 * cancelled bind keeps its own error, since cancelling is the engineer's choice and not a missing connection.
 */
export async function bindingRuntime<T>(bind: () => Promise<T>, cancelled: () => boolean = () => false): Promise<T> {
  try { return await bind(); }
  catch (error) {
    if (error instanceof MainWorkflowError || cancelled()) throw error;
    throw new MainWorkflowError(error instanceof Error ? error.message : String(error), { cause: "runtime", nextAction: "connect-runtime" });
  }
}

/**
 * The model qualification gate (A11.2, B4.7 "No model → Identify"): a model that does not qualify for the loop is
 * replaced by identifying one that does. The gate's own messages say what differs; the action is typed here, not read
 * from those messages. An error that already carries its classification keeps it.
 */
export function qualifyModel(check: () => void): void {
  try { check(); }
  catch (error) {
    if (error instanceof MainWorkflowError) throw error;
    throw new MainWorkflowError(error instanceof Error ? error.message : String(error), { cause: "model-or-data", nextAction: "identify" });
  }
}

/** The sentence that names the next action in a `reason`; empty when the action has no button. */
export function nextActionSentence(kind: MainWorkflowNextActionKind): string {
  const label = nextActionLabel(kind);
  return label ? ` Next: ${label}.` : "";
}

function withoutNextSuffix(text: string): string {
  return text.replace(/\s*\bNext: [\s\S]*$/, "").trim();
}

/** The first sentence: text up to a full stop, question or exclamation mark that ends a word, such as "0.21 °C" stays whole. */
function firstSentence(text: string): string {
  const match = /^[\s\S]*?[.!?](?=\s|$)/.exec(text.trim());
  return (match ? match[0] : text).trim() || "Control Studio could not complete this step.";
}

/** Missing observations are different from an observed performance failure. */
export function verificationFailure(results: readonly { id: string; status: string }[]): MainWorkflowClassification {
  if (results.some(result => result.status !== "Pass" && result.id.includes("fixed-parameter:"))) return { cause: "implementation", nextAction: "open-loop-setup" };
  if (results.some(result => result.status === "Inconclusive" && result.id.startsWith("switching-"))) return { cause: "settings", nextAction: "tune" };
  if (!results.length || results.some(result => result.status === "Inconclusive")) return { cause: "model-or-data", nextAction: "none" };
  return { cause: "settings", nextAction: "tune" };
}

/** Explain the observed requirement failure without hiding the complete result. */
export function verificationFailureReason(results: readonly RunRequirementResult[]): string {
  const missing = results.find(result => result.status === "Inconclusive");
  if (missing) return `Verification is incomplete: ${missing.reason}`;
  const error = results.find(result => result.id === "target-error" && result.status === "Fail"
    && Number.isFinite(result.value) && Number.isFinite(result.limit));
  if (error) return `Recorded target error ${formatEngineeringNumber(error.value!)} ${displayEngineeringUnit(error.unit)} exceeds ${formatEngineeringNumber(error.limit!)} ${displayEngineeringUnit(error.unit)}.`;
  const deadline = results.find(result => result.kind === "settling" && result.status === "Fail" && Number.isFinite(result.limit));
  if (deadline) return `The recorded response did not settle within ${formatEngineeringNumber(deadline.limit!)} ${displayEngineeringUnit(deadline.unit)}.`;
  const failed = results.find(result => result.status === "Fail");
  return failed ? `Verification failed: ${failed.reason}` : "Verification has no complete observed requirement results.";
}
