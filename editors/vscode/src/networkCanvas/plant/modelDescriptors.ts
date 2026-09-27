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
    kind: "static_gain", productName: "Static gain", purpose: "The output changes immediately in proportion to the input.",
    states: 0, inputs: 1, outputs: 1,
  },
  first_order: {
    kind: "first_order", productName: "First order", purpose: "The output moves gradually toward each new input.",
    states: 1, inputs: 1, outputs: 1,
  },
  first_order_dead_time: {
    kind: "first_order_dead_time",
    productName: "First order with dead time",
    purpose: "The output starts changing after a delay, then settles gradually.",
    states: 1,
    inputs: 1,
    outputs: 1,
  },
  integrator: {
    kind: "integrator", productName: "Integrator", purpose: "The output keeps accumulating while the input differs from its resting point; it does not settle by itself.",
    states: 1, inputs: 1, outputs: 1,
  },
  gravity_tank: {
    kind: "gravity_tank",
    productName: "Gravity-drained tank",
    purpose: "Inflow raises the level; gravity drains it faster as the level rises.",
    states: 1,
    inputs: 1,
    outputs: 1,
  },
  gravity_tank_cascade: {
    kind: "gravity_tank_cascade", productName: "Cascaded gravity tanks",
    purpose: "The upstream tank freely drains into an equal downstream tank; coupled balances preserve the transferred water.",
    states: 2, inputs: 1, outputs: 2,
  },
  robot_joint: {
    kind: "robot_joint",
    productName: "Gravity-loaded robot joint",
    purpose: "Torque changes joint angle and speed; gravity depends on angle, and friction resists motion.",
    states: 2,
    inputs: 1,
    outputs: 2,
  },
  second_order: {
    kind: "second_order", productName: "Second order", purpose: "The output can overshoot or oscillate before it settles.",
    states: 2, inputs: 1, outputs: 1,
  },
  piecewise_linear: {
    kind: "piecewise_linear",
    productName: "Piecewise linear",
    purpose: "A table of points defines an immediate nonlinear input-to-output curve.",
    states: 0,
    inputs: 1,
    outputs: 1,
  },
  state_space: {
    kind: "state_space", productName: "State space", purpose: "Coupled internal states turn one or more inputs into one or more outputs.",
    states: 1, inputs: 1, outputs: 1,
  },
};

const MODEL_DESCRIPTOR_ORDER = [
  "static_gain", "first_order", "first_order_dead_time", "integrator",
  "second_order", "piecewise_linear", "state_space", "gravity_tank", "gravity_tank_cascade", "robot_joint",
] as const;

/** The single product-order model list used by the Plant model selector. */
export function plantModelDescriptors(): PlantModelDescriptor[] {
  return MODEL_DESCRIPTOR_ORDER.map((kind) => ({ ...DESCRIPTORS[kind] }));
}

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
