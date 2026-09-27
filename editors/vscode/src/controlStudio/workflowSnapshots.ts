import { restoredEngineerView } from "./restoredEngineerView";
import { searchSummary } from "./finiteSearchPersistence";
import * as vscode from "vscode";
import { readProjectPlants } from "../networkCanvas/canvasDataService";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopRecording } from "./loopMonitor";
import { workflowSimulationPlantContext, type ControlStudioWorkflowResult, type WorkflowResultSample } from "./workflowResult";
import { buildEngineeringPlotEvidence, engineeringPlotMessageEvidence } from "./engineeringPlots";

/** Preserves acquisition evidence; UI observations never become runtime samples. */
export function recordingSamples(completedRecording: LoopRecording | undefined, retained: readonly WorkflowResultSample[]): readonly WorkflowResultSample[] {
  const recordedSamples = completedRecording?.samples ?? [];
  const firstObservedAt = recordedSamples[0]?.observedAtMs ?? 0;
  return completedRecording?.rawSamples ?? (recordedSamples.length > 0
    ? recordedSamples.map((sample, index) => ({
      elapsedSeconds: (sample.observedAtMs - firstObservedAt) / 1_000,
      observedAtMs: sample.observedAtMs,
      actualCycleSeconds: null,
      editorObservationIntervalSeconds: index === 0 ? null :
        (sample.observedAtMs - recordedSamples[index - 1].observedAtMs) / 1_000,
      additional: sample.additional?.map(signal => ({ id: signal.id, channel: signal.channel, role: signal.role,
        unit: signal.unit, value: signal.value, quality: signal.quality === "forced" ? "forced" : signal.quality === "good" ? "good" : "bad" })),
      scan: sample.scan,
      source: sample.source,
      loopRevision: sample.loopRevision,
      sp: sample.setpoint?.value ?? null,
      pv: sample.processVariable.value,
      requestedMv: null,
      appliedMv: sample.manipulatedVariable.value,
      units: { sp: sample.setpoint?.unit ?? "", pv: sample.processVariable.unit, mv: sample.manipulatedVariable.unit },
      quality: [sample.setpoint, sample.processVariable, sample.manipulatedVariable]
        .filter((signal): signal is NonNullable<typeof signal> => signal !== undefined)
        .some((signal) => signal.quality === "forced") ? "forced" : "good",
      freshnessMs: null,
      controllerMode: sample.controller.mode,
      outputLimits: { ...sample.outputLimits },
      rateLimits: { ...sample.rateLimits },
      safetyState: "Unavailable",
      events: [
        ...(index === 0 ? ["recording_started"] : []),
        ...(index === recordedSamples.length - 1 ? ["recording_stopped"] : []),
      ],
    }))
    : retained);
}

export async function loadWorkflowSimulationPlant(
  context: vscode.ExtensionContext,
  loop: ControlLoopDefinition
) {
  const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  if (!projectDir || !loop.plantId) {
    throw new Error("The saved simulation plant configuration is unavailable for this loop.");
  }
  const saved = await readProjectPlants(context, projectDir);
  const validityRange = `PV ${loop.signals.processVariable.range.minimum}…${loop.signals.processVariable.range.maximum} ${loop.signals.processVariable.unit}; ` +
    `MV ${loop.signals.manipulatedVariable.range.minimum}…${loop.signals.manipulatedVariable.range.maximum} ${loop.signals.manipulatedVariable.unit}`;
  const plant = workflowSimulationPlantContext(saved, loop.plantId, validityRange);
  if (!plant) throw new Error("The saved simulation plant configuration is invalid or no longer matches this loop.");
  return plant;
}


export async function postRestoredEngineeringPlots(result: ControlStudioWorkflowResult, postEngineering: (stage: string, payload: unknown) => Promise<void>): Promise<void> {
  if (result.engineerStudy) {
    await postEngineering("engineer", restoredEngineerView(result.engineerStudy, result.commissioning));
  }
  if (result.finiteSearch) await postEngineering("search", { status: "complete", result: searchSummary(result.finiteSearch) });
  if (result.runValidation) await postEngineering("run-validation", { status: "complete", saved: result.runValidation });
  if (result.generalModelAnalysis) await postEngineering("general-model", { status: "complete", saved: result.generalModelAnalysis });
  if (result.periodicFrequencyAnalysis) await postEngineering("general-model", { status: "periodic-complete", periodic: result.periodicFrequencyAnalysis });
  if (result.leadLagAnalysis) await postEngineering("lead-lag", { status: "complete", saved: result.leadLagAnalysis });
  if (result.declaredAnalysis) {
    await postEngineering("declared-root", { status: "complete", result: result.declaredAnalysis.result });
  }
  if (result.routhAnalysis) {
    await postEngineering("routh", { status: "complete", result: result.routhAnalysis.result });
  }
  if (!result.model || !result.candidate) return;
  const current = result.currentController ?? result.candidate;
  const plots = buildEngineeringPlotEvidence(result.model, current, result.candidate);
  await postEngineering("tune", {
    status: "complete",
    candidate: result.candidate,
    current,
    plots: engineeringPlotMessageEvidence(plots),
    recommendation: "Restored from the saved model and controller evidence; the running controller is unchanged.",
  });
}
