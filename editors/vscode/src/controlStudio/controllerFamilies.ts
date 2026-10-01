export const PROJECT_CONTROLLER_FAMILIES = ["P", "PI", "PD", "PID", "cascade", "feedforward", "ratio", "split-range", "override", "Smith", "paired-PI", "decoupled-PI", "gain-scheduled", "MPC", "adaptive", "robust", "feedback-linearization", "observer-feedback", "lead-lag"] as const;
/** Families with connected, typed coefficient adapters. */
export const CONTROLLER_FAMILIES = ["ON/OFF", "state-feedback", "LQR", "LQG", ...PROJECT_CONTROLLER_FAMILIES] as const;
export type ControllerFamily = typeof CONTROLLER_FAMILIES[number];
export function isControllerFamily(value: unknown): value is ControllerFamily {
  return typeof value === "string" && (CONTROLLER_FAMILIES as readonly string[]).includes(value);
}

/** How a family reads in the UI: acronyms as they are, lower-case ids as words. */
const FAMILY_LABELS: Readonly<Record<string, string>> = { "state-feedback": "State feedback", cascade: "Cascade", feedforward: "Feedforward",
  ratio: "Ratio", "split-range": "Split range", override: "Override", Smith: "Smith predictor", "paired-PI": "Paired PI",
  "decoupled-PI": "Decoupled PI", "gain-scheduled": "Gain schedule", adaptive: "Adaptive", robust: "Robust",
  "feedback-linearization": "Feedback linearization", "observer-feedback": "Observer feedback", "lead-lag": "Lead/lag" };
export function familyLabel(family: string): string { return FAMILY_LABELS[family] ?? family; }
