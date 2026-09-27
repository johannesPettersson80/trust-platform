import type { PlantActuatorSettings, PlantEditorDraft, PlantParameters } from "./editorModel";

export type TemperatureExampleField = "resting_temperature" | "full_power_rise" | "time_constant_s" | "dead_time_s";

/** Explicit teaching seed, not identification of the user's physical equipment. */
export function proposeTemperatureExample(): {
  model: "first_order_dead_time";
  parameters: PlantParameters;
  initialState: number[];
  actuator: PlantActuatorSettings;
} {
  return {
    model: "first_order_dead_time",
    parameters: {
      gain: 1, time_constant_s: 30, dead_time_s: 2,
      input_bias: 0, output_bias: 20, initial_delayed_input: 0,
    },
    initialState: [20],
    actuator: { minimum: 0, maximum: 100, initial_applied: 0 },
  };
}

/** Called only after the user confirms the explicitly listed replacements. */
export function applyTemperatureExample<T extends PlantEditorDraft>(draft: T): T {
  return {
    ...draft,
    ...proposeTemperatureExample(),
    startingPoint: "temperature_example",
    modelValuesTouched: true,
    dirty: true,
    initialInput: draft.commandSources.map(() => 0),
    inputScale: draft.commandSources.map(() => 1),
    inputOffset: draft.commandSources.map(() => 0),
    outputScale: draft.feedbackTargets.map(() => 1),
    outputOffset: draft.feedbackTargets.map(() => 0),
  };
}

export function temperatureExampleValues(draft: PlantEditorDraft): Record<TemperatureExampleField, number> {
  const gain = Number(draft.parameters.gain);
  return {
    resting_temperature: Number(draft.parameters.output_bias ?? 0) - gain * Number(draft.parameters.input_bias ?? 0),
    full_power_rise: gain * 100,
    time_constant_s: Number(draft.parameters.time_constant_s),
    dead_time_s: Number(draft.parameters.dead_time_s),
  };
}

/** Physical wording is a reversible view of the canonical model parameters. */
export function updateTemperatureExampleField<T extends PlantEditorDraft>(draft: T, field: TemperatureExampleField, value: number): T {
  const parameters = { ...draft.parameters };
  const inputBias = Number(parameters.input_bias ?? 0);
  if (field === "resting_temperature") {
    parameters.output_bias = value + Number(parameters.gain) * inputBias;
  } else if (field === "full_power_rise") {
    const resting = temperatureExampleValues(draft).resting_temperature;
    parameters.gain = value / 100;
    parameters.output_bias = resting + parameters.gain * inputBias;
  } else {
    parameters[field] = value;
  }
  return { ...draft, parameters, modelValuesTouched: true, dirty: true };
}
