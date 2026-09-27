import type { LoopMessageScope } from "./messages";
import { decodeGeneralModel, type GeneralModel } from "./generalModel";
import { linearTransferModel } from "./linearTransferModel";
import { calculateLeadLag } from "./threePageAnalysisOperations";

export interface SavedLeadLagAnalysis {
  model: GeneralModel;
  mode: "lead" | "lag";
  crossoverRadPerSecond: number;
  value: number;
  result: {
    status: "qualified";
    gain: number;
    zeroRadPerSecond: number;
    poleRadPerSecond: number;
    phaseContributionDegrees: number;
    openLoopMagnitudeAtCrossover: number;
    lowFrequencyGainRatio?: number;
  };
}

export function isSavedLeadLagAnalysis(value: unknown): value is SavedLeadLagAnalysis {
  if (!record(value) || !record(value.model) || !decodeGeneralModel(value.model)
    || (value.mode !== "lead" && value.mode !== "lag")
    || !finite(value.crossoverRadPerSecond) || (value.crossoverRadPerSecond as number) <= 0
    || !finite(value.value) || !record(value.result) || value.result.status !== "qualified") return false;
  const result = value.result as Record<string, unknown>;
  return [result.gain, result.zeroRadPerSecond, result.poleRadPerSecond,
    result.phaseContributionDegrees, result.openLoopMagnitudeAtCrossover].every(finite)
    && (result.lowFrequencyGainRatio === undefined || finite(result.lowFrequencyGainRatio));
}

/** Owns lead/lag design state so the browser only supplies bounded inputs. */
export class LeadLagSession {
  private saved: SavedLeadLagAnalysis | undefined;

  constructor(private readonly selectedModel: () => GeneralModel | undefined) {}

  async handle(message: unknown, scope: LoopMessageScope, post: (payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    if (!record(message) || message.schemaVersion !== 1 || message.projectId !== scope.projectId
      || message.editorSessionId !== scope.editorSessionId || message.loopId !== scope.loopId
      || message.expectedLoopRevision !== scope.loopRevision || typeof message.messageId !== "string"
      || message.kind !== "control-studio.lead-lag.run" || !record(message.payload)) return false;
    try {
      const payload = message.payload;
      const mode = payload.mode === "lag" ? "lag" : payload.mode === "lead" ? "lead" : undefined;
      const crossover = payload.crossoverRadPerSecond;
      const value = payload.value;
      const model = this.selectedModel();
      if (!mode || !finite(crossover) || (crossover as number) <= 0 || !finite(value) || !model)
        throw new Error("Lead/lag design requires a selected model and bounded settings.");
      const transfer = linearTransferModel(model);
      const result = calculateLeadLag(transfer, mode, crossover as number, value as number);
      if (result.status !== "qualified") throw new Error(`Lead/lag design was rejected: ${result.reason}.`);
      const saved: SavedLeadLagAnalysis = { model, mode, crossoverRadPerSecond: crossover as number, value: value as number, result };
      if (!isSavedLeadLagAnalysis(saved)) throw new Error("Lead/lag design did not satisfy the saved analysis contract.");
      this.saved = saved;
      await post({ status: "complete", saved });
    } catch (error) {
      await post({ status: "failed", reason: error instanceof Error ? error.message : String(error) });
    }
    return true;
  }

  snapshot(): SavedLeadLagAnalysis | undefined {
    if (!this.saved) return undefined;
    const selected = this.selectedModel();
    if (!selected || JSON.stringify(selected) !== JSON.stringify(this.saved.model)) return undefined;
    return JSON.parse(JSON.stringify(this.saved));
  }

  restore(value: SavedLeadLagAnalysis | undefined): void {
    if (value !== undefined && !isSavedLeadLagAnalysis(value)) throw new Error("Saved lead/lag analysis is invalid.");
    const selected = this.selectedModel();
    if (value && selected && JSON.stringify(selected) !== JSON.stringify(value.model)) throw new Error("Saved lead/lag analysis belongs to another selected model.");
    this.saved = value ? JSON.parse(JSON.stringify(value)) : undefined;
  }

  dispose(): void { this.saved = undefined; }
}

function record(value: unknown): value is Record<string, unknown> { return Boolean(value) && typeof value === "object" && !Array.isArray(value); }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
