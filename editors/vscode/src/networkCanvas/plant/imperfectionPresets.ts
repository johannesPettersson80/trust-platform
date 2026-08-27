import type { PlantUncertaintyDraft } from "./uncertaintyEditor";

export type PlantImperfectionType =
  | "bias"
  | "drift"
  | "gaussian_noise"
  | "quantization"
  | "sample_and_hold"
  | "dropout"
  | "stuck_sensor"
  | "outlier"
  | "custom";

export const PLANT_IMPERFECTION_TYPES: ReadonlyArray<{
  id: PlantImperfectionType;
  label: string;
  expectation: string;
}> = [
  { id: "bias", label: "Bias", expectation: "The measured value stays offset from the ideal value." },
  { id: "drift", label: "Drift", expectation: "The measured value wanders slowly over time." },
  { id: "gaussian_noise", label: "Gaussian noise", expectation: "Fast random variation appears around the ideal value." },
  { id: "quantization", label: "Quantization", expectation: "The measured value moves in discrete steps." },
  { id: "sample_and_hold", label: "Sample and hold", expectation: "The measured value pauses for the configured interval." },
  { id: "dropout", label: "Dropout", expectation: "Measurements intermittently disappear or use the selected fallback." },
  { id: "stuck_sensor", label: "Stuck sensor", expectation: "The measured value freezes when the effect becomes active." },
  { id: "outlier", label: "Outlier", expectation: "A short measurement spike challenges filtering and diagnostics." },
  { id: "custom", label: "Custom runtime effect", expectation: "Use the exact runtime target, mode, and generator controls." },
];

export function applyPlantImperfectionType(
  type: PlantImperfectionType,
  current: Pick<PlantUncertaintyDraft, "id" | "enabled">
): PlantUncertaintyDraft {
  const common = { id: current.id, enabled: current.enabled, target: "measurement" as const, channel: 0, start_s: 0 };
  switch (type) {
    case "bias": return { ...common, mode: "add", generator: "constant", value: 0 };
    case "drift": return { ...common, mode: "add", generator: "random_walk", diffusion_per_sqrt_s: 0.01 };
    case "gaussian_noise": return { ...common, mode: "add", generator: "normal_approx_white", mean: 0, standard_deviation: 0.01 };
    case "quantization": return { ...common, mode: "replace", generator: "quantise", quantum: 0.01 };
    case "sample_and_hold": return { ...common, mode: "hold", generator: "constant", end_s: 1 };
    case "dropout": return { ...common, mode: "replace", generator: "dropout", rate_per_s: 1, duration_s: 1, dropout_policy: "hold_last" };
    case "stuck_sensor": return { ...common, mode: "hold", generator: "constant" };
    case "outlier": return { ...common, mode: "add", generator: "pulse", value: 1, duration_s: 0.1 };
    case "custom": return { ...common, mode: "add", generator: "constant", value: 0 };
  }
}

export function plantImperfectionTypeForRow(row: PlantUncertaintyDraft): PlantImperfectionType {
  if (row.target !== "measurement") return "custom";
  if (row.mode === "hold") return row.end_s === undefined ? "stuck_sensor" : "sample_and_hold";
  if (row.generator === "constant" && row.mode === "add") return "bias";
  if (row.generator === "random_walk" && row.mode === "add") return "drift";
  if (row.generator === "normal_approx_white" && row.mode === "add") return "gaussian_noise";
  if (row.generator === "quantise" && row.mode === "replace") return "quantization";
  if (row.generator === "dropout" && row.mode === "replace") return "dropout";
  if (row.generator === "pulse" && row.mode === "add") return "outlier";
  return "custom";
}

export function plantImperfectionExpectation(type: PlantImperfectionType): string {
  return PLANT_IMPERFECTION_TYPES.find((entry) => entry.id === type)?.expectation ?? "";
}
