import type { NCPlant, NCPlantState } from "../webview/types";

export interface PlantStatePresentation {
  primary: NCPlantState;
  secondary: NCPlantState[];
}

/**
 * A plant's states as Rust answers them (`plant.topology`, specification 47 §5.2): the first is the
 * node's state and the others are the inspector's details. The add flow's unsaved plant is a draft.
 */
export function plantStatePresentation(plant: NCPlant): PlantStatePresentation {
  if (plant.draft) return { primary: "Draft", secondary: [] };
  const [primary = "Configured", ...secondary] = plant.states ?? [];
  return { primary, secondary };
}

export function primaryPlantState(plant: NCPlant): NCPlantState {
  return plantStatePresentation(plant).primary;
}
