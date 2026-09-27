import type { NCGraph, NCPlant, NCHost, NCRuntime } from "../webview/types";
import {
  createPlantEditorDraft,
  startingPointForPlantModel,
  type PlantModelKind,
  type PlantParameters,
  type PlantActuatorSettings,
  type PlantEditorDraft,
} from "./editorModel";
import type { PlantUncertaintyDraft } from "./uncertaintyEditor";
import type { PlantScenarioEventDraft } from "./scenarioEventEditor";
import { preservedPlantConfiguration } from "./preservedConfiguration";

export const PLANT_DRAG_MIME = "application/vnd.trust.simulated-plant+json";

export interface PlantDraft extends PlantEditorDraft {
  id: string;
  persisted: boolean;
  focusSection: "plant";
  projectId?: string;
  savedConfigurationRevision?: string;
  activeConfigurationDigest?: string;
}

export function createPlantEditDraft(input: {
  preservedConfiguration?: Record<string, unknown>;
  id: string;
  projectId: string;
  runtimeId: string;
  runtimeName: string;
  plantId: string;
  displayName: string;
  description: string;
  enabled: boolean;
  enabledDefaulted?: boolean;
  model: PlantModelKind;
  parameters: PlantParameters;
  initialState: number[];
  initialStateDefaulted?: boolean;
  commandSources: string[];
  feedbackTargets: string[];
  stepMs?: number;
  projectStepMs?: number;
  initialInput?: number[];
  inputScale?: number[];
  inputOffset?: number[];
  outputScale?: number[];
  outputOffset?: number[];
  actuator?: PlantActuatorSettings;
  uncertainties?: PlantUncertaintyDraft[];
  scenarioEvents?: PlantScenarioEventDraft[];
  savedConfigurationRevision: string;
  activeConfigurationDigest?: string;
}): PlantDraft {
  return {
    ...(input.preservedConfiguration ? {
      preservedConfiguration: preservedPlantConfiguration(input.preservedConfiguration),
    } : {}),
    id: input.id,
    projectId: input.projectId,
    plantId: input.plantId,
    displayName: input.displayName,
    description: input.description,
    runtimeId: input.runtimeId,
    runtimeName: input.runtimeName,
    enabled: input.enabled,
    ...(input.enabledDefaulted ? { enabledDefaulted: true } : {}),
    startingPoint: startingPointForPlantModel(input.model),
    model: input.model,
    parameters: structuredClone(input.parameters),
    initialState: [...input.initialState],
    ...(input.initialStateDefaulted ? { initialStateDefaulted: true } : {}),
    modelValuesTouched: false,
    commandSources: [...input.commandSources],
    feedbackTargets: [...input.feedbackTargets],
    stepMs: input.stepMs,
    ...(input.projectStepMs === undefined ? {} : { projectStepMs: input.projectStepMs }),
    initialInput: input.initialInput ? [...input.initialInput] : Array(input.commandSources.length).fill(0),
    inputScale: input.inputScale ? [...input.inputScale] : Array(input.commandSources.length).fill(1),
    inputOffset: input.inputOffset ? [...input.inputOffset] : Array(input.commandSources.length).fill(0),
    outputScale: input.outputScale ? [...input.outputScale] : Array(input.feedbackTargets.length).fill(1),
    outputOffset: input.outputOffset ? [...input.outputOffset] : Array(input.feedbackTargets.length).fill(0),
    actuator: input.actuator ? { ...input.actuator } : undefined,
    uncertainties: input.uncertainties?.map((effect) => ({ ...effect })),
    scenarioEvents: input.scenarioEvents?.map((event) => ({ ...event })) ?? [],
    savedConfigurationRevision: input.savedConfigurationRevision,
    activeConfigurationDigest: input.activeConfigurationDigest,
    dirty: false,
    persisted: true,
    focusSection: "plant",
  };
}

export function createPlantDraft(target: { id: string; name: string }): PlantDraft {
  return {
    id: `draft:plant:${target.id}`,
    ...createPlantEditorDraft({
      runtimeId: target.id,
      runtimeName: target.name,
      existingPlantIds: [],
    }),
    persisted: false,
    focusSection: "plant",
  };
}

/** Selects a stable return point after an unsaved draft is discarded. */
export function plantDraftCancelTarget(input: {
  previousSelectedId?: string;
  runtimeId: string;
  availableNodeIds: string[];
}): string | undefined {
  const available = new Set(input.availableNodeIds);
  if (input.previousSelectedId && available.has(input.previousSelectedId)) {
    return input.previousSelectedId;
  }
  return available.has(input.runtimeId) ? input.runtimeId : undefined;
}

export type PlantDragDecodeResult =
  | { ok: true; dragSessionId: string }
  | {
      ok: false;
      code:
        | "malformed_drag_payload"
        | "unsupported_drag_payload"
        | "stale_drag_payload";
    };

export function encodePlantDragPayload(dragSessionId: string): string {
  if (dragSessionId.trim().length === 0) {
    throw new Error("drag session id must not be empty");
  }
  return JSON.stringify({
    schema_version: 1,
    kind: "simulated_plant",
    drag_session_id: dragSessionId,
  });
}

export function decodePlantDragPayload(
  raw: string,
  activeDragSessionId: string
): PlantDragDecodeResult {
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    return { ok: false, code: "malformed_drag_payload" };
  }
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return { ok: false, code: "malformed_drag_payload" };
  }
  const record = value as Record<string, unknown>;
  const keys = Object.keys(record).sort();
  if (
    keys.length !== 3 ||
    keys[0] !== "drag_session_id" ||
    keys[1] !== "kind" ||
    keys[2] !== "schema_version" ||
    typeof record.drag_session_id !== "string" ||
    record.drag_session_id.length === 0 ||
    typeof record.schema_version !== "number" ||
    !Number.isInteger(record.schema_version) ||
    typeof record.kind !== "string"
  ) {
    return { ok: false, code: "malformed_drag_payload" };
  }
  if (record.schema_version !== 1 || record.kind !== "simulated_plant") {
    return { ok: false, code: "unsupported_drag_payload" };
  }
  if (record.drag_session_id !== activeDragSessionId) {
    return { ok: false, code: "stale_drag_payload" };
  }
  return { ok: true, dragSessionId: record.drag_session_id };
}

export type PlantDropResult =
  | { ok: true; runtimeId: string }
  | {
      ok: false;
      code:
        | "malformed_drag_payload"
        | "unsupported_drag_payload"
        | "stale_drag_payload"
        | "invalid_drop_target"
        | "ineligible_drop_target";
      message: string;
    };

export function resolvePlantDrop(input: {
  raw: string;
  activeDragSessionId: string;
  targetRuntimeId?: string;
  eligibleRuntimeIds: string[];
}): PlantDropResult {
  const decoded = decodePlantDragPayload(input.raw, input.activeDragSessionId);
  if (!decoded.ok) {
    return decoded.code === "stale_drag_payload"
      ? {
          ok: false,
          code: decoded.code,
          message: "This drag is no longer active. Start again from Simulated plant.",
        }
      : {
          ok: false,
          code: decoded.code,
          message: "Unsupported plant drag. Start again from Simulated plant.",
        };
  }
  if (!input.targetRuntimeId) {
    return {
      ok: false,
      code: "invalid_drop_target",
      message: "Drop the plant onto a runtime.",
    };
  }
  if (!input.eligibleRuntimeIds.includes(input.targetRuntimeId)) {
    return {
      ok: false,
      code: "ineligible_drop_target",
      message: "This runtime cannot accept a simulated plant.",
    };
  }
  return { ok: true, runtimeId: input.targetRuntimeId };
}

export function plantDropPresentation(input: {
  runtimeId: string;
  eligible: boolean;
  activeTargetRuntimeId?: string;
  ineligibleReason?: string;
}): { state: "eligible" | "current" | "ineligible"; label: string } {
  if (!input.eligible) {
    return {
      state: "ineligible",
      label:
        input.ineligibleReason?.trim() ||
        "This runtime cannot accept a simulated plant.",
    };
  }
  return input.runtimeId === input.activeTargetRuntimeId
    ? { state: "current", label: "Add simulated plant" }
    : { state: "eligible", label: "Drop simulated plant" };
}

function asCanvasPlant(draft: PlantDraft): NCPlant {
  return {
    id: draft.id,
    displayName: draft.displayName,
    model: draft.model,
    enabled: true,
    commandSources: draft.commandSources,
    feedbackTargets: draft.feedbackTargets,
    draft: true,
    statusAvailability: "not_requested",
  };
}

/** Projects an unsaved draft into one runtime without mutating saved topology. */
export function projectPlantDraft(graph: NCGraph, draft: PlantDraft): NCGraph {
  const append = (runtime: NCRuntime): NCRuntime =>
    runtime.id === draft.runtimeId
      ? {
          ...runtime,
          plants: [
            ...(runtime.plants ?? []).filter((plant) => plant.id !== draft.id),
            asCanvasPlant(draft),
          ],
        }
      : runtime;
  const host = (value: NCHost): NCHost => ({
    ...value,
    runtimes: value.runtimes.map(append),
    containers: value.containers.map((container) => ({
      ...container,
      runtimes: container.runtimes.map(append),
    })),
  });
  return { ...graph, hosts: graph.hosts.map(host) };
}
