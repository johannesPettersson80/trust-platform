import type { EngineeringPlotMessageEvidence } from "./engineeringPlots";

export type Margin = number | "unbounded";

/** How much model error a candidate tolerates on the selected model (Specification 35 §17.10). */
export interface CandidateRobustness {
  gainMarginDb: Margin;
  phaseMarginDegrees: Margin;
  /** The extra dead time the loop tolerates before it becomes unstable: the phase margin over the gain-crossover frequency. */
  delayMarginSeconds: Margin;
  /** The largest factor by which feedback amplifies a disturbance at any frequency, max |1/(1 + L(jω))|. */
  peakSensitivity: number;
}

export function candidateRobustness(evidence: Pick<EngineeringPlotMessageEvidence, "gainMarginDb" | "phaseMarginDegrees" | "gainCrossover" | "sensitivity">
  & Partial<Pick<EngineeringPlotMessageEvidence, "peakSensitivity">>): CandidateRobustness {
  const phase = evidence.phaseMarginDegrees;
  const crossover = evidence.gainCrossover?.frequencyRadPerSecond;
  return {
    gainMarginDb: evidence.gainMarginDb,
    phaseMarginDegrees: phase,
    delayMarginSeconds: typeof phase === "number" && crossover !== undefined && crossover > 0 ? phase * Math.PI / 180 / crossover : "unbounded",
    peakSensitivity: evidence.peakSensitivity ?? Math.max(...evidence.sensitivity.map(point => 10 ** (point.sensitivityMagnitudeDb / 20))),
  };
}
