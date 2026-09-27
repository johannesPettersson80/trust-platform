import type * as vscode from "vscode";
import { randomUUID } from "crypto";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopRecording } from "./loopMonitor";
import type { ExcludedInterval } from "./identification";
import { text } from "./panelBindings";
import { selectedIntervals } from "./panelPersistence";
import { recordingSamples, loadWorkflowSimulationPlant } from "./workflowSnapshots";
import { createWorkflowResult, type ControlStudioWorkflowResult, type WorkflowPlantContext } from "./workflowResult";

export type WorkflowResultSnapshotState = Pick<ControlStudioWorkflowResult, "model" | "generalModelAnalysis" | "periodicFrequencyAnalysis" | "leadLagAnalysis" | "engineerStudy" | "commissioning"
  | "runValidation" | "finiteSearch" | "declaredAnalysis" | "routhAnalysis" | "modelFit" | "currentController" | "candidate"
  | "validation" | "tuningComparison" | "experiment" | "identificationStudy"> & {
    previousResult?: ControlStudioWorkflowResult;
    source: ControlStudioWorkflowResult["source"];
    recording?: LoopRecording;
    excludedIntervals: readonly ExcludedInterval[];
    modelEvidence?: ControlStudioWorkflowResult["evidence"]["model"];
  };

/** Synchronously captures a validated draft without loading plant metadata or advancing history. */
export function buildWorkflowDraftSnapshot(suppliedLoop: ControlLoopDefinition, captured: WorkflowResultSnapshotState,
  chartSession?: ControlStudioWorkflowResult["chartSession"]): ControlStudioWorkflowResult {
  const state = structuredClone(captured);
  const loop = structuredClone(suppliedLoop);
  const provenance = state.recording ? state.recording.provenance : state.previousResult?.recording.provenance;
  const sameAcquisition = !state.recording || Boolean(state.recording.evidence.recordingId &&
    state.recording.evidence.recordingId === state.previousResult?.recording.acquisition?.recordingId);
  const plant = provenance?.plant ?? (sameAcquisition ? state.previousResult?.plant : undefined);
  return assembleWorkflowResult(loop, state, chartSession, plant,
    state.previousResult?.software.extensionVersion ?? "Unavailable");
}

/** One editor supplies a snapshot synchronously; later metadata reads cannot switch owners. */
export async function buildWorkflowResultSnapshot(context: vscode.ExtensionContext, suppliedLoop: ControlLoopDefinition,
  captured: WorkflowResultSnapshotState, chartSession?: ControlStudioWorkflowResult["chartSession"]): Promise<ControlStudioWorkflowResult> {
  const state = structuredClone(captured);
  const loop = structuredClone(suppliedLoop);
  const provenance = state.recording ? state.recording.provenance : state.previousResult?.recording.provenance;
  const source = provenance?.source ?? state.source;
  const plant = provenance ? provenance.plant : source === "Simulation" && loop.plantId
    ? await loadWorkflowSimulationPlant(context, loop)
    : undefined;
  return assembleWorkflowResult(loop, state, chartSession, plant, text(context.extension.packageJSON.version, "Unavailable"));
}

function assembleWorkflowResult(loop: ControlLoopDefinition, state: WorkflowResultSnapshotState,
  chartSession: ControlStudioWorkflowResult["chartSession"], plant: WorkflowPlantContext | undefined,
  extensionVersion: string): ControlStudioWorkflowResult {
  const rawSamples = recordingSamples(state.recording, state.previousResult?.recording.rawSamples ?? []);
  const provenance = state.recording ? state.recording.provenance : state.previousResult?.recording.provenance;
  const source = provenance?.source ?? state.source;
  const endSeconds = rawSamples.length > 0 ? rawSamples[rawSamples.length - 1].elapsedSeconds : 0;
  const experiment = provenance ? provenance.experiment : state.experiment;
  const engineerStudy = state.engineerStudy;
  const generalModelAnalysis = state.generalModelAnalysis;
  const runValidation = state.runValidation;
  const finiteSearch = state.finiteSearch;
  return createWorkflowResult({
    resultId: state.previousResult?.resultId ?? randomUUID(),
    resultRevision: (state.previousResult?.resultRevision ?? 0) + 1,
    projectId: loop.projectId,
    source,
    loop: { id: loop.id, revision: loop.revision, topology: loop.topology ?? "closed-loop" },
    controller: { instance: loop.controller.instance, revision: loop.controller.revision },
    software: {
      extensionVersion,
      numericalAdapters: { tuning: "trust-control-tuning-v2" },
    },
    units: { sp: loop.signals.setpoint?.unit ?? "", pv: provenance?.units.pv ?? loop.signals.processVariable.unit,
      mv: provenance?.units.mv ?? loop.signals.manipulatedVariable.unit },
    recording: { rawSamples, selectedIntervals: selectedIntervals(endSeconds, state.excludedIntervals),
      ...(provenance ? { provenance } : {}),
      ...(state.recording ? { acquisition: state.recording.evidence } :
        state.previousResult?.recording.acquisition ? { acquisition: state.previousResult.recording.acquisition } : {}) },
    ...(state.model ? { model: state.model } : {}),
    ...(generalModelAnalysis ? { generalModelAnalysis } : {}),
    ...(state.periodicFrequencyAnalysis ? { periodicFrequencyAnalysis: state.periodicFrequencyAnalysis } : {}),
    ...(state.leadLagAnalysis ? { leadLagAnalysis: state.leadLagAnalysis } : {}),
    ...(engineerStudy ? { engineerStudy } : {}),
    ...(state.commissioning ? { commissioning: state.commissioning } : {}),
    ...(runValidation ? { runValidation } : {}),
    ...(finiteSearch ? { finiteSearch } : {}),
    ...(state.declaredAnalysis ? { declaredAnalysis: state.declaredAnalysis } : {}),
    ...(state.routhAnalysis ? { routhAnalysis: state.routhAnalysis } : {}),
    ...(state.modelFit ? { modelFit: state.modelFit } : {}),
    ...(state.identificationStudy ? { identificationStudy: state.identificationStudy } : {}),
    ...(state.currentController ? { currentController: state.currentController } : {}),
    ...(state.candidate ? { candidate: state.candidate } : {}),
    ...(state.validation ? { validation: state.validation } : {}),
    ...(state.tuningComparison ? { tuningComparison: state.tuningComparison } : {}),
    ...(state.candidate?.source ? { method: state.candidate.source } : {}),
    ...(experiment ? { experiment } : {}),
    ...(plant ? { plant } : {}),
    ...(chartSession ? { chartSession } : {}),
    evidence: { observation: rawSamples.length ? "Observed" : "Not observed", model: state.declaredAnalysis || generalModelAnalysis ? "Model-predicted" : state.model
      ? (state.modelEvidence ??
        (source === "Simulation" ? "Identified from simulation data" : "Identified from physical data"))
      : "Not evaluated" },
    conclusion: state.validation
      ? "Validation result retained for review; no controller change was applied by saving."
      : "Workflow state retained for continued analysis.",
  }, new Date().toISOString());
}
