import { calculateDeclaredRootLocus } from "./declaredAnalysis";
import { importedRecordingId } from "./identificationRecordingImport";
import {
  identificationOriginalRecording,
  identificationRecordForSource,
  identificationSamplesFromEvidence,
} from "./panelWorkflowSupport";
import type { ControlPidCandidate } from "./controllerCandidate";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { StudioAnalysisSessions } from "./studioAnalysisSessions";
import type { SavedDeclaredAnalysis } from "./declaredAnalysisPersistence";
import { identificationPresentationState } from "./identificationPresentationState";
import {
  type ExcludedInterval,
  type IdentificationModelSelection,
  type IdentifiedModel,
  type ModelFitEvidence,
} from "./identification";
import { ExperimentIdentificationLedger } from "./experiment";
import type { TuningResult } from "./tuning";
import { excludedIntervalsFromSelection, type ControlStudioWorkflowResult, type WorkflowTuningComparison } from "./workflowResult";
import { type ControlLoopMonitor, type LoopRecording } from "./loopMonitor";
import { decodeEngineeringCommand } from "./messages";
import { type ControllerApplicationClient } from "./controllerApplication";

/** Mutable state owned by one Control Studio editor session. */
export interface ManualWorkflowState {
  model: IdentifiedModel | undefined;
  modelFit: ModelFitEvidence | undefined;
  modelEvidence: ControlStudioWorkflowResult["evidence"]["model"] | undefined;
  candidate: ControlPidCandidate | undefined;
  tuning: TuningResult | undefined;
  tuningComparison: WorkflowTuningComparison | undefined;
  currentRuntimeController: ControlPidCandidate | undefined;
  declaredAnalysis: SavedDeclaredAnalysis | undefined;
  excludedIntervals: ExcludedInterval[];
}

export function createManualWorkflowState(): ManualWorkflowState {
  return {
    model: undefined,
    modelFit: undefined,
    modelEvidence: undefined,
    candidate: undefined,
    tuning: undefined,
    tuningComparison: undefined,
    currentRuntimeController: undefined,
    declaredAnalysis: undefined,
    excludedIntervals: [],
  };
}

export function restoreManualWorkflowState(state: ManualWorkflowState, result: ControlStudioWorkflowResult): void {
  state.model = result.model; state.modelFit = result.modelFit; state.modelEvidence = result.evidence.model;
  state.declaredAnalysis = result.declaredAnalysis; state.candidate = result.candidate;
  state.tuningComparison = result.tuningComparison;
  state.currentRuntimeController = result.currentController;
  const rawSamples = result.recording.rawSamples;
  const endSeconds = rawSamples.length ? rawSamples[rawSamples.length - 1].elapsedSeconds : 0;
  state.excludedIntervals = excludedIntervalsFromSelection(endSeconds, result.recording.selectedIntervals);
}

export interface ManualWorkflowCommandContext {
  command: NonNullable<ReturnType<typeof decodeEngineeringCommand>>;
  loop: ControlLoopDefinition;
  state: ManualWorkflowState;
  sessions: StudioAnalysisSessions;
  selectedRecording: LoopRecording | undefined;
  currentSavedResult: ControlStudioWorkflowResult | undefined;
  currentLoopSample: ReturnType<ControlLoopMonitor["observe"]> | undefined;
  testLedger: ExperimentIdentificationLedger;
  runtimeCandidate: (loop: ControlLoopDefinition, clientFactory: () => Promise<ControllerApplicationClient>) => Promise<ControlPidCandidate>;
  post: (phase: string, payload: unknown) => Promise<void>;
  retainDraft: () => Promise<void>;
  runtimeClient: () => Promise<ControllerApplicationClient>;
}

export async function handleManualWorkflowCommand(input: ManualWorkflowCommandContext): Promise<void> {
  // Capture this editor's state and callbacks before any await. The panel owns
  // the scope check in `post`, so late work cannot publish into a new editor.
  const { command, loop, state, sessions, selectedRecording, currentSavedResult,
    currentLoopSample, testLedger, post, retainDraft } = input;
  try {
    const identificationWorkflow = sessions.identification;
    if (command.action === "identify-edit") {
      const update = sessions.editIdentification(command.form);
      state.model = undefined;
      state.modelFit = undefined;
      state.modelEvidence = undefined;
      state.candidate = undefined;
      state.tuning = undefined;
      state.tuningComparison = undefined;
      await retainDraft();
      await post("identify", identificationPresentationState(
        identificationWorkflow, loop.signals.manipulatedVariable.unit, loop.signals.processVariable.unit));
      if (update.engineer) await post("engineer", update.engineer);
      return;
    }
    if (command.action === "declared-root") {
      if (!loop.analysis) throw new Error("Configure a provided model and parameter sweep first.");
      const definition = { ...sessions.declaredAnalysis(loop.analysis), rootLocus: { ...loop.analysis.rootLocus,
        selectedValue: command.selectedValue ?? loop.analysis.rootLocus.selectedValue } };
      state.declaredAnalysis = { definition, result: calculateDeclaredRootLocus(definition) };
      await post("declared-root", { status: "complete", result: state.declaredAnalysis.result });
      return;
    }
    if (command.action === "identify-qualify") {
      const provenance = selectedRecording?.provenance ?? (!selectedRecording ? currentSavedResult?.recording.provenance : undefined);
      const testRecord = provenance ? undefined : identificationRecordForSource(
        testLedger.records, identificationWorkflow.nextFitRole, selectedRecording);
      if (!testRecord) {
        const acquisition = selectedRecording?.evidence ?? currentSavedResult?.recording.acquisition;
        if (!acquisition || acquisition.completeness !== "complete") {
          throw new Error(acquisition?.reason ??
            "Incomplete recording — retained evidence does not prove that every runtime scan was acquired.");
        }
      }
      const samples = identificationSamplesFromEvidence(testRecord, selectedRecording, currentSavedResult?.recording.rawSamples);
      const excludedIntervals = [...(command.excludedIntervals ?? [])];
      const source = provenance?.source ?? (currentLoopSample?.availability === "available"
        ? currentLoopSample.source : currentSavedResult?.source ?? "Simulation");
      const update = sessions.qualifyIdentification(samples, excludedIntervals, source,
        command.model as IdentificationModelSelection, provenance ? importedRecordingId(provenance) : selectedRecording?.evidence.recordingId ?? testRecord?.experimentId, command.stepResponseSettings,
        identificationOriginalRecording(selectedRecording, testRecord?.plan.kind));
      state.excludedIntervals = excludedIntervals;
      state.model = undefined; state.modelFit = undefined; state.modelEvidence = undefined;
      state.tuning = undefined; state.tuningComparison = undefined;
      state.candidate = undefined;
      await retainDraft();
      await post("identify", identificationPresentationState(identificationWorkflow, loop.signals.manipulatedVariable.unit, loop.signals.processVariable.unit));
      if (update.engineer) await post("engineer", update.engineer);
      return;
    }
    if (command.action === "identify-fit") {
      const [review] = await Promise.all([sessions.fitIdentification(), post("identify", { status: "running", reason: "Calculating model…" })]);
      if (!review) return;
      await retainDraft();
      await post("identify", identificationPresentationState(identificationWorkflow, loop.signals.manipulatedVariable.unit, loop.signals.processVariable.unit));
      return;
    }
    if (command.action === "identify-choose-structure") {
      sessions.chooseIdentificationStructure(command.choice);
      await retainDraft();
      await post("identify", identificationPresentationState(identificationWorkflow, loop.signals.manipulatedVariable.unit, loop.signals.processVariable.unit));
      return;
    }
    if (command.action === "identify-accept") {
      const accepted = identificationWorkflow.accept();
      sessions.acceptIdentification(accepted, loop);
      state.model = accepted.model;
      state.modelFit = accepted.fit;
      state.modelEvidence = accepted.evidence;
      const update = sessions.engineer?.modelChanged();
      await retainDraft();
      await post("identify", identificationPresentationState(identificationWorkflow, loop.signals.manipulatedVariable.unit, loop.signals.processVariable.unit));
      if (update) await post("engineer", update);
      return;
    }
    if (command.action === "identify-cancel") {
      sessions.cancelIdentification();
      await post("identify", { ...identificationPresentationState(identificationWorkflow, loop.signals.manipulatedVariable.unit, loop.signals.processVariable.unit), cancelled: true });
      return;
    }
  } catch (error) {
    await post(command.action.startsWith("identify") ? "identify" : command.action,
      { status: "failed", reason: error instanceof Error ? error.message : String(error) });
  }
}
