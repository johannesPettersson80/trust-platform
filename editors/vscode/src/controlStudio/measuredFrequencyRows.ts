import type { FrequencyResponsePoint } from "./numerics/contracts";

export interface MeasuredFrequencyRow { frequency: number; magnitude: number; phaseDegrees: number; coherence: number }

/** The measured tones in ascending frequency, each phase within 180° of the previous tone's (Specification 35 §8). */
export function measuredFrequencyRows(points: readonly FrequencyResponsePoint[]): MeasuredFrequencyRow[] {
  let previous: number | undefined;
  return [...points].sort((left, right) => left.frequencyRadPerSecond - right.frequencyRadPerSecond).map(point => {
    let phase = point.phaseRadians * 180 / Math.PI;
    if (previous !== undefined) {
      while (phase - previous > 180) phase -= 360;
      while (phase - previous < -180) phase += 360;
    }
    previous = phase;
    return { frequency: point.frequencyRadPerSecond, magnitude: point.magnitude, phaseDegrees: phase, coherence: point.coherence };
  });
}
