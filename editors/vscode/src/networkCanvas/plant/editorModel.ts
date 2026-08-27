import {
  persistedPlantUncertainties,
  type PlantUncertaintyDraft,
} from "./uncertaintyEditor";
import {
  persistedPlantScenarioEvents,
  type PlantScenarioEventDraft,
} from "./scenarioEventEditor";

export type PlantStartingPoint =
  | "tank"
  | "temperature"
  | "motor_speed"
  | "mechanical_position"
  | "static_scaling"
  | "nonlinear_characteristic"
  | "custom_state_space";

export type PlantModelKind =
  | "integrator"
  | "first_order_dead_time"
  | "first_order"
  | "second_order"
  | "static_gain"
  | "piecewise_linear"
  | "state_space";

export type PlantParameterValue = number | number[] | number[][];
export type PlantParameters = Record<string, PlantParameterValue>;
export type PlantActuatorSettings = Partial<Record<
  | "minimum"
  | "maximum"
  | "rate_up_per_s"
  | "rate_down_per_s"
  | "deadband"
  | "lag_s"
  | "initial_applied",
  number
>>;

const actuatorField = (
  key: keyof PlantActuatorSettings,
  label: string,
  plainTerm: string,
  controlTerm: string,
  unit: string,
  effect: string
) => ({ key, label, plainTerm, controlTerm, unit, effect, storedKey: `actuator.${key}` });

export const PLANT_ACTUATOR_FIELDS = [
  actuatorField("minimum", "Minimum", "Lowest actuator output", "Lower saturation limit", "output units", "A higher minimum prevents the actuator from applying smaller values."),
  actuatorField("maximum", "Maximum", "Highest actuator output", "Upper saturation limit", "output units", "A lower maximum makes the actuator saturate sooner on increasing commands."),
  actuatorField("rate_up_per_s", "Rising rate per second", "Maximum opening speed", "Positive slew-rate limit", "output units per second", "A smaller value makes increasing actuator movement slower; zero prevents upward movement."),
  actuatorField("rate_down_per_s", "Falling rate per second", "Maximum closing speed", "Negative slew-rate limit", "output units per second", "A smaller value makes decreasing actuator movement slower; zero prevents downward movement."),
  actuatorField("deadband", "Deadband", "Ignored command movement", "Actuator deadband", "command units", "A larger value ignores a wider band of small command changes."),
  actuatorField("lag_s", "Actuator lag (s)", "Actuator response speed", "Actuator time constant", "seconds", "A larger value makes the applied actuator output respond more slowly."),
  actuatorField("initial_applied", "Initial applied value", "Starting actuator output", "Initial applied output", "output units", "This is the actuator output held at simulation start before command dynamics advance."),
] as const;

export function validatePlantActuator(value: PlantActuatorSettings): PlantDynamicsIssue[] {
  const issues: PlantDynamicsIssue[] = [];
  for (const [key, current] of Object.entries(value)) {
    if (!Number.isFinite(current)) {
      issues.push({ field: key, message: "Enter a finite number or leave this field empty." });
    }
  }
  for (const [key, label] of [
    ["rate_up_per_s", "Rising rate"],
    ["rate_down_per_s", "Falling rate"],
    ["deadband", "Deadband"],
    ["lag_s", "Actuator lag"],
  ] as const) {
    if (value[key] !== undefined && value[key]! < 0) {
      issues.push({ field: key, message: `${label} must be 0 or greater.` });
    }
  }
  if (value.minimum !== undefined && value.maximum !== undefined && value.minimum > value.maximum) {
    issues.push({ field: "maximum", message: "Maximum must be greater than or equal to minimum." });
  } else if (
    value.initial_applied !== undefined &&
    ((value.minimum !== undefined && value.initial_applied < value.minimum) ||
      (value.maximum !== undefined && value.initial_applied > value.maximum))
  ) {
    issues.push({ field: "initial_applied", message: "Initial applied value must be within minimum and maximum." });
  }
  return issues;
}

export interface PlantDynamicsField {
  key: string;
  label: string;
  valueKind: "number" | "vector" | "matrix";
  plainTerm: string;
  controlTerm: string;
  unit: string;
  effect: string;
  storedKey: string;
  optional?: boolean;
}

function dynamicsField(
  key: string,
  label: string,
  valueKind: PlantDynamicsField["valueKind"],
  plainTerm: string,
  controlTerm: string,
  unit: string,
  effect: string,
  optional = false
): PlantDynamicsField {
  return {
    key,
    label,
    valueKind,
    plainTerm,
    controlTerm,
    unit,
    effect,
    storedKey: key === "initial_state" ? "initial_state" : `parameters.${key}`,
    optional,
  };
}

const GAIN = () => dynamicsField("gain", "Gain", "number", "Response size", "Process gain", "output per input", "A larger magnitude produces a larger output change for the same input change.");
const INPUT_BIAS = () => dynamicsField("input_bias", "Input bias", "number", "Input resting point", "Input operating-point bias", "input units", "The model responds to command changes relative to this value.");
const OUTPUT_BIAS = () => dynamicsField("output_bias", "Output bias", "number", "Output resting point", "Output operating-point bias", "output units", "This offsets the model output without changing its dynamic response.");
const INITIAL_STATE = (label = "Initial state") => dynamicsField("initial_state", label, "vector", "Starting internal value", "Initial state vector", "state units", "This is the model state at simulation start or reset.");

const DYNAMICS_FIELDS: Readonly<Record<PlantModelKind, readonly PlantDynamicsField[]>> = {
  static_gain: [
    GAIN(), INPUT_BIAS(), OUTPUT_BIAS(),
  ],
  first_order: [
    GAIN(),
    dynamicsField("time_constant_s", "Time constant (s)", "number", "Response speed", "Time constant", "s", "A larger value makes the response slower; about 63% of a step is reached after one time constant."),
    INPUT_BIAS(), OUTPUT_BIAS(), INITIAL_STATE(),
  ],
  first_order_dead_time: [
    GAIN(),
    dynamicsField("time_constant_s", "Time constant (s)", "number", "Response speed", "Time constant", "s", "A larger value makes the response slower after the delay has elapsed."),
    dynamicsField("dead_time_s", "Dead time (s)", "number", "Waiting time", "Transport delay", "s", "The output does not react to a new input until this delay has elapsed."),
    INPUT_BIAS(), OUTPUT_BIAS(),
    dynamicsField("initial_delayed_input", "Initial delayed input", "number", "Input already in transit", "Initial delay-line value", "input units", "This fills the delay before the first new command reaches the model."),
    INITIAL_STATE(),
  ],
  integrator: [
    dynamicsField("gain", "Integral gain", "number", "Accumulation rate", "Integrator gain", "output per input-second", "A larger magnitude stores change faster while the input differs from its bias."),
    INPUT_BIAS(),
    OUTPUT_BIAS(),
    dynamicsField("state_minimum", "Minimum state", "number", "Lowest stored value", "Lower integrator clamp", "state units", "When set, the accumulated state cannot fall below this value; empty means unbounded.", true),
    dynamicsField("state_maximum", "Maximum state", "number", "Highest stored value", "Upper integrator clamp", "state units", "When set, the accumulated state cannot rise above this value; empty means unbounded.", true),
    INITIAL_STATE("Initial stored value"),
  ],
  second_order: [
    GAIN(),
    dynamicsField("natural_frequency_rad_s", "Natural frequency (rad/s)", "number", "Natural response speed", "Undamped natural frequency", "rad/s", "A larger value moves the characteristic response and resonance to a higher frequency."),
    dynamicsField("damping_ratio", "Damping ratio", "number", "Oscillation decay", "Damping ratio", "ratio", "Values below 1 can oscillate; larger values reduce overshoot and resonance."),
    INPUT_BIAS(), OUTPUT_BIAS(), INITIAL_STATE("Initial states"),
  ],
  piecewise_linear: [
    dynamicsField("input_points", "Input points", "vector", "Curve input coordinates", "Piecewise-linear abscissas", "input units", "Inputs must increase strictly; values between points are interpolated."),
    dynamicsField("output_points", "Output points", "vector", "Curve output coordinates", "Piecewise-linear ordinates", "output units", "Each value is paired with the input point at the same row."),
  ],
  state_space: [
    dynamicsField("a", "A matrix", "matrix", "State-to-state dynamics", "State matrix A", "state rate per state", "Defines how the current states drive their own rates of change."),
    dynamicsField("b", "B matrix", "matrix", "Input-to-state dynamics", "Input matrix B", "state rate per input", "Defines how each command drives each state."),
    dynamicsField("c", "C matrix", "matrix", "State-to-output mapping", "Output matrix C", "output per state", "Defines how states contribute to each process output."),
    dynamicsField("d", "D matrix", "matrix", "Immediate input-to-output mapping", "Feedthrough matrix D", "output per input", "Defines output that changes immediately with the input."),
    INITIAL_STATE(),
  ],
};

export function plantDynamicsFields(model: string): readonly PlantDynamicsField[] {
  return DYNAMICS_FIELDS[model as PlantModelKind] ?? [];
}

export function startingPointForPlantModel(model: PlantModelKind): PlantStartingPoint {
  const match = PLANT_STARTING_POINTS.find((entry) => entry.model === model);
  return match?.id ?? "custom_state_space";
}

export interface PlantDynamicsIssue {
  field: string;
  message: string;
}

/** Converts the visible editor values to the existing trust.plant candidate shape. */
export function plantCandidateFromEditorDraft(
  draft: Pick<
    PlantEditorDraft,
    | "plantId"
    | "displayName"
    | "description"
    | "enabled"
    | "model"
    | "commandSources"
    | "feedbackTargets"
    | "stepMs"
    | "initialInput"
    | "inputScale"
    | "inputOffset"
    | "outputScale"
    | "outputOffset"
    | "parameters"
    | "initialState"
    | "actuator"
    | "uncertainties"
    | "scenarioEvents"
  >
): Record<string, unknown> {
  const candidate: Record<string, unknown> = {
    id: draft.plantId,
    name: draft.displayName,
    description: draft.description,
    enabled: draft.enabled,
    model: draft.model,
    command_sources: [...draft.commandSources],
    feedback_targets: [...draft.feedbackTargets],
    parameters: { ...draft.parameters },
    initial_state: [...draft.initialState],
  };
  if (draft.stepMs !== undefined) candidate.step_ms = draft.stepMs;
  for (const [key, current] of [
    ["initial_input", draft.initialInput],
    ["input_scale", draft.inputScale],
    ["input_offset", draft.inputOffset],
    ["output_scale", draft.outputScale],
    ["output_offset", draft.outputOffset],
  ] as const) {
    if (current !== undefined) candidate[key] = [...current];
  }
  if (draft.actuator && Object.keys(draft.actuator).length > 0) {
    candidate.actuator = { ...draft.actuator };
  }
  if (draft.uncertainties) candidate.uncertainties = persistedPlantUncertainties(draft.uncertainties);
  candidate.scenario_events = persistedPlantScenarioEvents(draft.scenarioEvents ?? []);
  return candidate;
}

export function validatePlantDynamics(input: {
  model: string;
  parameters: PlantParameters;
  initialState: number[];
}): PlantDynamicsIssue[] {
  const issues: PlantDynamicsIssue[] = [];
  const value = (key: string): PlantParameterValue | undefined =>
    key === "initial_state" ? input.initialState : input.parameters[key];

  for (const field of plantDynamicsFields(input.model)) {
    const current = value(field.key);
    if (field.optional && current === undefined) continue;
    const numbers = Array.isArray(current) ? current.flat(Infinity) : [current];
    if (
      current === undefined ||
      numbers.length === 0 ||
      numbers.some((entry) => typeof entry !== "number" || !Number.isFinite(entry))
    ) {
      issues.push({ field: field.key, message: `${field.label} must contain finite numbers.` });
    }
  }

  const scalar = (key: string): number | undefined => {
    const current = input.parameters[key];
    return typeof current === "number" && Number.isFinite(current) ? current : undefined;
  };
  if (scalar("time_constant_s") !== undefined && scalar("time_constant_s")! <= 0) {
    issues.push({ field: "time_constant_s", message: "Time constant must be greater than 0 s." });
  }
  if (scalar("dead_time_s") !== undefined && scalar("dead_time_s")! < 0) {
    issues.push({ field: "dead_time_s", message: "Dead time must be 0 s or greater." });
  }
  if (
    scalar("natural_frequency_rad_s") !== undefined &&
    scalar("natural_frequency_rad_s")! <= 0
  ) {
    issues.push({
      field: "natural_frequency_rad_s",
      message: "Natural frequency must be greater than 0 rad/s.",
    });
  }
  if (scalar("damping_ratio") !== undefined && scalar("damping_ratio")! < 0) {
    issues.push({ field: "damping_ratio", message: "Damping ratio must be 0 or greater." });
  }
  if (
    scalar("state_minimum") !== undefined &&
    scalar("state_maximum") !== undefined &&
    scalar("state_minimum")! > scalar("state_maximum")!
  ) {
    issues.push({
      field: "state_maximum",
      message: "Maximum state must be greater than or equal to minimum state.",
    });
  }

  if (input.model === "piecewise_linear") {
    const inputs = input.parameters.input_points;
    const outputs = input.parameters.output_points;
    if (Array.isArray(inputs) && Array.isArray(outputs) && inputs.length !== outputs.length) {
      issues.push({
        field: "output_points",
        message: "Input and output points must contain the same number of values.",
      });
    }
    if (Array.isArray(inputs)) {
      for (let index = 1; index < inputs.length; index += 1) {
        if (
          typeof inputs[index - 1] === "number" &&
          typeof inputs[index] === "number" &&
          inputs[index] <= inputs[index - 1]
        ) {
          issues.push({
            field: `input_points.${index}`,
            message: `Input point ${index + 1} must be greater than input point ${index}.`,
          });
          break;
        }
      }
    }
  }
  return issues;
}

export interface PlantStartingPointDescriptor {
  id: PlantStartingPoint;
  label: string;
  model: PlantModelKind;
  parameters: Readonly<Record<string, number | readonly number[]>>;
  initialState: readonly number[];
  whyReasonable: string;
  limitation: string;
}

export const CUSTOM_STATE_SPACE_HELP = {
  whyReasonable:
    "Coupled linear states represent multi-state or multi-channel linear dynamics.",
  limitation:
    "It cannot reproduce nonlinear or time-varying behavior, and delay requires an explicit state approximation.",
} as const;

// These are deliberately conservative, unit-neutral simulation seeds. They
// make every generated value visible and editable; they are not plant-ID or
// identification results.
export const PLANT_STARTING_POINTS: readonly PlantStartingPointDescriptor[] = [
  {
    id: "tank",
    label: "Tank or accumulated level",
    model: "integrator",
    parameters: { gain: 1, input_bias: 0, output_bias: 0 },
    initialState: [0],
    whyReasonable: "It accumulates the difference between inflow and outflow.",
    limitation:
      "It cannot settle by itself or reproduce leakage unless those effects are added.",
  },
  {
    id: "temperature",
    label: "Temperature or slow process",
    model: "first_order_dead_time",
    parameters: {
      gain: 1,
      time_constant_s: 10,
      dead_time_s: 1,
      input_bias: 0,
      output_bias: 0,
      initial_delayed_input: 0,
    },
    initialState: [0],
    whyReasonable:
      "One dominant lag and a transport delay approximate many heating processes.",
    limitation:
      "It cannot reproduce resonance, multiple time scales, or nonlinear heating.",
  },
  {
    id: "motor_speed",
    label: "Motor speed or first-order process",
    model: "first_order",
    parameters: { gain: 1, time_constant_s: 1, input_bias: 0, output_bias: 0 },
    initialState: [0],
    whyReasonable:
      "One dominant lag approximates speed response when electrical or mechanical dynamics dominate.",
    limitation:
      "It cannot reproduce resonance, transport delay, saturation, or friction.",
  },
  {
    id: "mechanical_position",
    label: "Mechanical position or resonant process",
    model: "second_order",
    parameters: {
      gain: 1,
      natural_frequency_rad_s: 1,
      damping_ratio: 0.7,
      input_bias: 0,
      output_bias: 0,
    },
    initialState: [0, 0],
    whyReasonable:
      "Two states represent inertia and damping, including oscillatory response.",
    limitation:
      "It cannot reproduce backlash, friction, hard stops, or other nonlinear mechanics.",
  },
  {
    id: "static_scaling",
    label: "Static scaling",
    model: "static_gain",
    parameters: { gain: 1, input_bias: 0, output_bias: 0 },
    initialState: [],
    whyReasonable:
      "An immediate gain represents a sensor, conversion, or process with negligible dynamics.",
    limitation: "It cannot reproduce stored energy, lag, delay, or oscillation.",
  },
  {
    id: "nonlinear_characteristic",
    label: "Nonlinear characteristic",
    model: "piecewise_linear",
    parameters: { input_points: [-1, 0, 1], output_points: [-1, 0, 1] },
    initialState: [],
    whyReasonable:
      "Ordered points represent a known static nonlinear input/output curve.",
    limitation: "It cannot reproduce dynamics, hysteresis, or rate dependence.",
  },
] as const;

export function proposePlantStartingPoint(id: string): {
  model: PlantModelKind;
  parameters: PlantParameters;
  initialState: number[];
} | undefined {
  const descriptor = PLANT_STARTING_POINTS.find((candidate) => candidate.id === id);
  if (!descriptor) return undefined;
  return {
    model: descriptor.model,
    parameters: Object.fromEntries(
      Object.entries(descriptor.parameters).map(([key, value]) => [
        key,
        typeof value === "number" ? value : Array.from(value),
      ])
    ),
    initialState: [...descriptor.initialState],
  };
}

export function proposeCustomStateSpace(): {
  startingPoint: "custom_state_space";
  model: "state_space";
  parameters: PlantParameters;
  initialState: number[];
} {
  return {
    startingPoint: "custom_state_space",
    model: "state_space",
    parameters: {
      a: [[0]],
      b: [[1]],
      c: [[1]],
      d: [[0]],
    },
    initialState: [0],
  };
}

function startingPointProductName(id: string): string {
  if (id === "custom_state_space") return "Custom state space";
  return PLANT_STARTING_POINTS.find((candidate) => candidate.id === id)?.label ?? id;
}

export function planPlantStartingPointChange(input: {
  currentStartingPoint: string;
  requestedStartingPoint: string;
  modelValuesTouched: boolean;
}):
  | { kind: "apply"; requestedStartingPoint: string }
  | {
      kind: "confirm";
      currentStartingPoint: string;
      requestedStartingPoint: string;
      message: string;
    } {
  if (!input.modelValuesTouched) {
    return { kind: "apply", requestedStartingPoint: input.requestedStartingPoint };
  }
  return {
    kind: "confirm",
    currentStartingPoint: input.currentStartingPoint,
    requestedStartingPoint: input.requestedStartingPoint,
    message: `Replace your edited ${startingPointProductName(
      input.currentStartingPoint
    )} dynamics with the ${startingPointProductName(
      input.requestedStartingPoint
    )} starting values?`,
  };
}

export interface PlantEditorDraft {
  plantId: string;
  displayName: string;
  description: string;
  runtimeId: string;
  runtimeName: string;
  enabled: boolean;
  startingPoint: PlantStartingPoint;
  model: PlantModelKind;
  parameters: PlantParameters;
  initialState: number[];
  actuator?: PlantActuatorSettings;
  uncertainties?: PlantUncertaintyDraft[];
  scenarioEvents?: PlantScenarioEventDraft[];
  modelValuesTouched: boolean;
  commandSources: string[];
  feedbackTargets: string[];
  stepMs?: number;
  initialInput?: number[];
  inputScale?: number[];
  inputOffset?: number[];
  outputScale?: number[];
  outputOffset?: number[];
  dirty: boolean;
}

export function validatePlantSignalEngineering(input: Pick<
  PlantEditorDraft,
  "commandSources" | "feedbackTargets" | "stepMs" | "initialInput" |
  "inputScale" | "inputOffset" | "outputScale" | "outputOffset"
>): PlantDynamicsIssue[] {
  const issues: PlantDynamicsIssue[] = [];
  if (input.stepMs !== undefined && (!Number.isSafeInteger(input.stepMs) || input.stepMs <= 0)) {
    issues.push({ field: "step_ms", message: "Plant step must be a positive whole number of milliseconds." });
  }
  const arrays = [
    ["initial_input", "Initial input", input.initialInput, input.commandSources.length, "command"],
    ["input_scale", "Input scale", input.inputScale, input.commandSources.length, "command"],
    ["input_offset", "Input offset", input.inputOffset, input.commandSources.length, "command"],
    ["output_scale", "Output scale", input.outputScale, input.feedbackTargets.length, "feedback"],
    ["output_offset", "Output offset", input.outputOffset, input.feedbackTargets.length, "feedback"],
  ] as const;
  for (const [field, label, values, expected, signal] of arrays) {
    if (values === undefined) continue;
    if (values.some((value) => !Number.isFinite(value))) {
      issues.push({ field, message: `${label} must contain only finite numbers.` });
    } else if (values.length !== expected) {
      issues.push({
        field,
        message: `${label} needs exactly ${expected} value${expected === 1 ? "" : "s"} for the ${expected} ${signal} signal${expected === 1 ? "" : "s"}.`,
      });
    }
  }
  return issues;
}

export interface PlantIdentityIssue {
  field: "id" | "displayName";
  code: "invalid_plant_id" | "duplicate_plant_id" | "empty_display_name";
  message: string;
}

function uniqueSuggestedId(existingPlantIds: readonly string[]): string {
  const existing = new Set(existingPlantIds);
  if (!existing.has("new_plant")) return "new_plant";
  let suffix = 2;
  while (existing.has(`new_plant_${suffix}`)) suffix += 1;
  return `new_plant_${suffix}`;
}

export function createPlantEditorDraft(input: {
  runtimeId: string;
  runtimeName: string;
  existingPlantIds: string[];
}): PlantEditorDraft {
  const proposal = proposePlantStartingPoint("temperature");
  if (!proposal) throw new Error("Temperature starting point is unavailable");
  return {
    plantId: uniqueSuggestedId(input.existingPlantIds),
    displayName: "New simulated plant",
    description: "",
    runtimeId: input.runtimeId,
    runtimeName: input.runtimeName,
    enabled: true,
    startingPoint: "temperature",
    ...proposal,
    modelValuesTouched: false,
    commandSources: [],
    feedbackTargets: [],
    initialInput: [],
    inputScale: [],
    inputOffset: [],
    outputScale: [],
    outputOffset: [],
    scenarioEvents: [],
    dirty: true,
  };
}

export function validatePlantIdentity(input: {
  id: string;
  displayName: string;
  existingPlantIds: string[];
  originalId?: string;
}): PlantIdentityIssue[] {
  const issues: PlantIdentityIssue[] = [];
  if (!/^[A-Za-z][A-Za-z0-9_-]*$/.test(input.id)) {
    issues.push({
      field: "id",
      code: "invalid_plant_id",
      message: "Start with a letter; then use letters, numbers, _ or -.",
    });
  } else if (
    input.id !== input.originalId &&
    input.existingPlantIds.includes(input.id)
  ) {
    issues.push({
      field: "id",
      code: "duplicate_plant_id",
      message: "Choose an ID that is not already used in this project.",
    });
  }
  if (input.displayName.trim().length === 0) {
    issues.push({
      field: "displayName",
      code: "empty_display_name",
      message: "Enter a display name.",
    });
  }
  return issues;
}
