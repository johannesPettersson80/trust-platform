import { decodeGeneralModel, type GeneralModel } from "./generalModel";
import type { GeneralFeedbackDefinition } from "./generalFeedback";
import { delayedModelRealization } from "./delayedModelRealization";
import { transferFunctionStateSpace } from "./numerics/stateSpaceRealization";

export interface ModelTimeSettings {
  stepSeconds: number; durationSeconds: number;
  initialState: readonly number[]; input: readonly number[];
  connection: "open-loop" | "feedback";
}
export interface ModelTimeResponse {
  model: GeneralModel; settings: ModelTimeSettings; controller?: GeneralFeedbackDefinition;
  stateCoordinates: readonly string[]; transform: string; scope: string;
  points: readonly { timeSeconds: number; zeroState: readonly number[]; initialCondition: readonly number[]; total: readonly number[] }[];
}
type Realization = ReturnType<typeof delayedModelRealization>;
export function modelTimeSettings(value: unknown, model: GeneralModel): ModelTimeSettings | undefined {
  if (!record(value) || !finite(value.stepSeconds) || value.stepSeconds <= 0 || !finite(value.durationSeconds) || value.durationSeconds <= 0
    || !["open-loop", "feedback"].includes(String(value.connection)) || !vector(value.input, model.inputs.length)
    || !vector(value.initialState, model.kind === "transfer-function" ? model.denominator.length - 1 : model.states.length)) return undefined;
  const count = value.durationSeconds / value.stepSeconds;
  if (!Number.isFinite(count) || count < 1 || (Math.floor(count) + 1) * model.outputs.length > 4096
    || Math.abs(count - Math.round(count)) > 1e-8
    || model.kind === "discrete-state-space" && Math.abs(value.stepSeconds - model.sampleTimeSeconds!) > 1e-10) return undefined;
  return { stepSeconds: value.stepSeconds, durationSeconds: value.durationSeconds, input: [...value.input], initialState: [...value.initialState], connection: value.connection as ModelTimeSettings["connection"] };
}
/** Reuses the same held-input delay realization as the native selected-model adapter. */
export function analyzeGeneralModelTime(value: unknown, rawSettings: unknown, feedback?: GeneralFeedbackDefinition): ModelTimeResponse {
  const model = decodeGeneralModel(value);
  if (!model) throw new Error("Select a valid model before calculating its response.");
  const settings = modelTimeSettings(rawSettings, model);
  if (!settings) throw new Error("Choose initial coordinates and inputs matching the model, its sample period, and at most 4096 output samples.");
  const plant = sampled(model, settings.stepSeconds, settings.initialState);
  let controller: Realization | undefined;
  if (settings.connection === "feedback") {
    if (!feedback || model.inputs.length !== 1 || model.outputs.length !== 1) throw new Error("Select a compatible scalar feedback controller for this response.");
    let law = feedback.controller;
    if (feedback.declaredController) {
      const declared = feedback.declaredController;
      if (declared.numerator.length > declared.denominator.length) throw new Error("Time response requires the controller's proper derivative filter model.");
      law = { ...law, kind: "transfer-function", numerator: declared.numerator, denominator: declared.denominator, delaySeconds: 0 };
    }
    if (law.inputs.length !== 1 || law.outputs.length !== 1) throw new Error("This feedback response requires one controller input and output.");
    controller = sampled(law, settings.stepSeconds, Array(law.kind === "transfer-function" ? law.denominator.length - 1 : law.states.length).fill(0));
  }
  let forcedPlant = plant.initialState.map(() => 0), initialPlant = [...plant.initialState];
  let forcedController = controller?.initialState.map(() => 0) ?? [], initialController = [...forcedController];
  const points: ModelTimeResponse["points"][number][] = [];
  for (let i = 0; i <= Math.round(settings.durationSeconds / settings.stepSeconds); i++) {
    const forced = advance(plant, forcedPlant, settings.input, controller, forcedController);
    const initial = advance(plant, initialPlant, settings.input.map(() => 0), controller, initialController);
    const total = forced.output.map((v, j) => v + initial.output[j]);
    if (![...forced.output, ...initial.output, ...total, ...forced.state, ...initial.state].every(Number.isFinite)) throw new Error("The response exceeded finite model coordinates.");
    points.push({ timeSeconds: i * settings.stepSeconds, zeroState: forced.output, initialCondition: initial.output, total });
    forcedPlant = forced.state; initialPlant = initial.state;
    forcedController = forced.controllerState; initialController = initial.controllerState;
  }
  return { model, settings, ...(feedback && controller ? { controller: feedback } : {}),
    stateCoordinates: model.kind === "transfer-function" ? settings.initialState.map((_, i) => `Companion state x${i + 1}`) : model.states.map(s => `${s.id} (${s.unit})`),
    transform: controller ? "zX(z) − z x(0) = A_cl X(z) + B_cl R(z)"
      : model.kind === "discrete-state-space" ? "Y(z) = [C(zI−A)⁻¹B + D] z⁻ᴸ U(z) + C(zI−A)⁻¹ z x(0)"
        : "Y(s) = [C(sI−A)⁻¹B + D] e⁻ˢᴸ U(s) + C(sI−A)⁻¹ x(0)",
    scope: controller ? "Sampled negative feedback with held inputs and the declared delay. Controller states and prior input history start at zero. Curves are deviations; actuator limits and nonlinear effects are checked by actual-ST validation."
      : "Linear-model output deviations. The zero-state input response includes the declared delay; the separate initial-state response retains its original time origin. Prior input history is zero. Runtime limits and nonlinear validity remain separate.", points };
}
function sampled(model: GeneralModel, step: number, state: readonly number[]): Realization {
  if (model.kind === "discrete-state-space" && Math.abs(model.sampleTimeSeconds! - step) > 1e-10) throw new Error("Controller and plant sample periods must match.");
  if (model.kind === "transfer-function" && model.denominator.length === 1 && model.delaySeconds === 0) {
    return { ...transferFunctionStateSpace(model.numerator, model.denominator), initialState: [], history: 0 };
  }
  return delayedModelRealization(model, step, state, model.inputs.map(() => 0));
}
function advance(plant: Realization, state: readonly number[], reference: readonly number[], controller: Realization | undefined, controllerState: readonly number[]) {
  let input = [...reference], nextController: number[] = [];
  if (controller) {
    const denominator = 1 + controller.d[0][0] * plant.d[0][0];
    if (Math.abs(denominator) < 1e-12) throw new Error("The selected feedback has a singular direct-feedthrough connection.");
    const u = (dot(controller.c[0], controllerState) + controller.d[0][0] * (reference[0] - dot(plant.c[0], state))) / denominator;
    input = [u];
    const error = reference[0] - dot(plant.c[0], state) - plant.d[0][0] * u;
    nextController = controller.a.map((row, i) => dot(row, controllerState) + controller.b[i][0] * error);
  }
  return { output: plant.c.map((row, i) => dot(row, state) + dot(plant.d[i], input)),
    state: plant.a.map((row, i) => dot(row, state) + dot(plant.b[i], input)), controllerState: nextController };
}
export function isModelTimeResponse(value: unknown, selected: GeneralModel): value is ModelTimeResponse {
  if (!record(value) || JSON.stringify(value.model) !== JSON.stringify(selected) || !modelTimeSettings(value.settings, selected)
    || !Array.isArray(value.stateCoordinates) || !value.stateCoordinates.every(v => typeof v === "string")
    || typeof value.transform !== "string" || typeof value.scope !== "string" || !Array.isArray(value.points)) return false;
  const settings = modelTimeSettings(value.settings, selected)!;
  return value.points.length === Math.round(settings.durationSeconds / settings.stepSeconds) + 1
    && value.points.every((p, i) => record(p) && p.timeSeconds === i * settings.stepSeconds && vector(p.zeroState, selected.outputs.length)
      && vector(p.initialCondition, selected.outputs.length) && vector(p.total, selected.outputs.length));
}
function dot(a: readonly number[], b: readonly number[]): number { return a.reduce((sum, v, i) => sum + v * b[i], 0); }
function vector(v: unknown, n: number): v is number[] { return Array.isArray(v) && v.length === n && v.every(finite); }
function finite(v: unknown): v is number { return typeof v === "number" && Number.isFinite(v); }
function record(v: unknown): v is Record<string, unknown> { return !!v && typeof v === "object" && !Array.isArray(v); }
