export type AntiWindupPresentationPhase = "hidden" | "ready" | "reviewed" | "staged";

export interface AntiWindupPresentation {
  readonly phase: AntiWindupPresentationPhase;
  readonly visible: boolean;
  readonly reviewVisible: boolean;
  readonly stageVisible: boolean;
  readonly rollbackVisible: boolean;
  readonly status: string;
}

/** Pure state transition for the Test pane's anti-windup actions. */
export function initialAntiWindupPresentation(): AntiWindupPresentation {
  return { phase: "hidden", visible: false, reviewVisible: false, stageVisible: false, rollbackVisible: false, status: "" };
}

export function reduceAntiWindupPresentation(
  current: AntiWindupPresentation,
  phase: string,
  payload: Record<string, unknown> | undefined
): AntiWindupPresentation {
  if (phase === "terminal") {
    if (!available(payload?.antiWindup)) return initialAntiWindupPresentation();
    if (current.phase === "staged") return current;
    return ready("A qualified saturation-return record is ready for one simulation-only tracking candidate.");
  }
  if (phase !== "anti-windup") return current;
  const status = String(payload?.status ?? "failed");
  if (status === "reviewed" && candidate(payload?.candidate)) {
    return {
      phase: "reviewed", visible: true, reviewVisible: true, stageVisible: true, rollbackVisible: false,
      status: `Candidate reviewed: Tt = ${format(payload!.candidate.trackingTimeSeconds)} s; back-calculation gain = ${format(payload!.candidate.backCalculationGain)} 1/s. Verify it only with the matched actual-ST simulation Test.`,
    };
  }
  if (status === "staged") {
    return {
      phase: "staged", visible: true, reviewVisible: false, stageVisible: false, rollbackVisible: true,
      status: "Tracked candidate is staged in simulation. Run the same bounded saturation-return Test, then restore the exact baseline.",
    };
  }
  if (status === "rolled-back") {
    return ready("The exact untracked baseline was restored after the candidate Test.");
  }
  if (status === "compared" && comparison(payload?.comparison)) {
    const value = payload!.comparison;
    return ready(`Matched actual-ST comparison: ${value.recommendation === "recommend_tracking" ? "recommend tracking" : "do not recommend tracking"}. ` +
      `Release delay ${format(value.baselineRecoverySeconds)} → ${format(value.candidateRecoverySeconds)} s; ` +
      `retained-integral change ${format(value.baselineIntegralChange)} → ${format(value.candidateIntegralChange)}.`);
  }
  if (status === "failed") return { ...current, status: String(payload?.reason ?? "The anti-windup action failed.") };
  return current;
}

function ready(status: string): AntiWindupPresentation {
  return { phase: "ready", visible: true, reviewVisible: true, stageVisible: false, rollbackVisible: false, status };
}

function available(value: unknown): value is { available: true } {
  return Boolean(value && typeof value === "object" && (value as Record<string, unknown>).available === true);
}

function candidate(value: unknown): value is { trackingTimeSeconds: number; backCalculationGain: number } {
  return Boolean(value && typeof value === "object" &&
    Number.isFinite((value as Record<string, unknown>).trackingTimeSeconds) &&
    Number.isFinite((value as Record<string, unknown>).backCalculationGain));
}

function comparison(value: unknown): value is {
  recommendation: "recommend_tracking" | "do_not_recommend_tracking";
  baselineRecoverySeconds: number; candidateRecoverySeconds: number;
  baselineIntegralChange: number; candidateIntegralChange: number;
} {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Record<string, unknown>;
  return (candidate.recommendation === "recommend_tracking" || candidate.recommendation === "do_not_recommend_tracking") &&
    [candidate.baselineRecoverySeconds, candidate.candidateRecoverySeconds, candidate.baselineIntegralChange,
      candidate.candidateIntegralChange].every(item => typeof item === "number" && Number.isFinite(item));
}

function format(value: number): string {
  return String(Math.round(value * 1e9) / 1e9);
}
