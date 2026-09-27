import { calculateDeclaredRootLocus } from "./declaredAnalysis";
import { importedRecordingId } from "./identificationRecordingImport";
import {
  identificationOriginalRecording,
  identificationRecordForSource,
  identificationSamplesFromEvidence,
  currentRuntimeCandidate,
} from "./panelWorkflowSupport";
import { generatedPiCandidate } from "./pidCandidateFactory";
import { candidateFromControlPid, type ControlPidCandidate } from "./controllerCandidate";
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
import { describeTuningMethod, tuneController, type TuningResult } from "./tuning";
import { buildEngineeringPlotEvidence, engineeringPlotMessageEvidence } from "./engineeringPlots";
import {
  compareControlPidCandidates,
  validationRequirementRows,
  type ValidationResult,
} from "./validation";
import { excludedIntervalsFromSelection, type ControlStudioWorkflowResult, type WorkflowTuningComparison } from "./workflowResult";
import { formatEngineeringNumber } from "./chartContract";
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
  validation: ValidationResult | undefined;
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
    validation: undefined,
    currentRuntimeController: undefined,
    declaredAnalysis: undefined,
    excludedIntervals: [],
  };
}

export function restoreManualWorkflowState(state: ManualWorkflowState, result: ControlStudioWorkflowResult): void {
  state.model = result.model; state.modelFit = result.modelFit; state.modelEvidence = result.evidence.model;
  state.declaredAnalysis = result.declaredAnalysis; state.candidate = result.candidate;
  state.validation = result.validation; state.tuningComparison = result.tuningComparison;
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
    currentLoopSample, testLedger, post, retainDraft, runtimeCandidate, runtimeClient } = input;
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
      state.validation = undefined;
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
      state.candidate = undefined; state.validation = undefined;
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
    if (command.action === "tune-settings") {
      if (!state.model) throw new Error("Identify and accept a model before tuning.");
      state.candidate = candidateFromControlPid(command.settings, command.method);
      state.tuning = undefined; state.tuningComparison = undefined; state.validation = undefined;
      const current = await runtimeCandidate(loop, runtimeClient);
      state.currentRuntimeController = current;
      const plots = buildEngineeringPlotEvidence(state.model, current, state.candidate);
      await post("tune", { status: "complete", candidate: state.candidate, current,
        plots: engineeringPlotMessageEvidence(plots),
        recommendation: `${command.method} settings retained exactly; the running controller is unchanged.` });
      return;
    }
    if (command.action === "tune") {
      if (!state.model) throw new Error("Identify and accept a model before tuning.");
      const methodEvidence = describeTuningMethod(state.model, command.method, command.objective,
        loop.signals.manipulatedVariable.unit, loop.signals.processVariable.unit);
      if (methodEvidence.status === "Unavailable") {
        await post("tune", { status: "unavailable", reason: methodEvidence.reason, methodEvidence });
        return;
      }
      state.tuning = tuneController(state.model, { method: command.method, objective: command.objective });
      state.tuningComparison = {
        objective: command.objective,
        winnerMethod: command.method,
        ...(state.tuning.closedLoopTimeSeconds === undefined ? {} :
          { closedLoopTimeSeconds: state.tuning.closedLoopTimeSeconds }),
        attempts: [{ id: command.method.toLowerCase(), method: command.method,
          structure: "PI", status: "Feasible",
          ...(state.tuning.closedLoopTimeSeconds === undefined ? {} :
            { closedLoopTimeSeconds: state.tuning.closedLoopTimeSeconds }) }],
      };
      const tiSeconds = state.tuning.controller.tiSeconds;
      if (tiSeconds === undefined) throw new Error("The selected candidate is not a PI controller.");
      state.candidate = generatedPiCandidate(state.tuning.controller.kp, tiSeconds, command.method, loop);
      state.validation = undefined;
      const current = await runtimeCandidate(loop, runtimeClient);
      state.currentRuntimeController = current;
      const plots = buildEngineeringPlotEvidence(state.model, current, state.candidate);
      const methodDetail = command.method === "SIMC"
        ? `lambda ${formatEngineeringNumber(state.tuning.closedLoopTimeSeconds)} s`
        : "published AMIGO coefficients; Response affects comparison ranking, not the formula";
      await post("tune", { status: "complete", result: state.tuning,
        candidate: state.candidate, current, plots: engineeringPlotMessageEvidence(plots), methodEvidence,
        recommendation: `${command.method} ${state.candidate.structure} is unapplied · ${methodDetail}; compare the prediction before validation.` });
      return;
    }
    if (!state.model || !state.candidate) throw new Error("Generate a candidate before validation.");
    const current = await runtimeCandidate(loop, runtimeClient);
    state.currentRuntimeController = current;
    state.validation = compareControlPidCandidates(state.model, current, state.candidate, {
      evidence: "Model-predicted",
      scenarios: [{ kind: "setpoint", amplitude: 1 }, { kind: "disturbance", amplitude: 0.2 },
        { kind: "noise", amplitude: 0.02 }],
      variationFractions: [-0.2, 0, 0.2],
      processUnit: loop.signals.processVariable.unit,
      outputUnit: loop.signals.manipulatedVariable.unit,
    });
    await post("validate", { status: "complete", result: {
      ...state.validation,
      requirements: validationRequirementRows(state.validation.requirements),
    },
      acceptable: !state.validation.requirements.some((requirement) =>
        requirement.status === "Fail" || requirement.status === "Inconclusive") });
  } catch (error) {
    await post(command.action.startsWith("identify") ? "identify" : command.action,
      { status: "failed", reason: error instanceof Error ? error.message : String(error) });
  }
}
