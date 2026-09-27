import { RuntimeVerificationExecutionError } from "./verificationClient";
import { formatEngineeringNumber } from "./chartContract";
import type { RunRequirementResult } from "./runValidation";

/**
 * The Main workflow has four deliberate recovery edges. Keep this small, typed
 * contract at the host boundary so a lower-level error cannot strand an
 * engineer at a generic failure message.
 */
export type MainWorkflowFailureCause =
  | "model-or-data"
  | "settings"
  | "controller-capability"
  | "task-infeasible"
  | "implementation";

export type MainWorkflowNextAction =
  | "configure-acquisition"
  | "configure-scenarios"
  | "find-model"
  | "retune"
  | "choose-controller"
  | "revise-requirements"
  | "review-implementation"
  | "choose-application-mode";

export interface MainWorkflowFailure {
  cause: MainWorkflowFailureCause;
  nextAction: MainWorkflowNextAction;
}

export class MainWorkflowError extends Error {
  constructor(message: string, readonly failure: MainWorkflowFailure) { super(message); this.name = "MainWorkflowError"; }
}
export function classifyMainWorkflowFailure(action: string, error: unknown): MainWorkflowFailure {
  if (error instanceof MainWorkflowError) return error.failure;
  if (action === "verify" && error instanceof RuntimeVerificationExecutionError) return { cause: "implementation", nextAction: "review-implementation" };
  const reason = explanation(error).toLowerCase();
  if (has(reason, ["review acquisition", "excitation amplitude", "excitation method", "headroom", "relay direction", "selected acquisition mode", "acquisition duration", "feedback relay", "frequency identification"])) return { cause: "settings", nextAction: "configure-acquisition" };
  if (has(reason, ["mandatory scenarios", "scenario effect", "scenario requirements", "scenario policy", "invalid mandatory verification", "measurement filter", "reference shaping", "reviewed input disturbance", "reviewed measurement-noise", "constrained recovery"])) return { cause: "settings", nextAction: "configure-scenarios" };
  if (has(reason, ["physical equilibrium", "equipment range", "no bounded setpoint excitation"])) return { cause: "task-infeasible", nextAction: "revise-requirements" };
  if (has(reason, ["identifiable", "independent", "physical balance", "dry-level", "model structure", "held-out", "coherence", "excitation headroom", "recorded model", "dynamic state model", "free-running prediction"])) return { cause: "model-or-data", nextAction: "find-model" };
  if (has(reason, ["st changed controller parameter", "candidate not adopted", "iec type does not match", "state initialization", "state-feedback recovery", "estimator perturbation", "compiled binding", "controller execution fault"])) {
    return { cause: "implementation", nextAction: "review-implementation" };
  }
  if (has(reason, [
    "multiple input or output channels", "multivariable controller", "no compatible automatic controller candidate", "frequency identification requires", "relay identification requires", "initialization adapter",
    "controller adapter", "actual st law", "declared proportional law",
    "parameter and reference bindings", "controller/model declaration", "requires compatible st",
  ])) return { cause: "controller-capability", nextAction: "choose-controller" };
  if (has(reason, [
    "find or supply a model", "selected model", "model channels", "model validation",
    "identification", "model evidence", "model json", "input and output",
  ])) return { cause: "model-or-data", nextAction: "find-model" };
  if (has(reason, [
    "cannot satisfy the declared", "cannot satisfy the task", "finite proportional gain bounds",
    "task is infeasible", "outside the actuator", "output limit", "capacity",
  ])) return { cause: "task-infeasible", nextAction: "revise-requirements" };
  if (action === "find") return { cause: "model-or-data", nextAction: "find-model" };
  if (action === "verify" || action === "apply" || has(reason, [
    "task requirements", "target change", "settling allowance", "run of at most",
    "duration", "manual proportional gain", "gain selection", "gain must be finite",
  ])) return { cause: "settings", nextAction: "retune" };
  return { cause: "settings", nextAction: "retune" };
}

export function mainWorkflowNextActionLabel(nextAction: MainWorkflowNextAction): string {
  switch (nextAction) {
    case "configure-acquisition": return "Review identification controls";
    case "configure-scenarios": return "Review scenario requirements";
    case "find-model": return "Identify model";
    case "retune": return "Tune again";
    case "choose-controller": return "Choose controller";
    case "revise-requirements": return "Revise requirements or equipment";
    case "choose-application-mode": return "Choose a supported application mode";
    case "review-implementation": return "Review ST implementation and bindings";
  }
}

function has(reason: string, fragments: readonly string[]): boolean {
  return fragments.some(fragment => reason.includes(fragment));
}

function explanation(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Missing observations are different from an observed performance failure. */
export function verificationFailure(results: readonly { id: string; status: string }[]): MainWorkflowFailure {
  if (results.some(result => result.status !== "Pass" && result.id.includes("fixed-parameter:"))) return { cause: "implementation", nextAction: "review-implementation" };
  if (results.some(result => result.status === "Inconclusive" && result.id.startsWith("switching-"))) return { cause: "settings", nextAction: "retune" };
  if (!results.length || results.some(result => result.status === "Inconclusive")) return { cause: "model-or-data", nextAction: "find-model" };
  return { cause: "settings", nextAction: "retune" };
}

/** Explain the observed requirement failure without hiding the complete result. */
export function verificationFailureReason(results: readonly RunRequirementResult[]): string {
  const missing = results.find(result => result.status === "Inconclusive");
  if (missing) return `Verification is incomplete: ${missing.reason}`;
  const error = results.find(result => result.id === "target-error" && result.status === "Fail"
    && Number.isFinite(result.value) && Number.isFinite(result.limit));
  if (error) return `Recorded target error ${formatEngineeringNumber(error.value!)} ${error.unit} exceeds ${formatEngineeringNumber(error.limit!)} ${error.unit}.`;
  const deadline = results.find(result => result.kind === "settling" && result.status === "Fail" && Number.isFinite(result.limit));
  if (deadline) return `The recorded response did not settle within ${formatEngineeringNumber(deadline.limit!)} ${deadline.unit}.`;
  const failed = results.find(result => result.status === "Fail");
  return failed ? `Verification failed: ${failed.reason}` : "Verification has no complete observed requirement results.";
}
