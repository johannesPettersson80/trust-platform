import { candidateFromControlPid, type ControlPidParameters } from "./controllerCandidate";

export interface ApplicationResult { previousRevision: string; activeRevision: string }
type RuntimeRequest = (operation: string, params: Record<string, unknown>) => Promise<Record<string, unknown>>;
export class ControllerApplicationClient {
  constructor(private readonly request: RuntimeRequest) {}
  async status(instancePath: string): Promise<{ activeRevision: string; pendingRevision?: string;
    confirmed: boolean; parameters: Record<string, unknown> }> {
    const response = await this.request("control.pid.status", { instance_path: required(instancePath) });
    const parameters = response.parameters;
    if (parameters === null || typeof parameters !== "object" || Array.isArray(parameters)) throw new Error("Runtime returned invalid controller parameters.");
    return { activeRevision: required(response.active_revision),
      ...(typeof response.pending_revision === "string" ? { pendingRevision: response.pending_revision } : {}),
      confirmed: response.confirmed === true,
      parameters: parameters as Record<string, unknown> };
  }
  async apply(value: { instancePath: string; source: "Simulation" | "Physical"; expectedRevision: string;
    candidate: ControlPidParameters; physicalApproved: boolean }): Promise<ApplicationResult> {
    if (value.source === "Physical" && !value.physicalApproved) throw new Error("Physical controller application requires separate immediate approval.");
    const candidate = candidateFromControlPid(value.candidate, "Manual").parameters;
    const response = await this.request("control.pid.apply", {
      instance_path: required(value.instancePath), source: value.source.toLowerCase(), expected_revision: required(value.expectedRevision),
      parameters: {
        ProportionalGain: candidate.proportionalGain,
        IntegralGain: candidate.integralGain,
        DerivativeGain: candidate.derivativeGain,
        ProportionalSetpointWeight: candidate.proportionalSetpointWeight,
        DerivativeSetpointWeight: candidate.derivativeSetpointWeight,
        Bias: candidate.bias,
        OutputMinimum: candidate.outputMinimum,
        OutputMaximum: candidate.outputMaximum,
        RateMinimum: candidate.rateMinimum,
        RateMaximum: candidate.rateMaximum,
        TrackingTime: candidate.trackingTime,
        AntiWindup: candidate.antiWindup,
      },
      physical_approved: value.physicalApproved,
    });
    const previousRevision = required(response.previous_revision);
    const pendingRevision = required(response.pending_revision);
    await this.waitForConfirmedRevision(value.instancePath, pendingRevision);
    return { previousRevision, activeRevision: pendingRevision };
  }
  async rollback(value: { instancePath: string; expectedActiveRevision: string; previousRevision: string }): Promise<{ activeRevision: string }> {
    const response = await this.request("control.pid.rollback", {
      instance_path: required(value.instancePath), expected_active_revision: required(value.expectedActiveRevision),
      previous_revision: required(value.previousRevision),
    });
    const pendingRevision = required(response.pending_revision);
    await this.waitForConfirmedRevision(value.instancePath, pendingRevision);
    return { activeRevision: pendingRevision };
  }

  private async waitForConfirmedRevision(instancePath: string, expectedRevision: string): Promise<void> {
    for (let attempt = 0; attempt < 20; attempt += 1) {
      const status = await this.status(instancePath);
      if (status.confirmed && status.activeRevision === expectedRevision) return;
      if (status.pendingRevision && status.pendingRevision !== expectedRevision) {
        throw new Error("A different controller write replaced the reviewed candidate.");
      }
      if (!status.pendingRevision && status.activeRevision !== expectedRevision) {
        throw new Error("The controller write was not confirmed by the runtime.");
      }
      await new Promise((resolve) => setTimeout(resolve, 25));
    }
    throw new Error("Timed out waiting for scan-boundary controller confirmation.");
  }
}
function required(value: unknown): string {
  if (typeof value !== "string" || value.trim() === "") throw new Error("Runtime returned an invalid controller revision.");
  return value;
}
