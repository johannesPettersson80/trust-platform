/**
 * The plant models' product names, which the canvas shows on a plant node and searches. The plant
 * editor takes every model fact (names, purposes, shapes) from Rust's editor tables instead.
 */
export interface PlantModelDescriptor {
  kind: string;
  productName: string;
}

const PRODUCT_NAMES: Readonly<Record<string, string>> = {
  static_gain: "Static gain",
  first_order: "First order",
  first_order_dead_time: "First order with dead time",
  integrator: "Integrator",
  gravity_tank: "Gravity-drained tank",
  gravity_tank_cascade: "Cascaded gravity tanks",
  robot_joint: "Gravity-loaded robot joint",
  second_order: "Second order",
  piecewise_linear: "Piecewise linear",
  state_space: "State space",
};

export function plantModelDescriptor(kind: string): PlantModelDescriptor {
  return { kind, productName: PRODUCT_NAMES[kind] ?? kind.replace(/_/g, " ") };
}
