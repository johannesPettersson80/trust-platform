import { identifiedModelPresentation, type IdentificationWorkflow } from "./identification";

/** Live and reopened Identify views project the same session state. */
export function identificationPresentationState(workflow: IdentificationWorkflow, inputUnit: string, outputUnit: string): Record<string, unknown> | undefined {
  const { phase, qualification, review, failure } = workflow.currentState;
  if (phase === "editing") {
    const saved = workflow.snapshot();
    return { status: "complete", phase, form: saved?.form,
      hasRecording: Boolean(saved?.current?.recording), nextFitRole: workflow.nextFitRole };
  }
  if (phase === "selected") return { status: "complete", phase: "recording-selected", nextFitRole: workflow.nextFitRole };
  if (!qualification) return undefined;
  if (["qualified", "validation-qualified", "rejected"].includes(phase)) {
    return { status: qualification.suitable ? "complete" : "failed", phase: "qualified", qualification,
      nextFitRole: workflow.nextFitRole, ...(qualification.suitable ? {} : { reason: qualification.reason }) };
  }
  if (!review) return { status: "failed", reason: failure ?? "Review the retained recording before fitting." };
  return { status: phase === "failed" ? "failed" : "complete", phase: phase === "accepted" ? "accepted" : review.stage,
    ...(phase === "failed" ? { reason: failure ?? "Independent validation failed." } : {}),
    source: review.stage === "validation_required" ? review.preview.source : review.result.source,
    model: review.selected, fit: review.fit,
    presentation: identifiedModelPresentation(review.selected, review.fit, inputUnit, outputUnit),
    estimationQualification: review.estimationQualification,
    ...(review.stage === "acceptance_required" ? { validationQualification: review.validationQualification } : {}) };
}
