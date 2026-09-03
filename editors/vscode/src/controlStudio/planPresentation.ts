// Confirms a reviewed Test plan stays bound to the exact form the learner sees before approval is authorized.

export interface ExperimentFormSnapshot {
  kind: string;
  amplitude: number;
  durationSeconds: number;
}

/**
 * True only when the live form still matches the snapshot captured at the last
 * "Review test" click. Spec §7 requires "any material change requires new approval";
 * this is the check the Approve action must pass before it is allowed to fire.
 */
export function experimentFormMatchesReviewedPlan(
  current: ExperimentFormSnapshot,
  reviewed: ExperimentFormSnapshot | undefined
): boolean {
  if (!reviewed) return false;
  return current.kind === reviewed.kind &&
    numbersMatch(current.amplitude, reviewed.amplitude) &&
    numbersMatch(current.durationSeconds, reviewed.durationSeconds);
}

function numbersMatch(left: number, right: number): boolean {
  return Number.isNaN(left) && Number.isNaN(right) ? true : left === right;
}

/** The authorizing action gets a visually distinct treatment for the higher-stakes physical case. */
export function experimentApprovalButtonClass(source: "simulation" | "physical"): "primary" | "danger" {
  return source === "physical" ? "danger" : "primary";
}
