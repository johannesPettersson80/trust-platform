import type { PlantEditorDraft } from "./editorModel";
import { stateSpaceDimensions } from "./stateSpaceEditor";
import { validateManualPlantSignalAddress, type PlantSignalCatalog, type PlantSignalDirection } from "./signalCatalog";

export interface PlantPortIssue {
  direction: PlantSignalDirection;
  index: number;
  message: string;
}

export function plantPortLabels(draft: Pick<PlantEditorDraft, "model" | "parameters">): { commands: string[]; feedbacks: string[] } {
  if (draft.model === "gravity_tank_cascade") {
    return { commands: ["Pump flow (m³/s)"], feedbacks: ["Upstream level (m)", "Downstream level (m)"] };
  }
  if (draft.model === "robot_joint") {
    return { commands: ["Torque command (N·m)"], feedbacks: ["Joint angle (rad)", "Angular speed (rad/s)"] };
  }
  if (draft.model === "state_space") {
    const dimensions = stateSpaceDimensions(draft.parameters);
    return {
      commands: Array.from({ length: dimensions.inputs }, (_, index) => `Input ${index + 1}`),
      feedbacks: Array.from({ length: dimensions.outputs }, (_, index) => `Output ${index + 1}`),
    };
  }
  return { commands: ["Command / PLC output"], feedbacks: ["Feedback / PLC input"] };
}

export function validatePlantPortBindings(draft: PlantEditorDraft, catalog: PlantSignalCatalog, usedFeedbackTargets: readonly string[]): PlantPortIssue[] {
  const labels = plantPortLabels(draft);
  const issues: PlantPortIssue[] = [];
  for (const [direction, addresses, names] of [
    ["command", draft.commandSources, labels.commands],
    ["feedback", draft.feedbackTargets, labels.feedbacks],
  ] as const) {
    for (let index = 0; index < Math.max(addresses.length, names.length); index += 1) {
      if (index >= names.length) {
        issues.push({ direction, index, message: `This model has ${names.length} ${direction} port(s). Review the extra binding before saving.` });
        continue;
      }
      const address = addresses[index];
      if (!address) {
        issues.push({ direction, index, message: `Choose a PLC ${direction === "command" ? "output" : "input"} for ${names[index]}.` });
        continue;
      }
      const result = validateManualPlantSignalAddress(catalog, direction, address, {
        usedFeedbackTargets: [...usedFeedbackTargets, ...draft.feedbackTargets.filter((_, other) => direction === "feedback" && other !== index)],
      });
      if (!result.ok) issues.push({ direction, index, message: `${names[index]}: ${result.message}` });
    }
  }
  return issues;
}

/** Port indices are identities: clearing one leaves a hole instead of shifting its neighbors. */
export function updatePlantPortBinding<T extends PlantEditorDraft>(draft: T, direction: PlantSignalDirection, index: number, address: string): T {
  const labels = plantPortLabels(draft);
  const count = direction === "command" ? labels.commands.length : labels.feedbacks.length;
  const current = direction === "command" ? draft.commandSources : draft.feedbackTargets;
  if (!Number.isInteger(index) || index < 0 || index >= count) return draft;
  // Retain extra configured ports for explicit review after dimension/model changes.
  const addresses = Array.from({ length: Math.max(count, current.length) }, (_, port) => port === index ? address : current[port] ?? "");
  const extend = (values: number[] | undefined, fallback: number) => values === undefined ? undefined
    : Array.from({ length: Math.max(count, values.length) }, (_, port) => values[port] ?? fallback);
  return direction === "command" ? {
    ...draft, commandSources: addresses, dirty: true,
    initialInput: extend(draft.initialInput, 0), inputScale: extend(draft.inputScale, 1), inputOffset: extend(draft.inputOffset, 0),
  } : {
    ...draft, feedbackTargets: addresses, dirty: true,
    outputScale: extend(draft.outputScale, 1), outputOffset: extend(draft.outputOffset, 0),
  };
}
