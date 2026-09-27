export type EngineerStage = "model" | "tune" | "verify" | "apply";

export type EngineerPlotId = "fit" | "general-model" | "engineering-response" | "recorded-responses" | "response";

export interface EngineerPlotEvidence {
  readonly model?: boolean;
  readonly suppliedModel?: boolean;
  readonly candidate?: boolean;
  readonly predictedResponse?: boolean;
  readonly recordedResponses?: boolean;
  readonly samples?: boolean;
}

export interface EngineerPlotSelection {
  readonly id: EngineerPlotId;
  readonly label: string;
  readonly available: boolean;
  readonly reason: string;
}

const PLOT_BY_STAGE: Record<Exclude<EngineerStage, "model">, Omit<EngineerPlotSelection, "available" | "reason">> = {
  tune: { id: "engineering-response", label: "Predicted response" },
  verify: { id: "recorded-responses", label: "Actual recorded response" },
  apply: { id: "response", label: "Live response" },
};

/**
 * Selects exactly one plot for the permanent engineer stage tabs. The source of
 * each plot is explicit so a prediction can never stand in for actual Verify or
 * live Apply evidence.
 */
export function engineerPlotForStage(stage: EngineerStage, evidence: EngineerPlotEvidence): EngineerPlotSelection {
  const modelPlot = evidence.model !== true && evidence.suppliedModel === true
    ? { id: "general-model" as const, label: "Model frequency response" }
    : { id: "fit" as const, label: "Model fit" };
  const plot = stage === "model" ? modelPlot : PLOT_BY_STAGE[stage];
  const available = stage === "model" ? (evidence.model === true || evidence.suppliedModel === true)
    : stage === "tune" ? evidence.predictedResponse === true
      : stage === "verify" ? evidence.recordedResponses === true
        : evidence.samples === true;
  const reason = available ? "" : stage === "tune" && evidence.candidate ? "No preview for this controller. Run verification to check its actual response." : unavailableReason(stage);
  return { ...plot, available, reason };
}

function unavailableReason(stage: EngineerStage): string {
  switch (stage) {
    case "model": return "Find or supply a model to view its response.";
    case "tune": return "Generate a qualified tuning result to view its predicted response.";
    case "verify": return "Run Verify and retain actual ST samples before viewing this response.";
    case "apply": return "Start the runtime to view the live response.";
  }
}
