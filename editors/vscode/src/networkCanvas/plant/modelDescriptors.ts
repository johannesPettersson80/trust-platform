export interface PlantModelDescriptor {
  kind: string;
  productName: string;
  purpose: string;
  states: number;
  inputs: number;
  outputs: number;
}

// Product wording belongs to one descriptor owner; the runtime contract keeps
// stable model identities and does not carry UI labels.
const DESCRIPTORS: Readonly<Record<string, PlantModelDescriptor>> = {
  static_gain: {
    kind: "static_gain", productName: "Static gain", purpose: "Instant scaling and bias",
    states: 0, inputs: 1, outputs: 1,
  },
  first_order: {
    kind: "first_order", productName: "First order", purpose: "One dominant lag",
    states: 1, inputs: 1, outputs: 1,
  },
  first_order_dead_time: {
    kind: "first_order_dead_time",
    productName: "First order with dead time",
    purpose: "One dominant lag and transport delay",
    states: 1,
    inputs: 1,
    outputs: 1,
  },
  integrator: {
    kind: "integrator", productName: "Integrator", purpose: "Accumulation without self-balancing",
    states: 1, inputs: 1, outputs: 1,
  },
  gravity_tank: {
    kind: "gravity_tank",
    productName: "Gravity-drained tank",
    purpose: "Nonlinear level from inlet and square-root outlet flow",
    states: 1,
    inputs: 1,
    outputs: 1,
  },
  robot_joint: {
    kind: "robot_joint",
    productName: "Gravity-loaded robot joint",
    purpose: "Nonlinear angle, speed, gravity, and friction dynamics",
    states: 2,
    inputs: 1,
    outputs: 2,
  },
  second_order: {
    kind: "second_order", productName: "Second order", purpose: "Oscillation or resonance with damping",
    states: 2, inputs: 1, outputs: 1,
  },
  piecewise_linear: {
    kind: "piecewise_linear",
    productName: "Piecewise linear",
    purpose: "Static nonlinear input/output curve",
    states: 0,
    inputs: 1,
    outputs: 1,
  },
  state_space: {
    kind: "state_space", productName: "State space", purpose: "Coupled linear states and channels",
    states: 1, inputs: 1, outputs: 1,
  },
};

export function plantModelDescriptor(kind: string): PlantModelDescriptor {
  return DESCRIPTORS[kind] ?? {
    kind,
    productName: kind.replace(/_/g, " "),
    purpose: "Custom plant model",
    states: 0,
    inputs: 0,
    outputs: 0,
  };
}
