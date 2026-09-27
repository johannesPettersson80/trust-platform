import { AntiWindupCampaign, type AntiWindupCandidateReview } from "./antiWindupCampaign";
import { decodePidLaw } from "./controllerLaw";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { ParameterApplicationClient, ParameterStatus } from "./parameterApplication";
import type { SaturationWindupAssessment } from "./saturationWindup";

type AntiWindupParameterClient = Pick<ParameterApplicationClient,
  "status" | "review" | "apply" | "reviewRollback" | "rollback">;

export interface AntiWindupRuntimeConnection {
  readonly source: "simulation" | "physical";
  readonly client: AntiWindupParameterClient;
}

export type AntiWindupAvailability =
  | { readonly available: true }
  | { readonly available: false; readonly reason: string };

export type AntiWindupWorkflowResult =
  | { readonly status: "reviewed"; readonly candidate: AntiWindupCandidateReview }
  | { readonly status: "staged"; readonly observed: ParameterStatus }
  | { readonly status: "rolled-back"; readonly observed: ParameterStatus };

/**
 * Owns the one bounded tracked candidate which follows a qualified
 * saturation-return record. It deliberately stages only in simulation; Test
 * owns the later matched runtime record and comparison.
 */
export class AntiWindupWorkflow {
  private campaign: AntiWindupCampaign | undefined;
  private reviewed = false;
  private staged = false;

  constructor(private readonly loop: ControlLoopDefinition,
    private readonly connect: () => Promise<AntiWindupRuntimeConnection>) {}

  async review(trackingTimeSeconds: number, evidence: SaturationWindupAssessment): Promise<AntiWindupWorkflowResult> {
    const available = antiWindupAvailability(this.loop, evidence);
    if (!available.available) throw new Error(available.reason);
    if (this.staged) throw new Error("Restore the staged anti-windup candidate before reviewing another one.");
    const connection = await this.connect();
    if (connection.source !== "simulation") {
      throw new Error("Back-calculation comparison requires the reviewed simulation path.");
    }
    const campaign = new AntiWindupCampaign(this.loop, connection.client);
    const candidate = await campaign.review(trackingTimeSeconds);
    this.campaign = campaign;
    this.reviewed = true;
    return { status: "reviewed", candidate };
  }

  async stage(): Promise<AntiWindupWorkflowResult> {
    if (!this.campaign || !this.reviewed || this.staged) {
      throw new Error("Review one anti-windup candidate before staging it.");
    }
    const observed = await this.campaign.stage();
    this.staged = true;
    return { status: "staged", observed };
  }

  async rollback(): Promise<AntiWindupWorkflowResult> {
    if (!this.campaign || !this.staged) {
      throw new Error("No staged anti-windup candidate is available for rollback.");
    }
    const observed = await this.campaign.rollback();
    this.campaign = undefined;
    this.reviewed = false;
    this.staged = false;
    return { status: "rolled-back", observed };
  }
}

/** Tests whether this actual recorded saturation case can begin a tracked candidate. */
export function antiWindupAvailability(loop: ControlLoopDefinition, evidence: SaturationWindupAssessment): AntiWindupAvailability {
  if (evidence.status !== "qualified") {
    return { available: false, reason: "A qualified saturation-return recording is required before reviewing anti-windup." };
  }
  const law = decodePidLaw(loop.controller.law, loop.parameterBinding);
  if (!law?.antiWindup) {
    return { available: false, reason: "A typed back-calculation anti-windup adapter is required." };
  }
  return { available: true };
}
