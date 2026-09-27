export const PROJECT_CONTROLLER_FAMILIES = ["P", "PI", "PD", "PID", "cascade", "feedforward", "ratio", "split-range", "override", "Smith", "paired-PI", "decoupled-PI", "gain-scheduled", "MPC", "adaptive", "robust", "feedback-linearization", "observer-feedback", "lead-lag"] as const;
/** Families with connected, typed coefficient adapters. */
export const CONTROLLER_FAMILIES = ["ON/OFF", "state-feedback", "LQR", "LQG", ...PROJECT_CONTROLLER_FAMILIES] as const;
export type ControllerFamily = typeof CONTROLLER_FAMILIES[number];
export function isControllerFamily(value: unknown): value is ControllerFamily {
  return typeof value === "string" && (CONTROLLER_FAMILIES as readonly string[]).includes(value);
}
