import type { NCPlant, NCPlantLiveSample } from "../webview/types";

/** Prevents cached or unqualified values from becoming canvas live evidence. */
export function qualifiedPlantLiveSample(plant: NCPlant): NCPlantLiveSample | undefined {
  if (
    plant.statusAvailability !== "available" ||
    plant.hasCompletedSample !== true ||
    !plant.liveSample
  ) {
    return undefined;
  }
  return {
    command: [...plant.liveSample.command],
    feedback: [...plant.liveSample.feedback],
  };
}
