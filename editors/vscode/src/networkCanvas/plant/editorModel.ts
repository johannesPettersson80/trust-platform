import type { PlantUncertaintyDraft } from "./uncertaintyEditor";
import type { PlantScenarioEventDraft } from "./scenarioEventEditor";

/**
 * The plant setup editor's draft as the webview holds it (specification 47 §20.1). Every rule about
 * it lives in Rust: `plant.edit` completes, changes and checks the draft and answers the candidate
 * Save sends. The webview writes the values the user enters and shows Rust's answer.
 */

export type PlantStartingPoint =
  | "tank"
  | "gravity_drained_tank"
  | "gravity_tank_cascade"
  | "temperature"
  | "temperature_example"
  | "motor_speed"
  | "mechanical_position"
  | "robot_joint"
  | "static_scaling"
  | "nonlinear_characteristic"
  | "custom_state_space";

export type PlantModelKind =
  | "integrator"
  | "gravity_tank"
  | "gravity_tank_cascade"
  | "first_order_dead_time"
  | "first_order"
  | "second_order"
  | "static_gain"
  | "piecewise_linear"
  | "robot_joint"
  | "state_space";

export type PlantParameterValue = number | number[] | number[][];
export type PlantParameters = Record<string, PlantParameterValue>;
export type PlantActuatorSettings = Partial<Record<
  | "minimum"
  | "maximum"
  | "rate_up_per_s"
  | "rate_down_per_s"
  | "deadband"
  | "stiction_band"
  | "slip_jump"
  | "characteristic_exponent"
  | "lag_s"
  | "initial_applied",
  number
>>;

export interface PlantEditorDraft {
  /** Saved omission; saving keeps it while the value stays the default. */
  enabledDefaulted?: boolean;
  /** Saved omission; saving keeps it while the state stays all zeros. */
  initialStateDefaulted?: boolean;
  /** The saved plant as it was read; Save carries the fields the editor does not manage. */
  preservedConfiguration?: Record<string, unknown>;
  /** The saved plant's ID, so that the checks do not count it as another plant. */
  originalPlantId?: string;
  plantId: string;
  displayName: string;
  description: string;
  runtimeId: string;
  runtimeName: string;
  enabled: boolean;
  /** Empty until Rust opens the draft. */
  startingPoint: PlantStartingPoint | "";
  /** Empty until Rust opens a new draft. */
  model: PlantModelKind | "";
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
