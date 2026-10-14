import { decodeGeneralModel, type GeneralFrequencyPoint, type GeneralModel } from "./generalModel";
import type { ComplexValue } from "./numerics";

/** The feedback definition of a selected model and its declared controller, with the decoding of its saved evidence.
 * The feedback analysis is the engine's (`analysis/feedback_tool.rs`, `analysis/feedback_margins.rs`). */
export interface GeneralFeedbackDefinition {
  plant: GeneralModel;
  controller: GeneralModel;
  plantChannel: { inputId: string; outputId: string };
  controllerChannel: { inputId: string; outputId: string };
  feedback: "negative";
  declaredController?: DeclaredFeedbackController;
}
export interface DeclaredFeedbackController {
  kind: "declared-rational";
  numerator: readonly number[];
  denominator: readonly number[];
  parameter: string;
  selectedValue: number;
  revision: string;
  inputUnit: string;
  outputUnit: string;
}
export interface GeneralFeedbackResult {
  status: "qualified";
  definition: GeneralFeedbackDefinition;
  openLoop: readonly GeneralFrequencyPoint[];
  nyquistScope: string;
  sensitivity?: readonly {
    frequencyRadPerSecond: number;
    sensitivityMagnitudeDb: number;
    complementarySensitivityMagnitudeDb: number;
    controlSensitivityMagnitudeDb: number;
  }[];
  nyquistStability?: { status: "qualified"; encirclementsCounterClockwise: number; openLoopRightHalfPlanePoles: number; closedLoopRightHalfPlanePoles: number; closedLoopStable: boolean; minimumDistanceToMinusOne: number; endpointMagnitude: number }
    | { status: "rejected"; reason: string };
  closedLoop: { status: "qualified"; poles: readonly ComplexValue[]; stable: boolean; scope: string }
    | { status: "unavailable"; reason: string };
  margins: { status: "estimated"; gainCrossoverRadPerSecond: number; phaseMarginDegrees: number; gainMarginDb?: number; scope: string }
    | { status: "unavailable"; reason: string };
}

export function decodeGeneralFeedback(value: unknown): GeneralFeedbackDefinition | undefined {
  if (!record(value) || value.feedback !== "negative" || !record(value.plantChannel) || !record(value.controllerChannel)) return undefined;
  const plant = decodeGeneralModel(value.plant); const controller = decodeGeneralModel(value.controller);
  if (!plant || !controller) return undefined;
  const pi = plant.inputs.find(c => c.id === (value.plantChannel as Record<string, unknown>).inputId);
  const po = plant.outputs.find(c => c.id === (value.plantChannel as Record<string, unknown>).outputId);
  const ci = controller.inputs.find(c => c.id === (value.controllerChannel as Record<string, unknown>).inputId);
  const co = controller.outputs.find(c => c.id === (value.controllerChannel as Record<string, unknown>).outputId);
  if (!pi || !po || !ci || !co || pi.unit !== co.unit || po.unit !== ci.unit) return undefined;
  if ((plant.kind === "discrete-state-space") !== (controller.kind === "discrete-state-space")) return undefined;
  if (plant.kind === "discrete-state-space" && controller.kind === "discrete-state-space" && plant.sampleTimeSeconds !== controller.sampleTimeSeconds) return undefined;
  const declaredController = value.declaredController === undefined ? undefined : decodeDeclaredController(value.declaredController);
  if (value.declaredController !== undefined && (!declaredController || declaredController.inputUnit !== po.unit || declaredController.outputUnit !== pi.unit)) return undefined;
  return { plant, controller, plantChannel: { inputId: pi.id, outputId: po.id }, controllerChannel: { inputId: ci.id, outputId: co.id }, feedback: "negative", ...(declaredController ? { declaredController } : {}) };
}

function decodeDeclaredController(value: unknown): DeclaredFeedbackController | undefined {
  if (!record(value) || value.kind !== "declared-rational" || !Array.isArray(value.numerator) || !Array.isArray(value.denominator)
      || value.numerator.length === 0 || value.denominator.length === 0 || !value.numerator.every(finite)
      || !value.denominator.every(finite) || value.denominator[0] === 0 || typeof value.parameter !== "string"
      || !finite(value.selectedValue) || typeof value.revision !== "string" || typeof value.inputUnit !== "string" || typeof value.outputUnit !== "string") return undefined;
  return value as unknown as DeclaredFeedbackController;
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
