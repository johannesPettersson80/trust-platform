import { connectedIdentificationPanelPorts, clearIdentificationRecording } from "./connectedIdentificationPanel";
import { routeIdentificationRecordingImport } from "./identificationRecordingImport";
import { decodeLoopLiveViewCommand, decodeLoopReady } from "./messages";
import { ThreePageBindingRoute, adoptStudioBinding, type StudioBindingOpen } from "./threePageBindingRoute";
import { createControlStudioRuntimeStartHandler, routeControlStudioRuntimeStart } from "./runtimeStartAction";
import { restoredEngineerStage, restoredEngineerViewSelection, storeEngineerStage } from "./engineerNavigationStorage";
import { completedCommissioningMatchesCheckpoint } from "./commissioningCheckpoint";
import { storeEngineerDraft, restoreEngineerDraft, restoreWorkflowDraft } from "./engineerDraftStorage";
import { observeStudioApplication } from "./observedStudioApplication";
import { restoredEngineerView } from "./restoredEngineerView";
import { restoreCommissioningCheckpoint, storeCommissioningCheckpoint, restoreIsolatedOperation, storeIsolatedOperation, restoreMimoAnalysis } from "./panelPersistence";
import { preserveUnresolvedRecovery, preservePendingManualApplication, preservePendingIdentification, deferRecoveryReopen } from "./unresolvedRecoveryPersistence";
import { buildWorkflowResultSnapshot, buildWorkflowDraftSnapshot, type WorkflowResultSnapshotState } from "./workflowResultSnapshot";
import { SymbolicLoopObservation } from "./symbolicLoopObservation";
import { StudioAnalysisSessions } from "./studioAnalysisSessions";
import { retainedPeriodicRecordingSource } from "./periodicRecordingSource";
import { postRestoredEngineeringPlots } from "./workflowSnapshots";
import { RecordingCoordinator, executeRecordingCommand, sameRecordingScope } from "./recordingCoordinator";
import { RuntimeRecordingSession } from "./runtimeRecording";
import { ExperimentRecordingOwner, restoreExperimentRecordingOwner, startExperimentWithRecording, stopExperimentRecording } from "./experimentRecording";
import { ExperimentStatusPoller } from "./experimentStatusPoller";
import { refreshTestExperiment } from "./testExperimentLifecycle";
import { buildTerminalTestEvidence } from "./testTerminalEvidence";
import { AntiWindupPanelSession, routeAntiWindupPanelCommand } from "./antiWindupPanelWorkflow";
import { RelayTuningSession } from "./relayTuning";
import { currentRuntimeCandidate, currentStageEvidence, postApplicationResult, postEngineeringResult, postExperimentResult, runtimeExperimentClient } from "./panelWorkflowSupport";
import { identificationPresentationState } from "./identificationPresentationState";
import { restoreRecordingRecoveryOwner,retryRecordingRecoveryOwner } from "./recordingRecoveryRestore";
import { randomUUID } from "crypto";
import * as path from "path";
import * as vscode from "vscode";
import { configureControlStudioCommand, openControlStudioCommand, registerControlStudioCommands } from "./openCommand";
import { completeLoopRequest,completePlantHandoff,isLoopRequest,text } from "./panelBindings";
import {
clearExperimentRecovery,clearRecordingRecovery,controlStudioScopeKey,
reconcileManualApplicationRecovery,
storePendingManualApplicationRecovery,
restoreExperimentRecovery,storeRecordingRecovery,
restoreWorkflowResultHistory,
storeExperimentRecovery,
workflowResultStorageKey
} from "./panelPersistence";
import { controlStudioHtml } from "./panelPresentation";
import { connectRuntimeParameters } from "./runtimeParameterConnection";
import type { PlantControlStudioHandoff } from "./handoff";
import type { ControlLoopDefinition } from "./loopDefinition";

import { readProjectPlants } from "../networkCanvas/canvasDataService";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import { runtimeLifecycleService } from "../runtimeLifecycle";
import { saveControlStudioChartExport, type ChartExportHost, type ChartExportScope } from "./chartExportHost";
import { runSavedSimulationAutomaticTune } from "./automaticTune";
import { candidateFromControlPid } from "./controllerCandidate";
import { ControllerApplicationClient } from "./controllerApplication";
import {
buildControlExperimentPlan,
classifyControlExperimentStatus,
ExperimentIdentificationLedger,
type ControlExperimentPlan,
} from "./experiment";
import { ControlLoopMonitor,controlLoopStaleAfterMs,type LoopRecording } from "./loopMonitor";
import {
automaticTuneResultEnvelope,
decodeAutomaticTuneApprove,
decodeControlExperimentCommand,
decodeAntiWindupCommand,
decodeControllerApplicationCommand,
decodeControlStudioNavigatePlant,
decodeControlStudioReady,
decodeEngineeringCommand,
decodeLoopAutomaticTuneApprove,
decodeLoopRecordingCommand,
decodeWorkflowResultCommand,
decodeMimoAnalysisCommand,
loopMonitorEnvelope,loopRecordingEvidenceEnvelope,
plantMonitorEnvelope,
} from "./messages";
import { loadPlantMonitor } from "./plantMonitor";
import {
compareWorkflowResultRevisions,
exportSamplesCsv,
exportWorkflowJson,
saveWorkflowResultRevision,
workflowExperimentContext,
workflowResultForExport,
workflowResultHistoryItems,
type ControlStudioWorkflowResult,
} from "./workflowResult";
import { createManualWorkflowState, handleManualWorkflowCommand, restoreManualWorkflowState, type ManualWorkflowState } from "./manualWorkflowCommands";
import { handleMimoAnalysisCommand } from "./mimoPanelActions";

export const CONTROL_STUDIO_OPEN_COMMAND = "trust-lsp.controlStudio.open";
const startStudioSimulator = createControlStudioRuntimeStartHandler(runtimeLifecycleService);

const CONTROL_STUDIO_VIEW_TYPE = "trust-control-studio";

let panel: vscode.WebviewPanel | undefined;
let extensionContext: vscode.ExtensionContext | undefined;
let activeHandoff: PlantControlStudioHandoff | undefined;
let activeLoop: ControlLoopDefinition | undefined;
let editorSessionId = "";
let bindingDiscovery: import("./configureLoop").DiscoverControlSignals | undefined;
let bindingRoute: ThreePageBindingRoute | undefined;
let openRequestGeneration = 0;
let draftSessions: StudioAnalysisSessions | undefined;
let refreshTimer: NodeJS.Timeout | undefined;
const experimentStatusPoller = new ExperimentStatusPoller(() => { void refreshExperimentStatus(); });
let symbolicLoopObservation = new SymbolicLoopObservation();
let loopMonitor = new ControlLoopMonitor({ staleAfterMs: 1_500, recordingCapacity: 12_000 });
let completedRecording: LoopRecording | undefined;
const runtimeRecorder = new RecordingCoordinator(RuntimeRecordingSession.start);
const experimentRecorder = new ExperimentRecordingOwner(RuntimeRecordingSession.start);
let currentLoopSample: ReturnType<ControlLoopMonitor["observe"]> | undefined;
let pendingExperiment: ControlExperimentPlan | undefined;
let currentExperimentId: string | undefined;
let currentExperimentPlan: ControlExperimentPlan | undefined;
let currentExperimentTerminal: unknown;
let experimentIdentification = new ExperimentIdentificationLedger();
const antiWindupSession = new AntiWindupPanelSession();
const relayTuningSession = new RelayTuningSession();
let manualWorkflowState: ManualWorkflowState = createManualWorkflowState();
let analysisSessions = new StudioAnalysisSessions();
let currentWorkflowResult: ControlStudioWorkflowResult | undefined;
let currentResultIsDraft = false;
let currentWorkflowHistory: ControlStudioWorkflowResult[] = [];
let workflowResultUnavailableReason: string | undefined;
let viewingWorkflowResult = false;
let appliedController: { instancePath: string; previousRevision: string; activeRevision: string } | undefined;
let activeScopeKey = "";
let manualRecoveryReconciled = false;

async function handleChartExportMessage(message: unknown, scope: ChartExportScope): Promise<Awaited<ReturnType<typeof saveControlStudioChartExport>> | undefined> {
  if (!message || typeof message !== "object" || (message as { kind?: unknown }).kind !== "control-studio.chart.export") return undefined;
  const host: ChartExportHost = {
    showSaveDialog: async options => {
      const extension = options.mimeType === "image/png" ? "png" : options.mimeType === "text/csv" ? "csv" : "json";
      const defaultUri = vscode.workspace.workspaceFolders?.[0]
        ? vscode.Uri.joinPath(vscode.workspace.workspaceFolders[0].uri, options.defaultFileName) : undefined;
      return vscode.window.showSaveDialog({ title: "Save export", defaultUri, filters: { [options.mimeType === "image/png" ? "PNG image" : options.mimeType === "text/csv" ? "CSV data" : "JSON data"]: [extension] } });
    },
    writeFile: async (uri, bytes) => { await vscode.workspace.fs.writeFile(uri as vscode.Uri, bytes); },
  };
  return saveControlStudioChartExport(message, scope, host);
}

export function registerControlStudioPanel(
  context: vscode.ExtensionContext,
  discover?: import("./configureLoop").DiscoverControlSignals
): void {
  extensionContext = context; bindingDiscovery = discover;
  registerControlStudioCommands(context, {
    configure: () => configureControlStudioCommand(context, showControlStudio, discover),
    open: request => openControlStudioCommand(context, request, showControlStudio, discover),
    exportRecording: async () => {
      if (!panel?.active || !activeLoop || !(completedRecording?.rawSamples?.length || currentWorkflowResult?.recording.rawSamples.length)) {
        void vscode.window.showInformationMessage("Open a completed recording in Control Studio before saving it."); return;
      }
      await handleWorkflowResultCommand(context, { action: "export-json", messageId: randomUUID() }, activeLoop);
    },
  });
  context.subscriptions.push(
    runtimeLifecycleService.onDidChange(() => scheduleMonitorRefresh())
  );
}

async function showControlStudio(
  context: vscode.ExtensionContext,
  request?: unknown, bindingOpen?: StudioBindingOpen
): Promise<void> {
  const loopRequest = completeLoopRequest(request);
  const handoff = completePlantHandoff(request);
  if (isLoopRequest(request) && !loopRequest) {
    void vscode.window.showErrorMessage("Control Studio could not open: the loop binding is invalid or outside the requested runtime.");
    return;
  }
  const nextScopeKey = controlStudioScopeKey(loopRequest, handoff);
  if ((runtimeRecorder.loop && !sameRecordingScope(runtimeRecorder.loop, loopRequest)) ||
      (experimentRecorder.loop && !sameRecordingScope(experimentRecorder.loop, loopRequest))) {
    void vscode.window.showErrorMessage("Stop the active recording before opening another loop.");
    const loop = runtimeRecorder.loop ?? experimentRecorder.loop;
    if (!loop) return;
    await showControlStudio(context, { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
    return;
  }
  if (panel && nextScopeKey === activeScopeKey) {
    panel.reveal(vscode.ViewColumn.Active);
    scheduleMonitorRefresh();
    return;
  }
  const openTicket = ++openRequestGeneration;
  if (loopRequest && deferRecoveryReopen(loopRequest, () => {
    if (openTicket === openRequestGeneration) void showControlStudio(context, request).catch(error => vscode.window.showErrorMessage(String(error)));
  })) return;
  disposeCurrentAnalysisSessions(context);
  const sessions = new StudioAnalysisSessions(loopRequest, () => manualWorkflowState.declaredAnalysis, async recording => {
    if (analysisSessions !== sessions || !loopRequest || !sameRecordingScope(activeLoop, loopRequest)) return;
    completedRecording = recording; viewingWorkflowResult = false;
    await panel?.webview.postMessage(loopRecordingEvidenceEnvelope(loopMessageScope(loopRequest), randomUUID(), recording.evidence, recording));
  }, loopRequest ? async owner => storePendingManualApplicationRecovery(context, loopRequest, owner) : undefined,
  loopRequest ? async post => {
    const terminal = await reconcileManualApplicationRecovery(context, loopRequest, async recovery =>
      Boolean(await sessions.engineer?.reconcileManualApplicationRecovery(recovery, post)));
    if (terminal) manualRecoveryReconciled = true;
    return terminal;
  } : undefined, loopRequest ? async attempt => storeIsolatedOperation(context, loopRequest, attempt) : undefined,
  loopRequest ? async checkpoint => storeCommissioningCheckpoint(context, loopRequest, checkpoint) : undefined,
  (recordingId, recordingRevision) => retainedPeriodicRecordingSource(completedRecording, recordingId, recordingRevision), loopRequest ? connectedIdentificationPanelPorts(context, loopRequest,
    async () => {
      await refreshLoopMonitor();
      if (analysisSessions !== sessions || !currentLoopSample) throw new Error("The connected loop changed during identification.");
      return currentLoopSample;
    }, async recording => { if (analysisSessions === sessions) completedRecording = recording; }) : undefined);
  analysisSessions = sessions; editorSessionId = bindingOpen?.sessionId ?? randomUUID();
  activeLoop = loopRequest;
  activeHandoff = handoff;
  activeScopeKey = nextScopeKey;
  symbolicLoopObservation = new SymbolicLoopObservation();
  loopMonitor = new ControlLoopMonitor({
    staleAfterMs: activeLoop ? controlLoopStaleAfterMs(activeLoop) : 1_500,
    recordingCapacity: 12_000,
  });
  completedRecording = undefined;
  experimentIdentification = new ExperimentIdentificationLedger();
  antiWindupSession.reset();
  relayTuningSession.reset();
  loopMonitor.setRuntimeRecording(runtimeRecorder.recording || experimentRecorder.recording);
  currentLoopSample = undefined;
  pendingExperiment = undefined;
  const recoveredExperiment = restoreExperimentRecovery(context, activeLoop);
  currentExperimentId = recoveredExperiment?.experimentId;
  currentExperimentPlan = recoveredExperiment?.plan;
  currentExperimentTerminal = undefined;
  manualWorkflowState = createManualWorkflowState();
  const restoredWorkflow = restoreWorkflowResultHistory(context, activeLoop);
  currentWorkflowHistory = restoredWorkflow.history;
  workflowResultUnavailableReason = restoredWorkflow.unavailableReason;
  currentWorkflowResult = activeLoop
    ? [...currentWorkflowHistory].reverse().find((result) => result.loop.revision === activeLoop?.revision)
    : undefined;
  currentResultIsDraft = false;
  try {
    const workflowDraft = activeLoop && restoreWorkflowDraft(context, activeLoop);
    if (workflowDraft) { currentWorkflowResult = workflowDraft; currentResultIsDraft = true; }
    if (currentWorkflowResult && !await activateWorkflowResult(currentWorkflowResult, false)) return;
  } catch (error) {
    if (analysisSessions !== sessions || openTicket !== openRequestGeneration) return;
    currentWorkflowResult = undefined;
    workflowResultUnavailableReason = `The working draft could not be restored. Stored evidence was preserved: ${String(error)}`;
  }
  if (analysisSessions !== sessions || openTicket !== openRequestGeneration) return;
  // Reopening restores the working study. Explicitly opened recordings own
  // their chart projection; restoring a draft must not suspend live observation.
  viewingWorkflowResult = false;
  if (loopRequest) {
    const draft = workflowResultUnavailableReason ? undefined : restoreEngineerDraft(context, loopRequest) ?? currentWorkflowResult?.engineerStudy;
    if (draft) {
      // Restore history intact before the latest durable owner reconciles it.
      sessions.engineer?.restoreStudy(draft, false);
      if (draft.selectedModel) sessions.model.selectModel(draft.selectedModel);
    }
    const commissioning = restoreCommissioningCheckpoint(context, loopRequest);
    const historicalReceipt = currentWorkflowResult?.commissioning;
    sessions.engineer?.restoreCommissioning(!commissioning || completedCommissioningMatchesCheckpoint(historicalReceipt, commissioning)
      ? historicalReceipt : undefined);
    if (commissioning) sessions.engineer?.restoreCommissioningCheckpoint(commissioning);
    const isolated = restoreIsolatedOperation(context, loopRequest)
      ?? [...(draft?.verificationAttempts ?? [])].reverse().find(attempt => attempt.status === "cancellation-unconfirmed");
    if (isolated) sessions.engineer?.restoreIsolatedOperation(isolated);
  }
  if (loopRequest && bindingOpen) await adoptStudioBinding(context, loopRequest, sessions, loopMessageScope(loopRequest), bindingOpen);
  bindingRoute = loopRequest ? new ThreePageBindingRoute(loopRequest, loopMessageScope(loopRequest), bindingDiscovery,
    () => analysisSessions === sessions && activeLoop === loopRequest,
    () => { if (currentExperimentId || runtimeRecorder.recording || experimentRecorder.recording) throw new Error("Finish or abort the current experiment before changing its binding."); sessions.engineer?.assertModelReplacementAllowed(); },
    (request, sessionId, family) => showControlStudio(context, request, { sessionId, family, sourceLoop: loopRequest, sourceStudy: sessions.engineer?.snapshotStudy() })) : undefined;
  appliedController = undefined;
  manualRecoveryReconciled = false;
  const title = activeLoop?.name ?? text(activeHandoff?.plantId, "Unbound loop");
  if (panel) {
    panel.title = `Control Studio · ${title}`;
    panel.webview.html = controlStudioHtml(
      panel.webview, context, title, activeLoop?.runtimeId ?? activeHandoff?.runtimeId,
      activeHandoff, activeLoop, editorSessionId, currentStageEvidence(manualWorkflowState.model, manualWorkflowState.candidate, manualWorkflowState.validation)
    );
    panel.reveal(vscode.ViewColumn.Active);
    return;
  }
  draftSessions = sessions;
  panel = vscode.window.createWebviewPanel(
    CONTROL_STUDIO_VIEW_TYPE,
    `Control Studio · ${title}`,
    vscode.ViewColumn.Active,
    {
      enableScripts: true,
      retainContextWhenHidden: true,
      localResourceRoots: [vscode.Uri.file(path.join(context.extensionPath, "media")),
        vscode.Uri.file(path.join(context.extensionPath, "src", "webview"))],
    }
  );
  panel.webview.html = controlStudioHtml(
    panel.webview, context, title, activeLoop?.runtimeId ?? activeHandoff?.runtimeId,
    activeHandoff, activeLoop, editorSessionId, currentStageEvidence(manualWorkflowState.model, manualWorkflowState.candidate, manualWorkflowState.validation)
  );
  panel.webview.onDidReceiveMessage(async (message) => {
    if (message?.kind === "control-studio.chart.export") {
      const ownerPanel = panel;
      const ownerEditor = editorSessionId;
      const scope = activeLoop ? loopMessageScope(activeLoop) : activeHandoff ? {
        projectId: activeHandoff.projectId, editorSessionId, plantId: activeHandoff.plantId,
        plantRevision: activeHandoff.savedConfigurationRevision,
      } : undefined;
      if (!scope) return;
      const result = await handleChartExportMessage(message, scope);
      if (panel === ownerPanel && editorSessionId === ownerEditor) await ownerPanel?.webview.postMessage({
        schemaVersion: 1, kind: "control-studio.chart.export.result", projectId: scope.projectId, editorSessionId: ownerEditor,
        ...("loopId" in scope ? { loopId: scope.loopId, expectedLoopRevision: scope.loopRevision }
          : { plantId: scope.plantId, expectedPlantRevision: scope.plantRevision }),
        requestId: message.payload?.requestId, result,
      });
      return;
    }

    const handoff = activeHandoff;
    if (handoff) {
      const expected = {
        projectId: handoff.projectId,
        editorSessionId,
        plantId: handoff.plantId,
        plantRevision: handoff.savedConfigurationRevision,
      };
      if (decodeControlStudioReady(message, expected)) {
        void refreshMonitor();
      } else if (decodeControlStudioNavigatePlant(message, expected)) {
        void vscode.commands.executeCommand("trust-lsp.networkCanvas.open", {
          focusNodeId: `plant:${handoff.runtimeId}:${handoff.plantId}`,
        });
      } else {
        const approval = decodeAutomaticTuneApprove(message, expected);
        if (approval) void completeAutomaticTune(context, handoff, approval.messageId, approval.objective);
      }
    }
    const loop = activeLoop;
    if (loop) {
      // A reused webview keeps its message listener, but owns the newly selected session.
      const sessions = analysisSessions;
      const expected = loopMessageScope(loop);
      const ownerPanel = panel;
      const stillCurrent = () => panel === ownerPanel && activeLoop === loop && editorSessionId === expected.editorSessionId;
      if (decodeLoopLiveViewCommand(message, expected)) { viewingWorkflowResult = false; void refreshLoopMonitor(); return; }
      if (await routeControlStudioRuntimeStart(message, expected, loop.runtimeId, startStudioSimulator,
        async value => { if (stillCurrent()) await ownerPanel?.webview.postMessage(value); })) return;
      if (await routeIdentificationRecordingImport(message, expected, loop,
        Boolean(currentExperimentId || runtimeRecorder.recording || experimentRecorder.recording), async (recording, reference) => {
          if (!stillCurrent()) return;
          const update = sessions.selectIdentificationRecording(recording, reference);
          completedRecording = recording; viewingWorkflowResult = false;
          manualWorkflowState.model = undefined; manualWorkflowState.modelFit = undefined; manualWorkflowState.modelEvidence = undefined;
          manualWorkflowState.tuning = undefined; manualWorkflowState.tuningComparison = undefined; manualWorkflowState.candidate = undefined; manualWorkflowState.validation = undefined;
          manualWorkflowState.excludedIntervals = [];
          await retainCurrentWorkflowDraft(context, loop);
          if (update) await postEngineeringResult(ownerPanel, loop, expected.editorSessionId, "engineer", update);
        }, async value => { if (stillCurrent()) await ownerPanel?.webview.postMessage(value); })) return;
      if (await bindingRoute?.handle(message, payload => postEngineeringResult(ownerPanel, loop, expected.editorSessionId, "bindings", payload))) return;
      if (await storeEngineerStage(context, loop, message, expected)) return;
      if (await sessions.handle(message, expected, async (stage, payload) => {
        if (!stillCurrent()) return;
        if (stage === "engineer" && payload.status === "running") viewingWorkflowResult = false;
        if (stage === "engineer" && payload.status === "reset") {
          manualWorkflowState = createManualWorkflowState(); completedRecording = undefined; currentWorkflowResult = undefined;
          currentExperimentPlan = undefined; currentExperimentTerminal = undefined; viewingWorkflowResult = false;
        }
        if (stage === "general-model" && payload.status === "imported") {
          manualWorkflowState.model = undefined; manualWorkflowState.modelFit = undefined; manualWorkflowState.modelEvidence = undefined;
          manualWorkflowState.candidate = undefined; manualWorkflowState.tuning = undefined; manualWorkflowState.tuningComparison = undefined;
          manualWorkflowState.validation = undefined; manualWorkflowState.declaredAnalysis = undefined;
        }
        if ((stage === "engineer" || stage === "general-model") && payload.status !== "running" && payload.status !== "commissioning") {
          try { await retainCurrentWorkflowDraft(context, loop); }
          catch (error) { if (payload.status === "reset") throw error; void vscode.window.showWarningMessage(`Could not retain the workflow draft: ${String(error)}`); }
        }
        if (payload.status === "reset" && ownerPanel) {
          ownerPanel.webview.html = controlStudioHtml(ownerPanel.webview, context, loop.name, loop.runtimeId,
            activeHandoff, loop, expected.editorSessionId, currentStageEvidence(undefined, undefined, undefined));
          return;
        }
        await ownerPanel?.webview.postMessage({ schemaVersion: 1, kind: "control-studio.engineering.result", messageId: randomUUID(),
          projectId: expected.projectId, editorSessionId: expected.editorSessionId, loopId: expected.loopId,
          expectedLoopRevision: expected.loopRevision, stage, payload });
      }, Boolean(currentExperimentId || runtimeRecorder.recording || experimentRecorder.recording))) return;
      if (!stillCurrent()) return;
      const command = decodeLoopRecordingCommand(message, expected);
      const experimentCommand = decodeControlExperimentCommand(message, expected);
      const antiWindupCommand = decodeAntiWindupCommand(message, expected);
      const applicationCommand = decodeControllerApplicationCommand(message, expected);
      const workflowResultCommand = decodeWorkflowResultCommand(message, expected);
      const mimoCommand = decodeMimoAnalysisCommand(message, expected);
      const automaticCommand = decodeLoopAutomaticTuneApprove(message, expected);
      const engineeringCommand = decodeEngineeringCommand(message, expected);
      if (command?.action === "start" || command?.action === "stop") {
        void handleRuntimeRecording(context, command.action, loop);
      } else if (decodeLoopReady(message, expected)) {
        await restoreRecordingRecoveryOwner(context, loop, runtimeRecorder, active => loopMonitor.setRuntimeRecording(active));
        await refreshExperimentStatus();
        const retainedRecordingState = runtimeRecorder.currentUpdate;
        if (retainedRecordingState) void ownerPanel?.webview.postMessage(
          loopRecordingEvidenceEnvelope(expected, randomUUID(), retainedRecordingState)
        );
        if (workflowResultUnavailableReason) await postWorkflowResult("failed", {
          reason: workflowResultUnavailableReason,
          history: workflowResultHistoryItems(currentWorkflowHistory),
        });
        else if (currentWorkflowResult) await postWorkflowResult("restored", {
          result: currentWorkflowResult,
          draft: currentResultIsDraft,
          history: workflowResultHistoryItems(currentWorkflowHistory),
        });
        else {
          void refreshLoopMonitor();
          await postWorkflowResult("history", { history: workflowResultHistoryItems(currentWorkflowHistory) });
        }
        const restoredDraft = workflowResultUnavailableReason ? undefined : restoreEngineerDraft(context, loop);
        const restoredModel = restoredDraft && !viewingWorkflowResult ? sessions.engineer?.modelChanged() : undefined;
        if (restoredDraft && !viewingWorkflowResult) await postEngineeringResult(panel, activeLoop, editorSessionId, "engineer",
          { ...restoredEngineerView(restoredDraft, sessions.engineer?.recordedCommissioning()), modelAnalysis: restoredModel?.modelAnalysis, task: restoredModel?.task ?? restoredDraft.task, stageEvidence: sessions.engineer?.currentStageEvidence(), ...sessions.engineer?.restoredOwnerView() });
        else if (sessions.engineer?.restoredOwnerView()) await postEngineeringResult(panel, activeLoop, editorSessionId, "engineer",
          { status: "restored", ...sessions.engineer.restoredOwnerView(), stageEvidence: sessions.engineer.currentStageEvidence() });
        if (!manualRecoveryReconciled) {
          const terminal = await reconcileManualApplicationRecovery(context, loop, async manualRecovery => Boolean(await sessions.engineer?.reconcileManualApplicationRecovery(manualRecovery, async payload => {
            if (!stillCurrent()) return;
            await ownerPanel?.webview.postMessage({ schemaVersion: 1, kind: "control-studio.engineering.result", messageId: randomUUID(),
              projectId: expected.projectId, editorSessionId: expected.editorSessionId, loopId: expected.loopId,
              expectedLoopRevision: expected.loopRevision, stage: "engineer", payload: { ...payload, retryManualRecovery: true } });
          })));
          if (terminal) manualRecoveryReconciled = true;
        }
        const application = await observeStudioApplication(loop, sessions.engineer?.recordedCommissioning());
        if (application && stillCurrent()) await postEngineeringResult(ownerPanel, loop, expected.editorSessionId, "engineer", application);
        await bindingRoute?.publish(payload => postEngineeringResult(ownerPanel, loop, expected.editorSessionId, "bindings", payload));
        await ownerPanel?.webview.postMessage({ schemaVersion: 1, kind: "control-studio.navigation.restore", ...expected,
          stage: restoredEngineerStage(context, loop), view: restoredEngineerViewSelection(context, loop) });
        if (currentExperimentId) void refreshMonitor();
      } else if (experimentCommand) {
        void handleExperimentCommand(experimentCommand, loop);
      } else if (antiWindupCommand) {
        void routeAntiWindupPanelCommand({ command: antiWindupCommand, loop, session: antiWindupSession, connectParameters: () => connectRuntimeParameters(loop), connectSearch: () => import("./runtimeSearchConnection").then(({ connectRuntimeSearch }) => connectRuntimeSearch(loop)), post: payload => postExperimentResult(ownerPanel, loop, expected.editorSessionId, "anti-windup", payload) });
      } else if (applicationCommand) {
        void handleControllerApplication(applicationCommand, loop);
      } else if (workflowResultCommand) {
        void handleWorkflowResultCommand(context, workflowResultCommand, loop);
      } else if (mimoCommand && ownerPanel) {
        void handleMimoAnalysisCommand({ context, loop, recording: completedRecording, sessions, command: mimoCommand,
          retainWorkflowDraft: () => retainCurrentWorkflowDraft(context, loop),
          post: payload => postEngineeringResult(ownerPanel, loop, expected.editorSessionId, "mimo", payload) });
      } else if (automaticCommand && loop.plantId) {
        void completeLoopAutomaticTune(context, loop, automaticCommand.messageId, automaticCommand.objective);
      } else if (engineeringCommand) {
        void dispatchManualWorkflowCommand(engineeringCommand, loop, context, ownerPanel, expected.editorSessionId, manualWorkflowState);
      }
    }
  }, undefined, context.subscriptions);
  panel.onDidDispose(() => {
    openRequestGeneration++;
    disposeCurrentAnalysisSessions(context);
    if (refreshTimer) clearTimeout(refreshTimer);
    experimentStatusPoller.dispose();
    refreshTimer = undefined;
    activeHandoff = undefined;
    activeLoop = undefined;
    activeScopeKey = "";
    editorSessionId = "";
    draftSessions = undefined;
    workflowResultUnavailableReason = undefined;
    viewingWorkflowResult = false;
    panel = undefined;
  }, undefined, context.subscriptions);
}

function loopMessageScope(loop: ControlLoopDefinition) {
  return {
    projectId: loop.projectId,
    editorSessionId,
    loopId: loop.id,
    loopRevision: loop.revision,
  };
}


function scheduleMonitorRefresh(): void {
  if (!panel || (!activeHandoff && !activeLoop) || refreshTimer) return;
  refreshTimer = setTimeout(() => {
    refreshTimer = undefined;
    void refreshMonitor();
  }, 250);
}

async function refreshMonitor(): Promise<void> {
  await Promise.all([refreshPlantMonitor(), refreshLoopMonitor(), refreshExperimentStatus()]);
}

async function refreshPlantMonitor(): Promise<void> {
  const currentPanel = panel;
  const handoff = activeHandoff;
  const sessionId = editorSessionId;
  if (!currentPanel || !handoff || !sessionId) return;
  const snapshot = await loadPlantMonitor(handoff, {
    lifecycleSnapshot: () => runtimeLifecycleService.snapshot(),
    authToken: (endpoint) => runtimeLifecycleService.controlAuthToken(endpoint),
    request: (endpoint, authToken, operation, params) =>
      sendRuntimeControlRequest(endpoint, authToken, operation, params, { timeoutMs: 2_000 }),
  });
  if (panel !== currentPanel || activeHandoff !== handoff || editorSessionId !== sessionId) return;
  await currentPanel.webview.postMessage(
    plantMonitorEnvelope(handoff, sessionId, randomUUID(), snapshot)
  );
}

async function refreshLoopMonitor(): Promise<void> {
  const currentPanel = panel;
  const loop = activeLoop;
  const sessionId = editorSessionId;
  if (!currentPanel || !loop || !sessionId || viewingWorkflowResult) return;
  const lifecycle = await runtimeLifecycleService.snapshot();
  const snapshot = await symbolicLoopObservation.observe(loopMonitor, loop, lifecycle, Date.now());
  // Opening saved evidence can race an already-running lifecycle request. Recheck
  // the view mode after the await so stale live data cannot replace that evidence.
  if (panel !== currentPanel || activeLoop !== loop || editorSessionId !== sessionId || viewingWorkflowResult ||
      !symbolicLoopObservation.isCurrentSession(lifecycle.sessionId)) return;
  currentLoopSample = snapshot;
  await currentPanel.webview.postMessage(
    loopMonitorEnvelope(loopMessageScope(loop), randomUUID(), snapshot.availability === "available"
      ? { ...snapshot, runtimeSessionId: lifecycle.sessionId } : snapshot)
  );
}

async function handleExperimentCommand(
  command: ReturnType<typeof decodeControlExperimentCommand> & {},
  loop: ControlLoopDefinition
): Promise<void> {
  if (!panel || !command) return;
  viewingWorkflowResult = false;
  if (command.action === "calculate-relay") {
    await postExperimentResult(panel, activeLoop, editorSessionId, "relay-candidate", {
      relayTuning: relayTuningSession.calculateCandidate(),
    });
    return;
  }
  if (command.action === "review") {
    const ownerPanel = panel;
    await refreshLoopMonitor();
    if (panel !== ownerPanel || activeLoop !== loop) return;
    const result = buildControlExperimentPlan({
      loop,
      sample: currentLoopSample ?? {
        availability: "unavailable",
        source: "Simulation",
        loopId: loop.id,
        reason: "No current loop sample",
      },
      kind: command.kind,
      amplitude: command.amplitude,
      durationSeconds: command.durationSeconds,
      physicalTopologyReviewed: command.physicalTopologyReviewed,
      seed: loop.experiment?.seed,
    });
    if (result.kind === "accepted") {
      relayTuningSession.reset();
      const records = experimentIdentification.records;
      const previous = records[records.length - 1];
      const plan = previous?.outcome === "failed"
        ? experimentIdentification.retry(previous.experimentId, result.plan)
        : result.plan;
      pendingExperiment = plan;
      await postExperimentResult(panel, activeLoop, editorSessionId, "review", { kind: "accepted", plan });
    } else {
      relayTuningSession.reset();
      pendingExperiment = undefined;
      await postExperimentResult(panel, activeLoop, editorSessionId, "review", result);
    }
    return;
  }
  if (command.action === "approve") {
    const plan = pendingExperiment;
    if (!plan || plan.id !== command.planId) {
      await postExperimentResult(panel, activeLoop, editorSessionId, "failed", { reason: "The reviewed plan changed. Review it again before starting." });
      return;
    }
    try {
      if (currentExperimentId || runtimeRecorder.recording || experimentRecorder.recording) {
        throw new Error("Stop the active runtime recording before starting a Test experiment.");
      }
      const result = await startExperimentWithRecording(experimentRecorder, loop,
        () => runtimeExperimentClient().start(plan, command.physicalApproved), undefined,
        owner => storeExperimentRecovery(extensionContext, loop, plan, owner));
      currentExperimentId = plan.id;
      currentExperimentPlan = plan;
      currentExperimentTerminal = undefined;
      loopMonitor.setRuntimeRecording(experimentRecorder.recording);
      pendingExperiment = undefined;
      experimentStatusPoller.schedule();
      await postExperimentResult(panel, activeLoop, editorSessionId, "running", result);
    } catch (error) {
      await postExperimentResult(panel, activeLoop, editorSessionId, "failed", { reason: error instanceof Error ? error.message : String(error) });
    }
    return;
  }
  try {
    const result = await runtimeExperimentClient().abort(command.experimentId);
    const phase = classifyControlExperimentStatus(result);
    if (phase === "invalid" || phase === "running") {
      throw new Error("The runtime returned an invalid abort state.");
    }
    if (phase === "terminal") {
      currentExperimentTerminal = result;
    }
    const payload = phase === "terminal" ? await experimentIdentificationPayload(currentExperimentPlan, result) : result;
    if (phase === "terminal" && !experimentRecorder.recording) {
      currentExperimentId = undefined;
      await clearExperimentRecovery(extensionContext, loop);
    }
    else experimentStatusPoller.schedule();
    await postExperimentResult(panel, activeLoop, editorSessionId, phase === "awaiting_restoration" ? "restoring" : "terminal", payload);
  } catch (error) {
    await postExperimentResult(panel, activeLoop, editorSessionId, "failed", { reason: error instanceof Error ? error.message : String(error) });
  }
}

/** Captures a terminal Test result before Identify can inspect it. */
async function experimentIdentificationPayload(plan: ControlExperimentPlan | undefined, result: unknown): Promise<unknown> {
  try {
    const loop = activeLoop;
    const payload = await buildTerminalTestEvidence({
      loop, plan, result,
      stopRecording: async () => {
        if (!loop) throw new Error("The Test loop is no longer available for runtime recording recovery.");
        const recording = await stopExperimentRecording(experimentRecorder, loop);
        completedRecording = recording;
        return recording;
      },
      recordIdentification: (reviewedPlan, terminal, samples, evidence) =>
        experimentIdentification.record(reviewedPlan, terminal, samples, evidence),
    });
    loopMonitor.setRuntimeRecording(runtimeRecorder.recording || experimentRecorder.recording);
    const augmented = antiWindupSession.augmentTerminalPayload(loop, payload);
    return relayTuningSession.augmentTerminalPayload(plan, augmented);
  } catch (error) {
    loopMonitor.setRuntimeRecording(runtimeRecorder.recording || experimentRecorder.recording);
    const identification_error = error instanceof Error ? error.message : String(error);
    return record(result) ? { ...result, identification_error } : { identification_error };
  }
}


async function refreshExperimentStatus(): Promise<void> {
  await refreshTestExperiment({
    experimentId: () => currentExperimentId,
    loop: () => activeLoop,
    recovery: loop => restoreExperimentRecovery(extensionContext!, loop),
    recordingActive: () => experimentRecorder.recording,
    recoverOwner: (loop, owner) => restoreExperimentRecordingOwner(experimentRecorder, loop, owner, RuntimeRecordingSession.recover),
    recovered: plan => { currentExperimentPlan ??= plan; loopMonitor.setRuntimeRecording(true); },
    status: experimentId => runtimeExperimentClient().status(experimentId),
    classify: classifyControlExperimentStatus,
    terminal: value => experimentIdentificationPayload(currentExperimentPlan, value),
    terminalObserved: value => { currentExperimentTerminal = value; },
    clear: async () => { currentExperimentId = undefined; if (activeLoop) {
      await clearExperimentRecovery(extensionContext, activeLoop);
      if (extensionContext) await clearIdentificationRecording(extensionContext, activeLoop);
    } },
    post: (phase, payload) => postExperimentResult(panel, activeLoop, editorSessionId, phase, payload),
    schedule: () => experimentStatusPoller.schedule(),
  });
  if (!currentExperimentId) await analysisSessions.engineer?.recoverIdentificationMode().catch(error => {
    void vscode.window.showWarningMessage(`Identification recovery remains pending: ${String(error)}`);
  });
}

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

async function completeAutomaticTune(
  context: vscode.ExtensionContext,
  handoff: PlantControlStudioHandoff,
  messageId: string,
  objective: "balanced" | "robust" | "fast"
): Promise<void> {
  const current = panel;
  const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  if (!current || !projectDir) return;
  try {
    const saved = await readProjectPlants(context, projectDir);
    const outcome = runSavedSimulationAutomaticTune(saved, handoff.plantId, objective);
    if (outcome.status === "failed") {
      await current.webview.postMessage(automaticTuneResultEnvelope(
        handoff, editorSessionId, messageId, { status: "failed", error: outcome.reason,
          failedStage: outcome.failedStage, completedStages: outcome.completedStages, recovery: outcome.recovery }
      ));
      return;
    }
    const result = outcome.result;
    manualWorkflowState.candidate = candidateFromControlPid(result.parameters, result.method);
    manualWorkflowState.model = { ...result.model, normalizedError: 0, stable: true };
    manualWorkflowState.modelFit = undefined;
    manualWorkflowState.modelEvidence = "Exact simulation model";
    manualWorkflowState.validation = result.validationResult;
    manualWorkflowState.tuningComparison = {
      objective,
      winnerMethod: result.method,
      ...(result.closedLoopTimeSeconds === undefined ? {} :
        { closedLoopTimeSeconds: result.closedLoopTimeSeconds }),
      attempts: result.attempts.map((attempt) => ({ ...attempt })),
    };
    await current.webview.postMessage(automaticTuneResultEnvelope(
      handoff, editorSessionId, messageId, { status: "complete", result }
    ));
  } catch (error) {
    await current.webview.postMessage(automaticTuneResultEnvelope(
      handoff,
      editorSessionId,
      messageId,
      { status: "failed", error: error instanceof Error ? error.message : String(error) }
    ));
  }
}

async function completeLoopAutomaticTune(
  context: vscode.ExtensionContext, loop: ControlLoopDefinition, messageId: string,
  objective: "balanced" | "robust" | "fast"
): Promise<void> {
  const current = panel; const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  if (!current || !projectDir || !loop.plantId) return;
  try {
    const saved = await readProjectPlants(context, projectDir);
    const runtimeCurrent = await currentRuntimeCandidate(loop, runtimeControllerClient);
    const outcome = runSavedSimulationAutomaticTune(saved, loop.plantId, objective, runtimeCurrent, {
      processUnit: loop.signals.processVariable.unit,
      outputUnit: loop.signals.manipulatedVariable.unit,
    });
    if (outcome.status === "failed") {
      await current.webview.postMessage({ schemaVersion: 1, kind: "control-studio.automatic.result",
        messageId, projectId: loop.projectId, editorSessionId, loopId: loop.id,
        expectedLoopRevision: loop.revision, payload: { status: "failed", error: outcome.reason,
          failedStage: outcome.failedStage, completedStages: outcome.completedStages, recovery: outcome.recovery } });
      return;
    }
    const result = outcome.result;
    manualWorkflowState.candidate = candidateFromControlPid(result.parameters, result.method);
    manualWorkflowState.model = { ...result.model, normalizedError: 0, stable: true };
    manualWorkflowState.modelFit = undefined;
    manualWorkflowState.modelEvidence = "Exact simulation model";
    manualWorkflowState.validation = result.validationResult;
    manualWorkflowState.tuningComparison = {
      objective,
      winnerMethod: result.method,
      ...(result.closedLoopTimeSeconds === undefined ? {} :
        { closedLoopTimeSeconds: result.closedLoopTimeSeconds }),
      attempts: result.attempts.map((attempt) => ({ ...attempt })),
    };
    manualWorkflowState.currentRuntimeController = runtimeCurrent;
    await current.webview.postMessage({ schemaVersion: 1, kind: "control-studio.automatic.result",
      messageId, projectId: loop.projectId, editorSessionId, loopId: loop.id,
      expectedLoopRevision: loop.revision, payload: { status: "complete", result } });
  } catch (error) {
    await current.webview.postMessage({ schemaVersion: 1, kind: "control-studio.automatic.result",
      messageId, projectId: loop.projectId, editorSessionId, loopId: loop.id,
      expectedLoopRevision: loop.revision, payload: { status: "failed", error: error instanceof Error ? error.message : String(error) } });
  }
}

async function handleControllerApplication(
  command: NonNullable<ReturnType<typeof decodeControllerApplicationCommand>>,
  loop: ControlLoopDefinition
): Promise<void> {
  try {
    const client = await runtimeControllerClient();
    if (command.action === "apply") {
      const candidate = manualWorkflowState.candidate;
      if (!candidate) throw new Error("No validated controller candidate is available.");
      if (!manualWorkflowState.validation || manualWorkflowState.validation.requirements.some((requirement) =>
        requirement.status === "Fail" || requirement.status === "Inconclusive")) {
        throw new Error("The current candidate has not passed required validation.");
      }
      const status = await client.status(loop.controller.instance);
      if (!status.confirmed || status.pendingRevision) {
        throw new Error("A controller write is still pending scan confirmation.");
      }
      const sampleRevision = currentLoopSample?.availability === "available"
        ? currentLoopSample.controller.revision : undefined;
      if (!sampleRevision || status.activeRevision !== sampleRevision) {
        throw new Error("The live controller revision changed. Refresh and validate again before applying.");
      }
      const result = await client.apply({ instancePath: loop.controller.instance,
        source: currentLoopSample!.availability === "available" ? currentLoopSample!.source : "Physical",
        expectedRevision: status.activeRevision, candidate: candidate.parameters,
        physicalApproved: command.physicalApproved });
      appliedController = { instancePath: loop.controller.instance, ...result };
      await postApplicationResult(panel, activeLoop, editorSessionId, "applied", { ...result, observationSeconds: 30 });
      return;
    }
    const applied = appliedController;
    if (!applied) throw new Error("No applied candidate is available to roll back.");
    const result = await client.rollback({ instancePath: applied.instancePath,
      expectedActiveRevision: applied.activeRevision, previousRevision: applied.previousRevision });
    appliedController = undefined;
    await postApplicationResult(panel, activeLoop, editorSessionId, "rolled-back", result);
  } catch (error) {
    await postApplicationResult(panel, activeLoop, editorSessionId, "failed", { reason: error instanceof Error ? error.message : String(error) });
  }
}

async function handleWorkflowResultCommand(
  context: vscode.ExtensionContext,
  command: NonNullable<ReturnType<typeof decodeWorkflowResultCommand>>,
  loop: ControlLoopDefinition
): Promise<void> {
  const owner = editorSessionId;
  const postOwned = (phase: string, payload: unknown): Promise<void> =>
    editorSessionId === owner ? postWorkflowResult(phase, payload) : Promise.resolve();
  try {
    if (command.action === "open") {
      const selected = currentWorkflowHistory.find((item) => item.resultRevision === command.resultRevision);
      if (!selected) throw new Error(`Saved result revision ${command.resultRevision} is unavailable.`);
      analysisSessions.engineer?.assertStudyRestoreAllowed(selected.engineerStudy);
      if (!await activateWorkflowResult(selected) || editorSessionId !== owner) return;
      currentWorkflowResult = selected;
      currentResultIsDraft = false;
      viewingWorkflowResult = true;
      await postOwned("opened", { result: selected,
        history: workflowResultHistoryItems(currentWorkflowHistory) });
      return;
    }
    if (command.action === "compare") {
      const left = currentWorkflowHistory.find((item) => item.resultRevision === command.leftRevision);
      const right = currentWorkflowHistory.find((item) => item.resultRevision === command.rightRevision);
      if (!left || !right) throw new Error("One of the selected saved result revisions is unavailable.");
      await postOwned("compared", { comparison: compareWorkflowResultRevisions(left, right),
        history: workflowResultHistoryItems(currentWorkflowHistory) });
      return;
    }
    if (command.action === "save") {
      const result = await buildCurrentWorkflowResult(context, loop, command.session);
      const nextHistory = await saveWorkflowResultRevision(
        context.workspaceState, workflowResultStorageKey(loop), result, 20
      );
      if (editorSessionId !== owner) return;
      currentWorkflowHistory = nextHistory;
      currentWorkflowResult = result;
      currentResultIsDraft = false;
      await postOwned("saved", { createdAt: result.createdAt,
        resultRevision: result.resultRevision, history: workflowResultHistoryItems(currentWorkflowHistory) });
      return;
    }
    const result = await workflowResultForExport(
      currentWorkflowResult,
      () => buildCurrentWorkflowResult(context, loop), viewingWorkflowResult
    );
    if (command.action === "export-csv") {
      if (result.recording.rawSamples.length === 0) throw new Error("No raw recording is available to export.");
      const uri = await vscode.window.showSaveDialog({
        defaultUri: vscode.Uri.file(path.join(vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? "",
          "control-studio-samples.csv")),
        filters: { "CSV samples": ["csv"] },
      });
      if (!uri) return;
      await vscode.workspace.fs.writeFile(uri, new TextEncoder().encode(exportSamplesCsv(result.recording.rawSamples)));
      await postOwned("exported", { format: "CSV", path: uri.fsPath });
      return;
    }
    const uri = await vscode.window.showSaveDialog({
      defaultUri: vscode.Uri.file(path.join(vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? "",
        "control-studio-result.json")),
      filters: { "Control Studio result": ["json"] },
    });
    if (!uri) return;
    await vscode.workspace.fs.writeFile(uri, new TextEncoder().encode(exportWorkflowJson(result)));
    await postOwned("exported", { format: "JSON", path: uri.fsPath });
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    if (editorSessionId !== owner) {
      void vscode.window.showWarningMessage(`Control Studio ${command.action} for ${loop.name} failed: ${reason}`);
      return;
    }
    await postOwned("failed", { reason });
  }
}

async function activateWorkflowResult(result: ControlStudioWorkflowResult, restoreEngineer = true): Promise<boolean> {
  const owner = analysisSessions, ownerEditor = editorSessionId;
  const adopt = await vscode.window.withProgress({ location: vscode.ProgressLocation.Window, title: "Opening saved study" },
    () => owner.prepareIdentificationRestoreAsync(result));
  if (!adopt || owner !== analysisSessions || ownerEditor !== editorSessionId || !adopt()) return false;
  completedRecording = result.recording.acquisition ? {
    samples: [], rawSamples: result.recording.rawSamples, evidence: result.recording.acquisition, provenance: result.recording.provenance,
  } : undefined;
  analysisSessions.model.restore(result.generalModelAnalysis);
  analysisSessions.routh?.restore(result.routhAnalysis);
  analysisSessions.model.restorePeriodic(result.periodicFrequencyAnalysis);
  analysisSessions.leadLag.restore(result.leadLagAnalysis);
  if (result.engineerStudy?.selectedModel) analysisSessions.model.selectModel(result.engineerStudy.selectedModel);
  if (restoreEngineer) {
    analysisSessions.engineer?.restoreStudy(result.engineerStudy);
    analysisSessions.engineer?.restoreCommissioning(result.commissioning);
  }
  analysisSessions.validation.restore(result.runValidation);
  analysisSessions.search?.restore(result.finiteSearch);
  analysisSessions.parameters?.dispose();
  restoreManualWorkflowState(manualWorkflowState, result);
  if (activeLoop && extensionContext) {
    const mimo = restoreMimoAnalysis(extensionContext, activeLoop);
    analysisSessions.restoreMimo(completedRecording, mimo);
  }
  return true;
}

function disposeCurrentAnalysisSessions(context: vscode.ExtensionContext): void {
  try {
    if (activeLoop) void retainCurrentWorkflowDraft(context, activeLoop)
      .catch(error => vscode.window.showWarningMessage(`Could not retain the workflow draft: ${String(error)}`));
    if (activeLoop && (preserveUnresolvedRecovery(context, activeLoop, analysisSessions, currentWorkflowSnapshotState)
      || preservePendingManualApplication(context, activeLoop, analysisSessions)
      || preservePendingIdentification(activeLoop, analysisSessions))) return;
  } catch (error) { void vscode.window.showWarningMessage(`Could not capture Control Studio recovery: ${String(error)}`); }
  analysisSessions.dispose();
}

function buildCurrentWorkflowResult(context: vscode.ExtensionContext, loop: ControlLoopDefinition,
  chartSession?: ControlStudioWorkflowResult["chartSession"]): Promise<ControlStudioWorkflowResult> {
  const revision = Math.max(0, ...currentWorkflowHistory.map(result => result.resultRevision)) + 1;
  return buildWorkflowResultSnapshot(context, loop, currentWorkflowSnapshotState(), chartSession)
    .then(result => ({ ...result, resultRevision: revision }));
}

function retainCurrentWorkflowDraft(context: vscode.ExtensionContext, loop: ControlLoopDefinition): Promise<void> {
  if (draftSessions !== analysisSessions) return Promise.resolve();
  if (workflowResultUnavailableReason) return Promise.reject(new Error(workflowResultUnavailableReason));
  const owner = panel, ownerSession = editorSessionId;
  const study = analysisSessions.engineer?.snapshotDraft() ?? { campaigns: [] };
  const workflow = buildWorkflowDraftSnapshot(loop, currentWorkflowSnapshotState(study));
  workflow.software.extensionVersion = text(context.extension.packageJSON.version, "Unavailable");
  return storeEngineerDraft(context, loop, study, workflow).then(async () => {
    if (!owner || panel !== owner || editorSessionId !== ownerSession || activeLoop !== loop) return;
    currentWorkflowResult = restoreWorkflowDraft(context, loop) ?? workflow;
    currentResultIsDraft = true;
    if (viewingWorkflowResult) await postWorkflowResult("draft", {});
  });
}

function currentWorkflowSnapshotState(engineerStudy = analysisSessions.engineer?.snapshotStudy()): WorkflowResultSnapshotState {
  return {
    previousResult: currentWorkflowResult, recording: completedRecording, excludedIntervals: manualWorkflowState.excludedIntervals,
    identificationStudy: analysisSessions.identification.snapshot(),
    source: currentLoopSample?.availability === "available" ? currentLoopSample.source : currentWorkflowResult?.source ?? "Simulation",
    experiment: workflowExperimentContext(currentExperimentPlan, currentExperimentTerminal),
    engineerStudy, generalModelAnalysis: analysisSessions.model.snapshot(), periodicFrequencyAnalysis: analysisSessions.model.periodicSnapshot(), leadLagAnalysis: analysisSessions.leadLag.snapshot(),
    runValidation: analysisSessions.validation.snapshot(), finiteSearch: analysisSessions.search?.snapshot(), routhAnalysis: analysisSessions.routh?.snapshot(),
    commissioning: analysisSessions.engineer?.recordedCommissioning(), model: manualWorkflowState.model, modelEvidence: manualWorkflowState.modelEvidence,
    declaredAnalysis: manualWorkflowState.declaredAnalysis, modelFit: manualWorkflowState.modelFit, currentController: manualWorkflowState.currentRuntimeController,
    candidate: manualWorkflowState.candidate, validation: manualWorkflowState.validation, tuningComparison: manualWorkflowState.tuningComparison,
  };
}

async function postWorkflowResult(phase: string, payload: unknown): Promise<void> {
  const current = panel; const loop = activeLoop;
  if (!current || !loop) return;
  await current.webview.postMessage({ schemaVersion: 1, kind: "control-studio.workflow-result",
    messageId: randomUUID(), projectId: loop.projectId, editorSessionId, loopId: loop.id,
    expectedLoopRevision: loop.revision, phase, payload });
  if ((phase === "restored" || phase === "opened") && record(payload) && record(payload.result)) {
    const result = payload.result as unknown as ControlStudioWorkflowResult;
    await postRestoredEngineeringPlots(result, (stage, payload) => postEngineeringResult(panel, activeLoop, editorSessionId, stage, payload));
    if (activeLoop && extensionContext) {
      const mimo = restoreMimoAnalysis(extensionContext, activeLoop);
      if (analysisSessions.restoreMimo(completedRecording, mimo) && mimo) {
        await postEngineeringResult(panel, activeLoop, editorSessionId, "mimo", { status: "complete", result: mimo.result,
          recordingId: mimo.recordingId, recordingHash: mimo.recordingHash, trials: mimo.trials, ...(mimo.options ? { options: mimo.options } : {}) });
      }
    }
    const identification = identificationPresentationState(analysisSessions.identification, loop.signals.manipulatedVariable.unit, loop.signals.processVariable.unit);
    if (identification) await postEngineeringResult(panel, activeLoop, editorSessionId, "identify", identification);
  }
}

async function dispatchManualWorkflowCommand(
  command: NonNullable<ReturnType<typeof decodeEngineeringCommand>>,
  loop: ControlLoopDefinition,
  context: vscode.ExtensionContext,
  ownerPanel: vscode.WebviewPanel | undefined,
  ownerEditor: string,
  ownerState: ManualWorkflowState,
): Promise<void> {
  if (!ownerPanel) return;
  const ownerSessions = analysisSessions;
  const ownerRecording = completedRecording;
  const ownerSavedResult = currentWorkflowResult;
  const ownerLoopSample = currentLoopSample;
  const ownerTestLedger = experimentIdentification;
  await handleManualWorkflowCommand({
    command,
    loop,
    state: ownerState,
    sessions: ownerSessions,
    selectedRecording: ownerRecording,
    currentSavedResult: ownerSavedResult,
    currentLoopSample: ownerLoopSample,
    testLedger: ownerTestLedger,
    runtimeCandidate: currentRuntimeCandidate,
    runtimeClient: runtimeControllerClient,
    retainDraft: () => analysisSessions === ownerSessions && manualWorkflowState === ownerState
      ? retainCurrentWorkflowDraft(context, loop) : Promise.resolve(),
    post: (stage, payload) => panel === ownerPanel && activeLoop === loop && editorSessionId === ownerEditor
      ? postEngineeringResult(ownerPanel, loop, ownerEditor, stage, payload) : Promise.resolve(),
  });
}

export async function runtimeControllerClient(): Promise<ControllerApplicationClient> {
  const lifecycle = await runtimeLifecycleService.snapshot();
  if (!lifecycle.status.running) throw new Error("The runtime is not running.");
  const endpoint = lifecycle.status.endpoint;
  const token = await runtimeLifecycleService.controlAuthToken(endpoint);
  return new ControllerApplicationClient((operation, params) =>
    sendRuntimeControlRequest(endpoint, token, operation, params, { timeoutMs: 2_000 }));
}

async function handleRuntimeRecording(context: vscode.ExtensionContext, action: "start" | "stop", loop: ControlLoopDefinition): Promise<void> {
  if (experimentRecorder.recording) {
    void vscode.window.showErrorMessage("The active Test owns runtime acquisition. Wait for its terminal recording before using Manual Record.");
    return;
  }
  if (action === "stop") await retryRecordingRecoveryOwner(context, loop, runtimeRecorder,
    active => loopMonitor.setRuntimeRecording(active));
  await executeRecordingCommand(runtimeRecorder, action, loop, {
    recordingChanged: value => loopMonitor.setRuntimeRecording(value || experimentRecorder.recording),
    started: () => { completedRecording = undefined; viewingWorkflowResult = false; },
    stopped: recording => { completedRecording = recording; },
    persistOwner: owner => storeRecordingRecovery(context, loop, owner),
    clearOwner: () => clearRecordingRecovery(context, loop),
    publish: async evidence => { await panel?.webview.postMessage(loopRecordingEvidenceEnvelope(loopMessageScope(loop), randomUUID(), evidence, "state" in evidence ? undefined : completedRecording)); },
    refresh: () => { void refreshLoopMonitor(); },
  });
}
