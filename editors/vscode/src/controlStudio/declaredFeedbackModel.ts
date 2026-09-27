import { decodeGeneralFeedback, type DeclaredFeedbackController, type GeneralFeedbackDefinition } from "./generalFeedback";
import { decodeGeneralModel, type GeneralModel, type GeneralTransferModel } from "./generalModel";
import { decodeDeclaredAnalysis, type DeclaredControlAnalysis, type DeclaredCoefficient } from "./declaredAnalysis";
import { linearTransferModel } from "./linearTransferModel";

/**
 * Adapts one explicitly declared controller equation to the shared feedback
 * analysis contract. The selected model supplies the plant channels and
 * operating ranges; controller coefficients are evaluated only at the
 * declared selected parameter values.
 */
export function declaredFeedbackDefinition(
  value: DeclaredControlAnalysis,
  selectedPlant: GeneralModel,
): GeneralFeedbackDefinition | undefined {
  const analysis = decodeDeclaredAnalysis(value);
  const plant = decodeGeneralModel(selectedPlant);
  if (!analysis || !plant || plant.inputs.length !== 1 || plant.outputs.length !== 1) return undefined;
  if (plant.kind === "discrete-state-space") return undefined;
  let analysisPlant: GeneralTransferModel;
  try {
    if (plant.kind === "continuous-state-space") analysisPlant = linearTransferModel(plant);
    else if (plant.kind === "transfer-function") analysisPlant = plant;
    else return undefined;
  }
  catch { return undefined; }
  if (!analysisPlant || analysisPlant.inputs[0].unit !== analysis.plant.inputUnit || analysisPlant.outputs[0].unit !== analysis.plant.outputUnit) return undefined;

  const evaluate = (coefficient: DeclaredCoefficient): number => typeof coefficient === "number" ? coefficient
    : (coefficient.scale ?? 1) * (coefficient.parameter === analysis.rootLocus.parameter
      ? analysis.rootLocus.selectedValue : analysis.controller.parameters[coefficient.parameter].value);
  const numerator = analysis.controller.numerator.map(evaluate);
  const denominator = analysis.controller.denominator.map(evaluate);
  if (!numerator.every(Number.isFinite) || !denominator.every(Number.isFinite) || denominator[0] === 0) return undefined;

  const controller: GeneralTransferModel = {
    kind: "transfer-function",
    numerator: [1],
    denominator: [1],
    delaySeconds: 0,
    revision: `${analysis.plant.revision}:declared-controller-analysis`,
    source: analysis.plant.source,
    validation: analysis.plant.validation,
    evidence: [`declared-controller-law:${analysis.plant.revision}`],
    inputs: [{ id: `${plant.outputs[0].id}:feedback`, unit: plant.outputs[0].unit, operatingRange: { ...plant.outputs[0].operatingRange } }],
    outputs: [{ id: `${plant.inputs[0].id}:command`, unit: plant.inputs[0].unit, operatingRange: { ...plant.inputs[0].operatingRange } }],
  };
  const declaredController: DeclaredFeedbackController = {
    kind: "declared-rational", numerator, denominator, parameter: analysis.rootLocus.parameter,
    selectedValue: analysis.rootLocus.selectedValue, revision: analysis.plant.revision,
    inputUnit: plant.outputs[0].unit, outputUnit: plant.inputs[0].unit,
  };
  return decodeGeneralFeedback({
    plant, controller, declaredController,
    plantChannel: { inputId: plant.inputs[0].id, outputId: plant.outputs[0].id },
    controllerChannel: { inputId: controller.inputs[0].id, outputId: controller.outputs[0].id },
    feedback: "negative" as const,
  });
}
