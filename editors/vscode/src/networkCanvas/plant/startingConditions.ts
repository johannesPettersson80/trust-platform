export interface PlantStartingStateField {
  readonly label: string;
  readonly description: string;
}

// Array order is the canonical runtime state index, not a physical-model conversion.
const fields: Readonly<Record<string, readonly PlantStartingStateField[]>> = {
  first_order: [
    { label: "Starting output", description: "Absolute ideal output, in the configured model's output units" },
  ],
  integrator: [
    { label: "Starting accumulated value", description: "Stored state before output bias; output equals this value plus output bias" },
  ],
  gravity_tank: [
    { label: "Starting level (m)", description: "Tank liquid level in metres" },
  ],
  gravity_tank_cascade: [
    { label: "Starting upstream level (m)", description: "Upstream tank liquid level in metres" },
    { label: "Starting downstream level (m)", description: "Downstream tank liquid level in metres" },
  ],
  second_order: [
    { label: "Starting displacement", description: "Output displacement before output bias" },
    { label: "Starting rate of change", description: "Time derivative in output units per second" },
  ],
  robot_joint: [
    { label: "Starting angle (rad)", description: "Joint angle when simulation starts or resets. Enter radians, not degrees; 2π radians is one complete turn." },
    { label: "Starting angular speed (rad/s)", description: "Joint speed when simulation starts or resets. Zero starts at rest; the sign gives the direction of motion." },
  ],
};

export function plantStartingStateFields(model: string): readonly PlantStartingStateField[] {
  return Object.prototype.hasOwnProperty.call(fields, model) ? fields[model] : [];
}
