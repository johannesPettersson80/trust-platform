import { decodeGeneralModel, type GeneralModel } from "./generalModel";

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

/** Keeps the lead/lag design saved with a parked loop, for the selected model it was made from. The engine calculates
 * lead/lag designs (`analysis/lead_lag_tool.rs`), so this owner calculates nothing. */
export class LeadLagSession {
  private saved: SavedLeadLagAnalysis | undefined;

  constructor(private readonly selectedModel: () => GeneralModel | undefined) {}

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
