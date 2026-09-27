import { estimateFrequencyResponse } from "./numerics/frequencyIdentification";
import type { FrequencyResponseEstimate } from "./numerics/contracts";
import { designLagAtCrossover, designLeadAtCrossover, type TransferFunction } from "./numerics/classicalAnalysis";

export function estimatePeriodicFrequency(
  input: readonly number[],
  output: readonly number[],
  sampleTimeSeconds: number,
  frequenciesRadPerSecond: readonly number[],
  options: { segmentCount: number; minimumCoherence: number; startupSamples?: number },
): FrequencyResponseEstimate {
  const startupSamples = options.startupSamples ?? 0;
  if (!Number.isSafeInteger(startupSamples) || startupSamples < 0 || startupSamples >= input.length || input.length !== output.length) return { status: "rejected", reason: "insufficient-data" };
  const trimmedInput = input.slice(startupSamples); const trimmedOutput = output.slice(startupSamples);
  if (trimmedInput.length % options.segmentCount !== 0) return { status: "rejected", reason: "insufficient-data" };
  return estimateFrequencyResponse(trimmedInput, trimmedOutput, sampleTimeSeconds, frequenciesRadPerSecond, options);
}

export function calculateLeadLag(
  plant: TransferFunction,
  mode: "lead" | "lag",
  crossoverRadPerSecond: number,
  value: number,
) {
  return mode === "lead"
    ? designLeadAtCrossover(plant, crossoverRadPerSecond, value)
    : designLagAtCrossover(plant, crossoverRadPerSecond, value);
}
