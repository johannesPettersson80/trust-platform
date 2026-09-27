interface SaturationWindupEvidenceDependencies {
  setText(selector: string, value: unknown): void;
}

type QualifiedEvidence = {
  status: "qualified";
  limit: "upper" | "lower";
  enteredAtSeconds: number;
  targetReturnAtSeconds: number;
  releasedAtSeconds: number;
  recoverySeconds: number;
  peakRequestedOutput: number;
  peakAppliedOutput: number;
  maximumMismatch: number;
  integralAtEntry: number;
  integralAtTargetReturn: number;
  integralAtRelease: number;
  integralChangeDuringSaturation: number;
  recommendation: string;
};

type RejectedEvidence = { status: "not_qualified"; reason: string };

/** Renders only recorded requested/applied/integral evidence; it never estimates missing values. */
export function initializeSaturationWindupEvidence(dependencies: SaturationWindupEvidenceDependencies) {
  const card = () => document.querySelector<HTMLElement>("[data-saturation-windup-evidence]");
  const unit = () => document.querySelector<HTMLElement>("[data-control-studio]")?.dataset.outputUnit || "output unit";
  const show = () => { const element = card(); if (element) element.hidden = false; };
  return {
    clear(): void {
      const element = card();
      if (element) element.hidden = true;
    },
    receive(value: unknown): void {
      if (!isEvidence(value)) return;
      show();
      if (value.status === "not_qualified") {
        dependencies.setText("[data-saturation-windup-summary]", `Saturation evidence is not qualified: ${value.reason}`);
        for (const selector of ["[data-saturation-windup-requested]", "[data-saturation-windup-applied]",
          "[data-saturation-windup-recovery]", "[data-saturation-windup-integral]"]) dependencies.setText(selector, "—");
        return;
      }
      const direction = value.limit === "upper" ? "upper" : "lower";
      dependencies.setText("[data-saturation-windup-summary]",
        `Recorded ${direction} output saturation: requested output remained separate from the applied clamp. ${value.recommendation}`);
      dependencies.setText("[data-saturation-windup-requested]", format(value.peakRequestedOutput));
      dependencies.setText("[data-saturation-windup-applied]", format(value.peakAppliedOutput));
      dependencies.setText("[data-saturation-windup-recovery]", format(value.recoverySeconds));
      dependencies.setText("[data-saturation-windup-integral]", format(value.integralChangeDuringSaturation));
      dependencies.setText("[data-saturation-windup-details]",
        `Entered ${format(value.enteredAtSeconds)} s; target returned ${format(value.targetReturnAtSeconds)} s; ` +
        `left the limit ${format(value.releasedAtSeconds)} s. Maximum request/applied mismatch ${format(value.maximumMismatch)} ${unit()}. ` +
        `Retained integral: ${format(value.integralAtEntry)} → ${format(value.integralAtTargetReturn)} → ${format(value.integralAtRelease)} ${unit()}.`);
    },
  };
}

function isEvidence(value: unknown): value is QualifiedEvidence | RejectedEvidence {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const candidate = value as Record<string, unknown>;
  if (candidate.status === "not_qualified") return typeof candidate.reason === "string";
  return candidate.status === "qualified" && [candidate.enteredAtSeconds, candidate.targetReturnAtSeconds,
    candidate.releasedAtSeconds, candidate.recoverySeconds, candidate.peakRequestedOutput, candidate.peakAppliedOutput,
    candidate.maximumMismatch, candidate.integralAtEntry, candidate.integralAtTargetReturn,
    candidate.integralAtRelease, candidate.integralChangeDuringSaturation].every(value => typeof value === "number" && Number.isFinite(value)) &&
    (candidate.limit === "upper" || candidate.limit === "lower") && typeof candidate.recommendation === "string";
}

function format(value: number): string {
  return Number.isInteger(value) ? String(value) : String(Math.round(value * 1e6) / 1e6);
}
