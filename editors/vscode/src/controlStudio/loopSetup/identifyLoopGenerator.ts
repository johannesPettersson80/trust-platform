import type { ControlLoopDefinition } from "../loopDefinition";
import type { CompiledCatalog } from "./compiledCatalog";
import { binding, measurementBinding, identificationDuration, loopId, withRevision, type MeasuredSignal } from "./loopBindings";

export interface IdentifyLoopDraft {
  projectId: string; runtimeId: string; name: string;
  manipulatedVariable: MeasuredSignal; processVariable: MeasuredSignal;
  /** How long Studio records the project's experiment, in s. */
  experimentDurationSeconds?: number;
}

/** An open-loop definition that records the chosen plant input and measured output; the program keeps authority. */
export function identifyLoopDefinition(catalog: CompiledCatalog, draft: IdentifyLoopDraft): ControlLoopDefinition {
  const owner = draft.manipulatedVariable.symbol.split(".")[0];
  const program = catalog.controllers.find(entry => entry.instance === owner);
  if (!program) throw new Error(`${draft.manipulatedVariable.symbol} does not belong to a compiled program instance.`);
  if (program.tasks.length !== 1 || !(program.tasks[0].intervalMs > 0)) throw new Error(`${owner} must run in exactly one cyclic task.`);
  const period = program.tasks[0].intervalMs, input = draft.manipulatedVariable;
  return withRevision({
    schemaVersion: 2, id: loopId(draft.name), name: draft.name, projectId: draft.projectId, runtimeId: draft.runtimeId,
    revision: "sha256:pending", topology: "open-loop",
    controller: { instance: owner, type: program.type, design: "project", revision: "sha256:unverified", action: "direct", taskPeriodMs: period },
    signals: { processVariable: measurementBinding(catalog, draft.processVariable), manipulatedVariable: binding(catalog, input, "read") },
    limits: { outputMinimum: input.minimum, outputMaximum: input.maximum },
    observedSampleTiming: { nominalPeriodMs: period },
    experiment: { execution: "project-scripted", kind: "observation", amplitude: 0,
      durationSeconds: identificationDuration(draft.name, draft.experimentDurationSeconds), seed: 1 },
  } as ControlLoopDefinition);
}
