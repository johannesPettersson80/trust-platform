import type { PlantModelDescriptor } from "./modelDescriptors";
import type { NCPlant, NCPlantState } from "../webview/types";

/** Matches the identities an engineer can see or use to locate a configured plant. */
export function plantMatchesSearch(
  plant: NCPlant,
  descriptor: PlantModelDescriptor,
  state: NCPlantState,
  query: string
): boolean {
  const needle = query.trim().toLocaleLowerCase();
  if (!needle) return true;
  return [
    plant.id,
    plant.displayName,
    descriptor.kind,
    descriptor.productName,
    plant.startingPointName ?? "",
    ...(plant.mappedSymbols ?? []),
    ...plant.commandSources,
    ...plant.feedbackTargets,
    state,
  ].some((value) => value.toLocaleLowerCase().includes(needle));
}
