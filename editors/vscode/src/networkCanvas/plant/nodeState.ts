import type { NCPlant, NCPlantState } from "../webview/types";

export interface PlantStatePresentation {
  primary: NCPlantState;
  secondary: NCPlantState[];
}

/** Preserves every true condition while selecting the specification's one primary label. */
export function plantStatePresentation(plant: NCPlant): PlantStatePresentation {
  if (plant.draft) return { primary: "Draft", secondary: [] };
  const states: NCPlantState[] = [];
  if (plant.statusAvailability === "available" && plant.activeOperatingState === "faulted") {
    states.push("Faulted");
  }
  if (
    plant.activeConfigurationRevision &&
    plant.savedConfigurationRevision &&
    plant.activeConfigurationRevision !== plant.savedConfigurationRevision
  ) {
    states.push("Restart required");
  }
  if (!plant.enabled) states.push("Disabled");
  if (plant.statusAvailability === "unavailable") states.push("Status unavailable");
  if (plant.statusAvailability === "available" && plant.activeOperatingState === "running") {
    states.push(plant.hasCompletedSample ? "Running" : "Waiting for samples");
  }
  if (states.length === 0) states.push("Configured");
  return { primary: states[0], secondary: states.slice(1) };
}

export function primaryPlantState(plant: NCPlant): NCPlantState {
  return plantStatePresentation(plant).primary;
}
