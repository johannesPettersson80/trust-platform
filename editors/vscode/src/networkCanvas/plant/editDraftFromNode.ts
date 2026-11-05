import type { Node } from "@xyflow/react";

import { createPlantEditDraft, type PlantDraft } from "./addFlow";
import type { PlantModelKind } from "./editorModel";
import type { PlantScenarioEventDraft } from "./scenarioEventEditor";
import type { PlantUncertaintyDraft } from "./uncertaintyEditor";

/** The saved configuration a plant node carries from Rust's `plant.topology` (specification 47 §20.6). */
interface SavedPlantConfiguration {
  preservedConfiguration?: Record<string, unknown>;
  description?: string;
  parameters?: Record<string, number | number[] | number[][]>;
  initialState?: number[];
  enabledDefaulted?: boolean;
  initialStateDefaulted?: boolean;
  stepMs?: number;
  initialInput?: number[];
  inputScale?: number[];
  inputOffset?: number[];
  outputScale?: number[];
  outputOffset?: number[];
  actuator?: Record<string, number>;
  uncertainties?: Array<Record<string, unknown>>;
  scenarioEvents?: Array<Record<string, unknown>>;
}

/**
 * The setup editor's draft of a saved plant's node, which the inspector's Configure plant opens
 * (specification 47 §6.1); undefined for any other node, and for a plant whose saved configuration
 * the editor cannot open.
 */
export function plantEditDraftForNode(node: Node, runtimeName: string): PlantDraft | undefined {
  const data = node.data as Record<string, unknown>;
  const configuration = data.configuration as SavedPlantConfiguration | undefined;
  if (
    data.kind !== "plant" ||
    data.draft === true ||
    !node.parentId ||
    typeof data.plantId !== "string" ||
    typeof data.displayName !== "string" ||
    typeof data.modelKind !== "string" ||
    !configuration?.parameters ||
    !Array.isArray(configuration.initialState)
  ) {
    return undefined;
  }
  return createPlantEditDraft({
    ...configuration,
    parameters: configuration.parameters,
    initialState: configuration.initialState,
    id: node.id,
    projectId: String(data.projectId ?? ""),
    runtimeId: node.parentId,
    runtimeName,
    plantId: data.plantId,
    displayName: data.displayName,
    description: configuration.description ?? "",
    enabled: data.enabled !== false,
    model: data.modelKind as PlantModelKind,
    commandSources: Array.isArray(data.commandSources) ? (data.commandSources as string[]) : [],
    feedbackTargets: Array.isArray(data.feedbackTargets) ? (data.feedbackTargets as string[]) : [],
    savedConfigurationRevision: String(data.savedConfigurationRevision ?? ""),
    actuator: configuration.actuator,
    uncertainties: configuration.uncertainties as PlantUncertaintyDraft[] | undefined,
    scenarioEvents: configuration.scenarioEvents as PlantScenarioEventDraft[] | undefined,
  });
}

/**
 * The typed Control Studio handoff of a saved plant (specification 47 §14.1), which both the
 * inspector and the setup editor send; undefined for a plant that is not saved yet.
 */
export function controlStudioHandoff(draft: PlantDraft): Record<string, unknown> | undefined {
  if (!draft.persisted || !draft.projectId || !draft.savedConfigurationRevision) return undefined;
  return {
    type: "openControlStudio",
    projectId: draft.projectId,
    runtimeId: draft.runtimeId,
    plantId: draft.plantId,
    savedConfigurationRevision: draft.savedConfigurationRevision,
    ...(draft.activeConfigurationDigest
      ? { activeConfigurationDigest: draft.activeConfigurationDigest }
      : {}),
  };
}
