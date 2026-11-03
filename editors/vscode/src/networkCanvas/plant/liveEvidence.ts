import type { NCPlant, NCPlantLiveSample } from "../webview/types";

/**
 * The live sample of a plant, copied for the canvas. Rust's `plant.topology` gives one only from
 * current runtime evidence (specification 47 §20.6).
 */
export function qualifiedPlantLiveSample(plant: NCPlant): NCPlantLiveSample | undefined {
  if (!plant.liveSample) return undefined;
  return {
    command: [...plant.liveSample.command],
    feedback: [...plant.liveSample.feedback],
  };
}
