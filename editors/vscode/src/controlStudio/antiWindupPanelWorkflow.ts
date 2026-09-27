import { AntiWindupWorkflow, antiWindupAvailability, type AntiWindupRuntimeConnection, type AntiWindupWorkflowResult } from "./antiWindupWorkflow";
import type { AntiWindupCandidateReview } from "./antiWindupCampaign";
import { runAntiWindupIsolatedCampaign, type AntiWindupIsolatedCampaignResult } from "./antiWindupIsolatedCampaign";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { RunRequirement } from "./runValidation";
import type { SaturationWindupAssessment } from "./saturationWindup";
import { RuntimeVerificationClient, type RuntimeVerificationRequest, type RuntimeVerificationResult } from "./verificationClient";

export type AntiWindupPanelCommand =
  | { readonly action: "review"; readonly trackingTimeSeconds: number }
  | { readonly action: "stage" | "rollback" };
export type AntiWindupPanelMessageCommand = AntiWindupPanelCommand | { readonly action: "verify" };

export interface ExecuteAntiWindupCommandInput {
  readonly command: AntiWindupPanelCommand;
  readonly loop: ControlLoopDefinition;
  readonly saturationWindup?: SaturationWindupAssessment;
  readonly workflow?: AntiWindupWorkflow;
  readonly connect: () => Promise<AntiWindupRuntimeConnection>;
}

export interface ExecuteAntiWindupCommandResult {
  readonly workflow: AntiWindupWorkflow | undefined;
  readonly result: AntiWindupWorkflowResult;
}

/** Retains only current-loop Test evidence and its one staged candidate. */
export class AntiWindupPanelSession {
  private workflow: AntiWindupWorkflow | undefined;
  private saturationWindup: SaturationWindupAssessment | undefined;
  private candidate: AntiWindupCandidateReview | undefined;

  reset(): void {
    this.workflow = undefined;
    this.saturationWindup = undefined;
    this.candidate = undefined;
  }

  augmentTerminalPayload(loop: ControlLoopDefinition | undefined, payload: unknown): unknown {
    this.workflow = undefined;
    this.candidate = undefined;
    this.saturationWindup = saturationWindupFromTestPayload(payload);
    if (!loop || !this.saturationWindup || !record(payload)) return payload;
    return { ...payload, antiWindup: antiWindupAvailability(loop, this.saturationWindup) };
  }

  async execute(command: AntiWindupPanelCommand, loop: ControlLoopDefinition,
    connect: () => Promise<AntiWindupRuntimeConnection>): Promise<AntiWindupWorkflowResult> {
    const outcome = await executeAntiWindupCommand({ command, loop, saturationWindup: this.saturationWindup,
      workflow: this.workflow, connect });
    this.workflow = outcome.workflow;
    if (outcome.result.status === "reviewed") this.candidate = outcome.result.candidate;
    if (outcome.result.status === "rolled-back") this.candidate = undefined;
    return outcome.result;
  }

  async verify(
    loop: ControlLoopDefinition,
    verify: (request: RuntimeVerificationRequest, requirements: readonly RunRequirement[]) => Promise<RuntimeVerificationResult>,
    campaign: typeof runAntiWindupIsolatedCampaign = runAntiWindupIsolatedCampaign,
  ): Promise<AntiWindupIsolatedCampaignResult> {
    if (!this.candidate) throw new Error("Review one anti-windup candidate before running the matched comparison.");
    if (!this.saturationWindup || this.saturationWindup.status !== "qualified") {
      throw new Error("A qualified saturation-return recording is required before running the matched comparison.");
    }
    return campaign({ loop, candidate: this.candidate, saturationWindup: this.saturationWindup, verify });
  }
}

/** Converts a scoped command result into one terminal-style panel payload. */
export async function dispatchAntiWindupCommand(input: {
  readonly command: AntiWindupPanelCommand;
  readonly loop: ControlLoopDefinition;
  readonly session: AntiWindupPanelSession;
  readonly connect: () => Promise<AntiWindupRuntimeConnection>;
  readonly post: (payload: AntiWindupWorkflowResult | { status: "failed"; reason: string }) => Promise<void>;
}): Promise<void> {
  try {
    await input.post(await input.session.execute(input.command, input.loop, input.connect));
  } catch (error) {
    await input.post({ status: "failed", reason: error instanceof Error ? error.message : String(error) });
  }
}

/** Runs the verified candidate in a private simulation, never writing the live controller. */
export async function dispatchAntiWindupVerification(input: {
  readonly loop: ControlLoopDefinition;
  readonly session: AntiWindupPanelSession;
  readonly connect: () => Promise<{ source: "simulation" | "physical"; request: (operation: string, params: Record<string, unknown>) => Promise<unknown> }>;
  readonly post: (payload: Record<string, unknown>) => Promise<void>;
}): Promise<void> {
  try {
    const connection = await input.connect();
    if (connection.source !== "simulation") throw new Error("Anti-windup comparison requires the local simulator.");
    const verifier = new RuntimeVerificationClient({ source: "simulation", request: connection.request });
    const result = await input.session.verify(input.loop, (request, requirements) => verifier.verify(request, requirements));
    await input.post({ status: "compared", comparison: result.comparison, baseline: result.baseline.run, candidate: result.candidate.run });
  } catch (error) {
    await input.post({ status: "failed", reason: error instanceof Error ? error.message : String(error) });
  }
}

/** Routes one versioned panel command without giving the panel campaign state. */
export async function routeAntiWindupPanelCommand(input: {
  readonly command: AntiWindupPanelMessageCommand;
  readonly loop: ControlLoopDefinition;
  readonly session: AntiWindupPanelSession;
  readonly connectParameters: () => Promise<AntiWindupRuntimeConnection>;
  readonly connectSearch: () => Promise<{ source: "simulation" | "physical"; request: (operation: string, params: Record<string, unknown>) => Promise<unknown> }>;
  readonly post: (payload: Record<string, unknown>) => Promise<void>;
}): Promise<void> {
  if (input.command.action === "verify") {
    return dispatchAntiWindupVerification({ loop: input.loop, session: input.session, connect: input.connectSearch, post: input.post });
  }
  return dispatchAntiWindupCommand({ command: input.command, loop: input.loop, session: input.session, connect: input.connectParameters,
    post: async payload => input.post(payload as unknown as Record<string, unknown>) });
}

/** Binds scoped Test evidence to one staged parameter workflow without UI state. */
export async function executeAntiWindupCommand(input: ExecuteAntiWindupCommandInput): Promise<ExecuteAntiWindupCommandResult> {
  if (input.command.action === "review") {
    if (!input.saturationWindup) throw new Error("A qualified saturation-return recording is required before reviewing anti-windup.");
    const workflow = input.workflow ?? new AntiWindupWorkflow(input.loop, input.connect);
    return { workflow, result: await workflow.review(input.command.trackingTimeSeconds, input.saturationWindup) };
  }
  if (!input.workflow) throw new Error(input.command.action === "stage"
    ? "Review one anti-windup candidate before staging it."
    : "No staged anti-windup candidate is available for rollback.");
  if (input.command.action === "stage") {
    return { workflow: input.workflow, result: await input.workflow.stage() };
  }
  return { workflow: undefined, result: await input.workflow.rollback() };
}

/** Strictly projects only a complete Test saturation assessment into panel scope. */
export function saturationWindupFromTestPayload(value: unknown): SaturationWindupAssessment | undefined {
  if (!record(value) || !record(value.saturationWindup)) return undefined;
  const assessment = value.saturationWindup;
  if (assessment.status === "not_qualified" && typeof assessment.reason === "string") {
    return { status: "not_qualified", reason: assessment.reason };
  }
  if (assessment.status !== "qualified" || (assessment.limit !== "upper" && assessment.limit !== "lower") ||
      typeof assessment.recommendation !== "string") return undefined;
  const numeric = [assessment.enteredAtSeconds, assessment.targetReturnAtSeconds, assessment.releasedAtSeconds,
    assessment.recoverySeconds, assessment.peakRequestedOutput, assessment.peakAppliedOutput, assessment.maximumMismatch,
    assessment.integralAtEntry, assessment.integralAtTargetReturn, assessment.integralAtRelease,
    assessment.integralChangeDuringSaturation];
  if (!numeric.every(value => typeof value === "number" && Number.isFinite(value))) return undefined;
  return assessment as SaturationWindupAssessment;
}

function record(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}
