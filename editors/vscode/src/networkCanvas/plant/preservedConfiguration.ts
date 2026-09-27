/** Fields owned by the editable draft must never be restored from saved extras. */
const EDITABLE_FIELDS = new Set([
  "id", "name", "description", "enabled", "model", "command_sources", "feedback_targets",
  "parameters", "initial_state", "step_ms", "initial_input", "input_scale", "input_offset",
  "output_scale", "output_offset", "actuator", "uncertainties", "scenario_events",
]);

/** Carry unedited runtime configuration losslessly, without sharing mutable references. */
export function preservedPlantConfiguration(
  saved: Record<string, unknown> = {}
): Record<string, unknown> {
  return structuredClone(Object.fromEntries(
    Object.entries(saved).filter(([key]) => !EDITABLE_FIELDS.has(key))
  ));
}
