import { transferFunctionStateSpace } from "./numerics/stateSpaceRealization";
import { delayedModelRealization } from "./delayedModelRealization";
import { validActuator, type ActuatorCapabilities } from "./loopChannels";
import { decodeGeneralModel, type GeneralModel, type GeneralStateSpaceModel, type GeneralTransferModel } from "./generalModel";

export interface SelectedModelSimulationBinding {
  plantId: string;
  commandSources: readonly string[];
  feedbackTargets: readonly string[];
  actuatorFeedbackTargets?: readonly (string | null)[];
  initialState: readonly number[];
  initialInput: readonly number[];
  initialDelayedInput?: number;
  stepMs: number;
  actuators?: readonly ActuatorCapabilities[];
}

export interface SelectedModelSimulation {
  simulation_override: {
    model_revision: string;
    model_provenance: string;
    simulation_toml: string;
  };
  /** Human-facing only: never forwarded into the strict runtime request. */
  stateCoordinateDescription: string;
}

/**
 * Produces the inline, simulation-only plant consumed by an isolated Verify run.
 * The returned coordinate description remains editor metadata because the runtime
 * request accepts only the three `simulation_override` fields.
 */
export function materializeSelectedModelSimulation(
  value: unknown,
  binding: SelectedModelSimulationBinding,
): SelectedModelSimulation {
  const model = decodeGeneralModel(value);
  if (!model) throw new Error("Select a valid general model before materializing a simulated plant.");
  validateBinding(binding);
  if (binding.actuators && binding.actuators.length !== model.inputs.length) throw new Error("Actuator count must match model inputs.");
  if (model.physical && model.physical.kind !== "thermal-balance") {
    const physical = model.physical;
    validatePlantShape(binding, model.outputs.length, 1, model.outputs.length);
    const absolute = { ...binding, initialState: binding.initialState.map((value, index) => value + model.operatingPoint!.outputs[index]),
      initialInput: binding.initialInput.map((value, index) => value + model.operatingPoint!.inputs[index]) };
    if (absolute.initialState.some(value => value < 0)) throw new Error("Gravity-tank initial levels must remain nonnegative.");
    return { simulation_override: { model_revision: model.revision, model_provenance: `declared-${model.source}-${model.validation}`,
      simulation_toml: simulationToml(absolute, { ...model, operatingPoint: undefined }, physical.kind === "gravity-tank" ? "gravity_tank" : "gravity_tank_cascade", {
        area_m2: physical.areaM2, outflow_coefficient_m2_sqrt_m_per_s: physical.outflowCoefficient,
      }) }, stateCoordinateDescription: "The native gravity balance executes absolute physical levels. Declared initial state deviations are added to the selected equilibrium; linear analysis uses its local Jacobian." };
  }
  const exactScalarDelay = model.kind === "transfer-function" && model.denominator.length === 2 && model.numerator.length === 1 && model.denominator[0] > 0 && model.denominator[1] >= 0;
  const materialized = model.delaySeconds > 0 && !exactScalarDelay
    ? materializeDelayedModel(model, binding)
    : model.kind === "transfer-function" ? materializeTransferFunction(model, binding) : materializeStateSpace(model, binding);
  return {
    simulation_override: {
      model_revision: model.revision,
      model_provenance: `declared-${model.source}-${model.validation}`,
      simulation_toml: materialized.toml,
    },
    stateCoordinateDescription: materialized.coordinates,
  };
}

function materializeDelayedModel(model: GeneralModel, binding: SelectedModelSimulationBinding): { toml: string; coordinates: string } {
  if (model.kind === "discrete-state-space" && Math.abs(model.sampleTimeSeconds! * 1000 - binding.stepMs) > 1e-9) throw new Error("Discrete sample time and simulation step disagree.");
  validatePlantShape(binding, model.kind === "transfer-function" ? model.denominator.length - 1 : model.states.length, model.inputs.length, model.outputs.length);
  const realized = delayedModelRealization(model, binding.stepMs / 1000, binding.initialState, binding.initialInput);
  return { toml: simulationToml({ ...binding, initialState: realized.initialState }, model, "discrete_state_space", {
    a: matrix(realized.a), b: matrix(realized.b), c: matrix(realized.c), d: matrix(realized.d),
  }), coordinates: `Original model coordinates followed by ${realized.history} ordered input-history slots. Exact held-input delay response at task sample instants.` };
}

function materializeStateSpace(
  model: GeneralStateSpaceModel,
  binding: SelectedModelSimulationBinding,
): { toml: string; coordinates: string } {
  if (model.kind === "discrete-state-space" && Math.abs(model.sampleTimeSeconds! * 1000 - binding.stepMs) > 1e-9) {
    throw new Error("The discrete plant update period must equal its declared sample time.");
  }
  if (model.delaySeconds !== 0) {
    throw new Error("A continuous state-space delay simulation adapter is not implemented.");
  }
  validatePlantShape(binding, model.states.length, model.inputs.length, model.outputs.length);
  return {
    toml: simulationToml(binding, model, model.kind === "discrete-state-space" ? "discrete_state_space" : "state_space", {
      a: matrix(model.a), b: matrix(model.b), c: matrix(model.c), d: matrix(model.d),
    }),
    coordinates: `Supplied state coordinates in declared order: ${model.states.map(state => state.id).join(", ")}.`,
  };
}

function materializeTransferFunction(
  model: GeneralTransferModel,
  binding: SelectedModelSimulationBinding,
): { toml: string; coordinates: string } {
  validatePlantShape(binding, model.denominator.length - 1, 1, 1);
  const { order, denominator, numerator, a, b, c, d } = transferFunctionStateSpace(model.numerator, model.denominator);
  if (model.delaySeconds > 0) {
    if (order !== 1 || numerator[0] !== 0 || !Number.isFinite(binding.initialDelayedInput)) {
      throw new Error("An exact delayed simulation adapter is available only for a scalar first-order model with declared input history.");
    }
    // Native scalar-delay adapters retain output deviation, whereas the supplied
    // transfer model declares the companion state. Convert that initial coordinate.
    const outputBinding = { ...binding, initialState: [c[0][0] * binding.initialState[0]] };
    if (denominator[1] === 0) {
      return {
        toml: simulationToml(outputBinding, model, "integrator_dead_time", {
          gain: numerator[1], dead_time_s: model.delaySeconds,
          initial_delayed_input: binding.initialDelayedInput!,
        }),
        coordinates: "Output-deviation state for the delayed integrating model, with the declared input history and operating-point offsets.",
      };
    }
    const timeConstant = 1 / denominator[1];
    const gain = numerator[1] / denominator[1];
    if (!Number.isFinite(timeConstant) || timeConstant <= 0 || !Number.isFinite(gain)) {
      throw new Error("The delayed first-order model is outside the runtime adapter domain.");
    }
    return {
      toml: simulationToml(outputBinding, model, "first_order_dead_time", {
        gain, time_constant_s: timeConstant, dead_time_s: model.delaySeconds,
        initial_delayed_input: binding.initialDelayedInput!,
      }),
      coordinates: "Runtime output-state coordinate for the exact first-order dead-time model; it is not a physical measured-state claim.",
    };
  }
  if (order === 0) {
    if (binding.initialState.length !== 0) throw new Error("A static transfer function requires no initial state coordinates.");
    return {
      toml: simulationToml(binding, model, "static_gain", { gain: numerator[0] }),
      coordinates: "Static transfer function has no dynamic state coordinates.",
    };
  }
  return {
    toml: simulationToml(binding, model, "state_space", { a: matrix(a), b: matrix(b), c: matrix(c), d: matrix(d) }),
    coordinates: `Controllable companion coordinates [${Array.from({ length: order }, (_, index) => `x${index + 1}`).join(", ")}] for the unreduced transfer-function denominator; they are not physical measured states.`,
  };
}

function validateBinding(binding: SelectedModelSimulationBinding): void {
  if (binding.actuatorFeedbackTargets && (binding.actuatorFeedbackTargets.length !== binding.commandSources.length
    || binding.actuatorFeedbackTargets.some(address => address !== null && !address.startsWith("%I")))) throw new Error("Actuator feedback must map ordered input addresses.");
  if (binding.actuators && !binding.actuators.every(validActuator)) throw new Error("Invalid actuator capability or limits.");
  if (!/^[A-Za-z][A-Za-z0-9_-]*$/.test(binding.plantId)) throw new Error("Plant id must match the runtime plant identifier contract.");
  if (!Number.isInteger(binding.stepMs) || binding.stepMs < 1 || binding.stepMs > 1000) throw new Error("Simulation step must be a whole millisecond from 1 to 1000.");
  for (const [kind, addresses, prefix] of [
    ["command", binding.commandSources, "%Q"],
    ["feedback", binding.feedbackTargets, "%I"],
  ] as const) {
    if (addresses.length === 0 || addresses.some(address => typeof address !== "string" || !address.startsWith(prefix))) {
      throw new Error(`Ordered ${kind} bindings must use addressed ${prefix} signals.`);
    }
  }
  for (const values of [binding.initialState, binding.initialInput]) {
    if (values.some(value => !Number.isFinite(value))) throw new Error("Initial state and input values must be finite.");
  }
}

function validatePlantShape(binding: SelectedModelSimulationBinding, states: number, inputs: number, outputs: number): void {
  if (binding.commandSources.length !== inputs || binding.feedbackTargets.length !== outputs
    || binding.initialState.length !== states || binding.initialInput.length !== inputs) {
    throw new Error("Ordered bindings, initial state, and initial input must match the selected model dimensions.");
  }
}

function simulationToml(binding: SelectedModelSimulationBinding, selected: GeneralModel, model: string, parameters: Record<string, string | number>): string {
  return [
    "[simulation]", "enabled = true", `plant_step_ms = ${binding.stepMs}`, "",
    "[[plants]]", `id = ${quote(binding.plantId)}`, `model = ${quote(model)}`,
    `command_sources = ${array(binding.commandSources)}`,
    `feedback_targets = ${array(binding.feedbackTargets)}`,
    ...(binding.actuatorFeedbackTargets?.some(address => address !== null) ? [`actuator_feedback_targets = [${binding.actuatorFeedbackTargets.map(address => address === null ? "{}" : `{ address = ${quote(address)} }`).join(", ")}]`] : []),
    `initial_state = ${array(binding.initialState)}`,
    `initial_input = ${array(binding.initialInput)}`,
    ...(selected.operatingPoint ? [
      `input_offset = ${array(selected.operatingPoint.inputs.map(value => -value))}`,
      `output_offset = ${array(selected.operatingPoint.outputs)}`,
    ] : []),
    "", "[plants.parameters]",
    ...Object.entries(parameters).map(([key, value]) => `${key} = ${typeof value === "number" ? scalar(value) : value}`), "",
    ...(binding.actuators ?? []).flatMap((actuator, index) => {
      const offset = selected.operatingPoint?.inputs[index] ?? 0;
      return ["[[plants.actuators]]", `kind = ${quote(actuator.kind)}`,
        ...(actuator.minimumOnSeconds === undefined ? [] : [`minimum_on_s = ${scalar(actuator.minimumOnSeconds)}`]),
        ...(actuator.minimumOffSeconds === undefined ? [] : [`minimum_off_s = ${scalar(actuator.minimumOffSeconds)}`]), `minimum = ${scalar(actuator.minimum - offset)}`,
        `maximum = ${scalar(actuator.maximum - offset)}`,
        `initial_applied = ${scalar(binding.initialInput[index])}`,
        ...(actuator.rateUpPerS === undefined ? [] : [`rate_up_per_s = ${scalar(actuator.rateUpPerS)}`]),
        ...(actuator.rateDownPerS === undefined ? [] : [`rate_down_per_s = ${scalar(actuator.rateDownPerS)}`]), ""];
    }),
  ].join("\n");
}

function quote(value: string): string { return JSON.stringify(value); }
function scalar(value: number): string {
  if (!Number.isFinite(value)) throw new Error("Simulation values must be finite.");
  return Object.is(value, -0) ? "0" : String(value);
}
function array(values: ReadonlyArray<string | number>): string {
  return `[${values.map(value => typeof value === "string" ? quote(value) : scalar(value)).join(", ")}]`;
}
function matrix(values: ReadonlyArray<ReadonlyArray<number>>): string {
  return `[${values.map(row => array(row)).join(", ")}]`;
}
