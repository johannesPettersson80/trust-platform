import type { DeclaredControlAnalysis } from "./declaredAnalysis";
import type { GeneralModel } from "./generalModel";
import type { GeneralTransferModel, ModelChannel } from "./generalModel";
import { identificationOperatingPoint, type AcceptedIdentificationModel, type IdentifiedModel, type IdentificationSample } from "./identification";
import { createHash } from "crypto";
import { linearTransferModel } from "./linearTransferModel";

/** Retains independent record identities and exact transport delay when identification enters shared analysis. */
export function acceptedIdentificationModel(
  accepted: AcceptedIdentificationModel,
  channels: { inputs: readonly ModelChannel[]; outputs: readonly ModelChannel[] }
): GeneralTransferModel {
  const { model, review } = accepted;
  if (review.stage !== "acceptance_required" || review.result.partition !== "independent-runs"
      || !review.estimationQualification.suitable || !review.validationQualification.suitable) {
    throw new Error("Independent identification validation is required before selecting a fitted model.");
  }
  const evidence = [digest(review.estimation), digest(review.validation)];
  if (evidence[0] === evidence[1]) throw new Error("Estimation and validation records must be independent.");
  return identifiedTransferModel(model, review.estimationQualification.samples, channels, "validated", evidence);
}

/** A single-record model is available for tuning without inventing independent validation. */
export function provisionalIdentificationModel(model: IdentifiedModel, samples: readonly IdentificationSample[],
  channels: { inputs: readonly ModelChannel[]; outputs: readonly ModelChannel[] }, evidence: string): GeneralTransferModel {
  return identifiedTransferModel(model, samples, channels, "unvalidated", [evidence]);
}

function identifiedTransferModel(model: IdentifiedModel, samples: readonly IdentificationSample[],
  channels: { inputs: readonly ModelChannel[]; outputs: readonly ModelChannel[] }, validation: GeneralModel["validation"], evidence: string[]): GeneralTransferModel {
  if (channels.inputs.length !== 1 || channels.outputs.length !== 1) {
    throw new Error("Low-order identification requires one input and one output channel.");
  }
  let denominator = model.kind === "integrating" ? [1, 0] : [1];
  for (const time of [model.timeConstantSeconds, model.secondTimeConstantSeconds]) {
    if (time === undefined) continue;
    const next = Array<number>(denominator.length + 1).fill(0);
    denominator.forEach((coefficient, i) => { next[i] += coefficient * time; next[i + 1] += coefficient; });
    denominator = next;
  }
  const { input0, output0 } = identificationOperatingPoint(samples);
  return { kind: "transfer-function", numerator: [model.gain], denominator,
    delaySeconds: model.deadTimeSeconds ?? 0, source: "fitted", validation, evidence,
    operatingPoint: { inputs: [input0], outputs: [output0] },
    revision: digest({ model, evidence, channels }),
    inputs: structuredClone(channels.inputs), outputs: structuredClone(channels.outputs) };
}

function digest(value: unknown): string {
  return `sha256:${createHash("sha256").update(JSON.stringify(value)).digest("hex")}`;
}

/** Connects the selected plant to the existing declared controller without changing its law. */
export function selectedModelAnalysis(
  declared: DeclaredControlAnalysis,
  selected: GeneralModel | undefined
): DeclaredControlAnalysis {
  if (!selected) return declared;
  if (selected.kind === "discrete-state-space") {
    throw new Error("The selected discrete model needs a sampled-domain controller analysis rather than continuous Routh or root-locus analysis.");
  }
  const selectedTransfer = selected.kind === "continuous-state-space" ? linearTransferModel(selected) : selected;
  if (selectedTransfer.kind !== "transfer-function") throw new Error("The selected model needs a supported continuous transfer conversion before scalar analysis.");
  if (selectedTransfer.inputs[0].unit !== declared.plant.inputUnit || selectedTransfer.outputs[0].unit !== declared.plant.outputUnit) {
    throw new Error("The selected plant units do not match the declared controller. Confirm compatible input/output bindings before analysis.");
  }
  return { ...declared, plant: {
    kind: selectedTransfer.kind, numerator: [...selectedTransfer.numerator], denominator: [...selectedTransfer.denominator],
    delaySeconds: selectedTransfer.delaySeconds, inputUnit: selectedTransfer.inputs[0].unit,
    outputUnit: selectedTransfer.outputs[0].unit, revision: selectedTransfer.revision,
    source: selectedTransfer.source, validation: selectedTransfer.validation,
  } };
}
