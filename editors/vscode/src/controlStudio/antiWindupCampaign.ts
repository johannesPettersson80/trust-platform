import { decodePidLaw, materializePidGainRecord } from "./controllerLaw";
import type { ControlLoopDefinition } from "./loopDefinition";
import { sameParameterValues, type ParameterApplicationClient, type ParameterReview, type ParameterStatus } from "./parameterApplication";

export interface AntiWindupCandidateReview {
  readonly trackingTimeSeconds: number;
  readonly backCalculationGain: number;
  readonly values: Readonly<Record<string, number>>;
  readonly review: ParameterReview;
}

/** Owns the unapplied tracked candidate; execution and comparison remain separate stages. */
export class AntiWindupCampaign {
  private candidate: AntiWindupCandidateReview | undefined;
  private staged = false;
  constructor(private readonly loop: ControlLoopDefinition,
    private readonly parameters: Pick<ParameterApplicationClient, "status" | "review" | "apply" | "reviewRollback" | "rollback">) {}

  async review(trackingTimeSeconds: number): Promise<AntiWindupCandidateReview> {
    if (!Number.isFinite(trackingTimeSeconds) || trackingTimeSeconds <= 0) {
      throw new Error("Enter a finite positive tracking time before reviewing anti-windup.");
    }
    const law = decodePidLaw(this.loop.controller.law, this.loop.parameterBinding);
    if (!law?.antiWindup) throw new Error("A typed back-calculation anti-windup adapter is required.");
    const observed = await this.parameters.status();
    this.assertUntrackedBaseline(law.antiWindup.trackingGainSymbol, observed);
    const backCalculationGain = 1 / trackingTimeSeconds;
    const values = materializePidGainRecord(this.loop, {
      proportionalGain: observed.values[law.proportionalGainSymbol],
      integralGain: observed.values[law.integralGainSymbol],
      derivativeGain: observed.values[law.derivativeGainSymbol],
      backCalculationGain,
    }, observed.values);
    const review = await this.parameters.review(values);
    const candidate = Object.freeze({ trackingTimeSeconds, backCalculationGain, values: Object.freeze({ ...values }), review });
    this.candidate = candidate;
    return candidate;
  }

  /** Staging is an explicit simulator Test preparation, never final controller adoption. */
  async stage(): Promise<ParameterStatus> {
    if (!this.candidate || this.staged) throw new Error("Review one anti-windup candidate before staging it.");
    const observed = await this.parameters.apply(this.candidate.review, false);
    if (!sameParameterValues(observed.values, this.candidate.values)) {
      throw new Error("The staged anti-windup record does not match the reviewed candidate.");
    }
    this.staged = true;
    return observed;
  }

  /** Restores the exact untracked record after the candidate Test ends. */
  async rollback(): Promise<ParameterStatus> {
    if (!this.candidate || !this.staged) throw new Error("No staged anti-windup candidate is available for rollback.");
    const rollback = await this.parameters.reviewRollback();
    if (!sameParameterValues(rollback.values, this.candidate.review.observed.values)) {
      throw new Error("The runtime rollback record no longer matches the reviewed untracked baseline.");
    }
    const observed = await this.parameters.rollback(rollback, false);
    if (!sameParameterValues(observed.values, this.candidate.review.observed.values)) {
      throw new Error("The runtime did not restore the reviewed untracked baseline.");
    }
    this.staged = false;
    return observed;
  }

  private assertUntrackedBaseline(symbol: string, observed: ParameterStatus): void {
    const gain = observed.values[symbol];
    if (!Number.isFinite(gain)) throw new Error("The declared back-calculation gain is unavailable in the current parameter record.");
    if (gain !== 0) throw new Error("A matched tracked comparison requires an untracked baseline with back-calculation gain zero.");
  }
}
