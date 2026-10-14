import { connectedIdentificationPanelPorts, clearIdentificationRecording, journalledIdentificationExperimentId } from "./connectedIdentificationPanel";
import { routeIdentificationRecordingImport } from "./identificationRecordingImport";
import { decodeLoopReady, decodeControlStudioHostCommand, decodeManualPreviewRequest, type ControlStudioHostCommand } from "./messages";
import { ThreePageBindingRoute, adoptStudioBinding, adoptStudioFamily, type StudioBindingOpen } from "./threePageBindingRoute";
import type { ControllerFamily } from "./controllerFamilies";
import { createControlStudioRuntimeStartHandler, routeControlStudioRuntimeStart } from "./runtimeStartAction";
import { restoredEngineerStage, restoredEngineerViewSelection, storeEngineerStage } from "./engineerNavigationStorage";
import { completedCommissioningMatchesCheckpoint } from "./commissioningCheckpoint";
import { storeEngineerDraft, restoreEngineerDraft, restoreWorkflowDraft } from "./engineerDraftStorage";
import { CoalescedTask } from "./coalescedTask";
import { observeStudioApplication } from "./observedStudioApplication";
import { restoredEngineerView } from "./restoredEngineerView";
import { restoreCommissioningCheckpoint, storeCommissioningCheckpoint, restoreIsolatedOperation, storeIsolatedOperation, restoreMimoAnalysis } from "./panelPersistence";
import { preserveUnresolvedRecovery, preservePendingManualApplication, preservePendingIdentification, deferRecoveryReopen } from "./unresolvedRecoveryPersistence";
import { buildWorkflowResultSnapshot, buildWorkflowDraftSnapshot, type WorkflowResultSnapshotState } from "./workflowResultSnapshot";
import { SymbolicLoopObservation } from "./symbolicLoopObservation";
import { StudioAnalysisSessions } from "./studioAnalysisSessions";
import { retainedPeriodicRecordingSource } from "./periodicRecordingSource";
import { postRestoredEngineeringPlots } from "./workflowSnapshots";
import { sameRecordingScope } from "./recordingCoordinator";
import { RuntimeRecordingSession } from "./runtimeRecording";
import { latestRequestGate } from "./engineerActions/manualPreview";
import { ExperimentRecordingOwner, experimentRecordingWindowMs, restoreExperimentRecordingOwner, startExperimentWithRecording, stopExperimentRecording } from "./experimentRecording";
import { ExperimentStatusPoller } from "./experimentStatusPoller";
import { refreshTestExperiment } from "./testExperimentLifecycle";
import { buildTerminalTestEvidence } from "./testTerminalEvidence";
import { onDidReadSimulationSpeed, targetLabelUpdates } from "../simulationSpeed";
import { runtimeTargetFromStatus, simulationTargetLabel } from "./runtimeTarget";
import { currentRuntimeCandidate, postEngineeringResult, postExperimentResult, resolveStudyRuntimeTarget, runtimeExperimentClient, type StudyRuntimeTarget } from "./panelWorkflowSupport";
import { identificationPresentationState } from "./identificationPresentationState";
import { randomUUID } from "crypto";
import * as path from "path";
import * as vscode from "vscode";
import { configureControlStudioCommand, openControlStudioCommand, registerControlStudioCommands } from "./openCommand";
import { completeLoopRequest,completePlantHandoff,isLoopRequest,text } from "./panelBindings";
import {
clearExperimentRecovery,controlStudioScopeKey,
reconcileManualApplicationRecovery,
storePendingManualApplicationRecovery,
restoreExperimentRecovery,
restoreWorkflowResultHistory,
storeExperimentRecovery,
workflowResultStorageKey
} from "./panelPersistence";
import { controlStudioHtml } from "./panelPresentation";
import { connectRuntimeParameters } from "./runtimeParameterConnection";
import { appliedProjectHoldSeconds } from "./projectExperimentHold";
import type { PlantControlStudioHandoff } from "./handoff";
import type { ControlLoopDefinition } from "./loopDefinition";
import { openWorkspaceStudy } from "./studyMigration";
import { projectRootHoldingLoop } from "./studyStore";
import { experimentRecordingJournal, journalBeforeDispatch, testStartPorts, UNPINNED_RUNTIME_MESSAGE, UnconfirmedTestStarts } from "./testExperimentGuards";

import { sendRuntimeControlRequest } from "../runtimeControlClient";
import { runtimeLifecycleService } from "../runtimeLifecycle";
import { saveControlStudioChartExport, type ChartExportHost, type ChartExportScope } from "./chartExportHost";
import { ControllerApplicationClient } from "./controllerApplication";
import {
buildControlExperimentPlan,
classifyControlExperimentStatus,
ExperimentIdentificationLedger,
type ControlExperimentPlan,
} from "./experiment";
import { ControlLoopMonitor,controlLoopStaleAfterMs,type LoopRecording } from "./loopMonitor";
import {
decodeControlExperimentCommand,
decodeEngineeringCommand,
decodeMimoAnalysisCommand,
loopMonitorEnvelope,loopRecordingEvidenceEnvelope,
plantMonitorEnvelope,
} from "./messages";
import { loadPlantMonitor } from "./plantMonitor";
import {
exportWorkflowJson,
workflowExperimentContext,
workflowResultForExport,
workflowResultHistoryItems,
type ControlStudioWorkflowResult,
} from "./workflowResult";
import { createManualWorkflowState, handleManualWorkflowCommand, restoreManualWorkflowState, type ManualWorkflowState } from "./manualWorkflowCommands";
import { handleMimoAnalysisCommand } from "./mimoPanelActions";
import { controlStudioEngine, controlStudioStorageFolder, createEnginePanelRoute, type EnginePanelRoute } from "./engine/enginePanelRoute";
import { studyRuntimeConnection } from "./engine/engineConnection";
import { createEngineClient } from "./engine/engineClient";
import { getBinaryPath } from "../binary";
import { CONNECTION_LOSS_TIMEOUT_MS, RECORDING_SAMPLE_LIMIT } from "./controlStudioLimits";

export const CONTROL_STUDIO_OPEN_COMMAND = "trust-lsp.controlStudio.open";
const startStudioSimulator = createControlStudioRuntimeStartHandler(runtimeLifecycleService);

const CONTROL_STUDIO_VIEW_TYPE = "trust-control-studio";

let panel: vscode.WebviewPanel | undefined;
let extensionContext: vscode.ExtensionContext | undefined;
let activeHandoff: PlantControlStudioHandoff | undefined;
let activeLoop: ControlLoopDefinition | undefined;
/** The runtime the open loop is pinned to (A2.7). Resolved when the loop opens; a selection change never moves it. */
let activeRuntimeTarget: StudyRuntimeTarget | undefined;
/** The latest status of an identification reattached on reopening; the page gets it again once it reports ready (A8.6). */
let reattachedIdentificationStatus: Record<string, unknown> | undefined;
let editorSessionId = "";
let bindingDiscovery: import("./configureLoop").DiscoverControlSignals | undefined;
let bindingRoute: ThreePageBindingRoute | undefined;
let openRequestGeneration = 0;
let draftSessions: StudioAnalysisSessions | undefined;
/** The engine's side of the open loop: the loops the engine runs are opened and paged through it (V6 S3, §0.1a). */
let engineRoute: EnginePanelRoute | undefined;
/** The controller family the binding chose for the loop being opened: the TypeScript session takes it when the engine does not serve the loop (B1.3). */
let bindingFamilyToAdopt: ControllerFamily | undefined;
let refreshTimer: NodeJS.Timeout | undefined;
const experimentStatusPoller = new ExperimentStatusPoller(() => { void refreshExperimentStatus(); });
let symbolicLoopObservation = new SymbolicLoopObservation();
let loopMonitor = new ControlLoopMonitor({ staleAfterMs: CONNECTION_LOSS_TIMEOUT_MS, recordingCapacity: RECORDING_SAMPLE_LIMIT });
let completedRecording: LoopRecording | undefined;
const experimentRecorder = new ExperimentRecordingOwner(RuntimeRecordingSession.start);
let currentLoopSample: ReturnType<ControlLoopMonitor["observe"]> | undefined;
let pendingExperiment: ControlExperimentPlan | undefined;
let currentExperimentId: string | undefined;
let currentExperimentPlan: ControlExperimentPlan | undefined;
let currentExperimentTerminal: unknown;
let experimentIdentification = new ExperimentIdentificationLedger();
/** The target label the page shows for the pinned runtime, and the speed-read subscription that keeps it current (B9.2). */
let targetLabelSubscription: vscode.Disposable | undefined;
/** The pinned runtime's label as last read, for every render of the page (specification 35, B9.2). */
let currentTargetLabel: string | undefined;
/** The Test identity whose start is landing: journalled, and not yet answered or failed (A8.5). */
let landingTestStart: string | undefined;
let manualWorkflowState: ManualWorkflowState = createManualWorkflowState();
let analysisSessions = new StudioAnalysisSessions();
let currentWorkflowResult: ControlStudioWorkflowResult | undefined;
let currentResultIsDraft = false;
let currentWorkflowHistory: ControlStudioWorkflowResult[] = [];
let workflowResultUnavailableReason: string | undefined;
/** The latest manual-preview request; an answer for an earlier one is dropped (B4.5). */
const manualPreviewRequests = latestRequestGate();
let viewingWorkflowResult = false;
let activeScopeKey = "";
let manualRecoveryReconciled = false;
/** Test identities whose start outcome is unknown; they are reconciled by status and never sent again (A8.5). */
const unconfirmedTestStarts = new UnconfirmedTestStarts();

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
      await exportWorkflowResultJson(context, activeLoop);
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
  if (experimentRecorder.loop && !sameRecordingScope(experimentRecorder.loop, loopRequest)) {
    void vscode.window.showErrorMessage("Stop the active recording before opening another loop.");
    const loop = experimentRecorder.loop;
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
    // The caller releases the runtime's buffer once this returns, so the samples are stored in the study first (E4.7, E11.6).
    await retainCurrentWorkflowDraft(context, loopRequest);
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
    staleAfterMs: activeLoop ? controlLoopStaleAfterMs(activeLoop) : CONNECTION_LOSS_TIMEOUT_MS,
    recordingCapacity: RECORDING_SAMPLE_LIMIT,
  });
  completedRecording = undefined;
  experimentIdentification = new ExperimentIdentificationLedger();
  loopMonitor.setRuntimeRecording(experimentRecorder.recording);
  currentLoopSample = undefined;
  pendingExperiment = undefined;
  // An identification campaign journals its experiment as well; reopening leaves that experiment to the identification's own
  // recovery, which follows it on the Identify page (A8.5, A8.6), and adopts only a Test experiment here.
  const restored = restoreExperimentRecovery(context, activeLoop);
  const recoveredExperiment = restored && activeLoop && restored.experimentId === journalledIdentificationExperimentId(context, activeLoop)
    ? undefined : restored;
  currentExperimentId = recoveredExperiment?.experimentId;
  currentExperimentPlan = recoveredExperiment?.plan;
  currentExperimentTerminal = undefined;
  manualWorkflowState = createManualWorkflowState();
  // A8.3 and A8.6: the loop's study is read from its project folder, and opening it starts nothing.
  const runtimeTarget = loopRequest ? await resolveStudyRuntimeTarget(loopRequest).catch(() => undefined) : undefined;
  const studyRoot = loopRequest ? await projectRootHoldingLoop(loopRequest, vscode.workspace.workspaceFolders ?? []) : undefined;
  if (analysisSessions !== sessions || openTicket !== openRequestGeneration) return;
  bindingFamilyToAdopt = undefined;
  engineRoute = loopRequest && studyRoot ? createEnginePanelRoute({
    engine: controlStudioEngine(context),
    projectRoot: studyRoot,
    storageFolder: controlStudioStorageFolder(),
    post: message => panel?.webview.postMessage(message),
    isCurrent: () => analysisSessions === sessions,
    connection: () => loopRequest ? studyRuntimeConnection(loopRequest) : Promise.resolve(undefined),
    // A copy: the engine's request type is an open record, which an interface value does not satisfy.
    acquisitionSettings: () => {
      const settings = sessions.engineer?.snapshotStudy()?.acquisitionSettings;
      return settings && { ...settings };
    },
    // The pins and the controller choice that the session holds, which a manual preview is answered with (B4.2, B4.5).
    // snapshotStudy refuses while the session is busy; the preview is then answered without them.
    tuningInputs: () => {
      try {
        const study = sessions.engineer?.snapshotStudy();
        return study && { settingsPins: study.settingsPins, controllerChoice: study.controllerChoice };
      } catch {
        return undefined;
      }
    },
  }) : undefined;
  activeRuntimeTarget = runtimeTarget;
  const initialTargetLabel = await readTargetLabel(runtimeTarget);
  if (analysisSessions !== sessions || openTicket !== openRequestGeneration) return;
  pinTargetLabel(initialTargetLabel);
  let studyOpenFailure: string | undefined;
  if (loopRequest) {
    try {
      if (!studyRoot) throw new Error("This loop is not in an open project folder, so its study cannot be stored in the project.");
      await openWorkspaceStudy(context.workspaceState, studyRoot, loopRequest);
    } catch (error) {
      studyOpenFailure = `The study could not be opened. Its stored files were preserved: ${String(error)}`;
    }
  }
  if (analysisSessions !== sessions || openTicket !== openRequestGeneration) return;
  const restoredWorkflow = restoreWorkflowResultHistory(activeLoop);
  currentWorkflowHistory = restoredWorkflow.history;
  workflowResultUnavailableReason = studyOpenFailure ?? restoredWorkflow.unavailableReason;
  currentWorkflowResult = activeLoop
    ? [...currentWorkflowHistory].reverse().find((result) => result.loop.revision === activeLoop?.revision)
    : undefined;
  currentResultIsDraft = false;
  try {
    const workflowDraft = activeLoop && restoreWorkflowDraft(activeLoop);
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
    const draft = workflowResultUnavailableReason ? undefined : restoreEngineerDraft(loopRequest) ?? currentWorkflowResult?.engineerStudy;
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
  // The engine route takes the family when it opens the loop; a loop the engine does not serve takes it in the TypeScript session.
  if (loopRequest && bindingOpen) await adoptStudioBinding(context, loopRequest, sessions, loopMessageScope(loopRequest), bindingOpen, {
    bindingFamily: family => { bindingFamilyToAdopt = family; engineRoute?.bindingFamily(family); },
  });
  bindingRoute = loopRequest ? new ThreePageBindingRoute(loopRequest, loopMessageScope(loopRequest), bindingDiscovery,
    () => analysisSessions === sessions && activeLoop === loopRequest,
    () => { if (currentExperimentId || experimentRecorder.recording) throw new Error("Finish or abort the current experiment before changing its binding."); sessions.engineer?.assertModelReplacementAllowed(); },
    (request, sessionId, family) => showControlStudio(context, request, { sessionId, family, sourceLoop: loopRequest, sourceStudy: sessions.engineer?.snapshotStudy() }),
    // The engine discovers and selects the bindings of a loop in a project (§0.1a `setupDiscover`); the compiled project is read
    // through the language tool.
    studyRoot ? { engine: createEngineClient(controlStudioEngine(context)), projectRoot: studyRoot, languageTool: getBinaryPath(context, "trust-lsp", "server.path") } : undefined) : undefined;
  manualRecoveryReconciled = false;
  const title = activeLoop?.name ?? text(activeHandoff?.plantId, "Unbound loop");
  if (panel) {
    panel.title = `Control Studio · ${title}`;
    panel.webview.html = controlStudioHtml(panel.webview, context, activeHandoff, activeLoop, editorSessionId, currentTargetLabel);
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
  panel.webview.html = controlStudioHtml(panel.webview, context, activeHandoff, activeLoop, editorSessionId, currentTargetLabel);
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

    const loop = activeLoop;
    if (loop) {
      // A reused webview keeps its message listener, but owns the newly selected session.
      const sessions = analysisSessions;
      const expected = loopMessageScope(loop);
      const route = engineRoute;
      const ownerPanel = panel;
      const stillCurrent = () => panel === ownerPanel && activeLoop === loop && editorSessionId === expected.editorSessionId;
      if (await routeControlStudioRuntimeStart(message, expected, loop.runtimeId, startStudioSimulator,
        async value => { if (stillCurrent()) await ownerPanel?.webview.postMessage(value); })) return;
      if (await routeIdentificationRecordingImport(message, expected, loop,
        Boolean(currentExperimentId || experimentRecorder.recording), async (recording, reference) => {
          if (!stillCurrent()) return;
          const update = sessions.selectIdentificationRecording(recording, reference);
          completedRecording = recording; viewingWorkflowResult = false;
          manualWorkflowState.model = undefined; manualWorkflowState.modelFit = undefined; manualWorkflowState.modelEvidence = undefined;
          manualWorkflowState.tuning = undefined; manualWorkflowState.tuningComparison = undefined; manualWorkflowState.candidate = undefined;
          manualWorkflowState.excludedIntervals = [];
          await retainCurrentWorkflowDraft(context, loop);
          if (update) await postEngineeringResult(ownerPanel, loop, expected.editorSessionId, "engineer", update);
        }, async value => { if (stillCurrent()) await ownerPanel?.webview.postMessage(value); })) return;
      const hostCommand = decodeControlStudioHostCommand(message, expected);
      if (hostCommand) { await runControlStudioHostCommand(hostCommand); return; }
      if (await route?.preview(message, expected)) return;
      const preview = decodeManualPreviewRequest(message, expected);
      if (preview && sessions.engineer) {
        const serial = manualPreviewRequests.begin();
        const payload = await sessions.engineer.previewManualSettings(preview.settings);
        // Only the latest request is answered; a preview superseded while it was computed is dropped.
        if (manualPreviewRequests.isLatest(serial) && stillCurrent()) await postEngineeringResult(ownerPanel, loop, expected.editorSessionId, "tune", payload);
        return;
      }
      if (await bindingRoute?.handle(message, payload => postEngineeringResult(ownerPanel, loop, expected.editorSessionId, "bindings", payload))) return;
      if (await route?.navigation(message, expected)) return;
      if (await route?.identify(message, expected)) return;
      if (await route?.tune(message, expected)) return;
      if (await route?.handle(message, expected)) return;
      if (await storeEngineerStage(loop, message, expected)) return;
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
          manualWorkflowState.declaredAnalysis = undefined;
        }
        if ((stage === "engineer" || stage === "general-model") && payload.status !== "running" && payload.status !== "commissioning") {
          if (stage === "engineer" && payload.status === "edited") {
            // Settings edits arrive in bursts; validating and storing the whole study for each one blocks the editor.
            // Closing the panel retains the latest draft, so a pending retention cannot lose an edit.
            draftRetention.schedule(() => void retainCurrentWorkflowDraft(context, loop)
              .catch(error => vscode.window.showWarningMessage(`Could not retain the workflow draft: ${String(error)}`)));
          } else {
            draftRetention.cancel();
            try { await retainCurrentWorkflowDraft(context, loop); }
            catch (error) { if (payload.status === "reset") throw error; void vscode.window.showWarningMessage(`Could not retain the workflow draft: ${String(error)}`); }
          }
        }
        if (payload.status === "reset" && ownerPanel) {
          ownerPanel.webview.html = controlStudioHtml(ownerPanel.webview, context, activeHandoff, loop, expected.editorSessionId, currentTargetLabel);
          return;
        }
        await ownerPanel?.webview.postMessage({ schemaVersion: 1, kind: "control-studio.engineering.result", messageId: randomUUID(),
          projectId: expected.projectId, editorSessionId: expected.editorSessionId, loopId: expected.loopId,
          expectedLoopRevision: expected.loopRevision, stage, payload });
      }, Boolean(currentExperimentId || experimentRecorder.recording))) return;
      if (!stillCurrent()) return;
      const experimentCommand = decodeControlExperimentCommand(message, expected);
      const mimoCommand = decodeMimoAnalysisCommand(message, expected);
      const engineeringCommand = decodeEngineeringCommand(message, expected);
      if (decodeLoopReady(message, expected)) {
        await refreshExperimentStatus();
        if (reattachedIdentificationStatus) await postEngineeringResult(panel, activeLoop, editorSessionId, "flow", reattachedIdentificationStatus);
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
        const restoredDraft = workflowResultUnavailableReason ? undefined : restoreEngineerDraft(loop);
        const restoredModel = restoredDraft && !viewingWorkflowResult ? sessions.engineer?.modelChanged() : undefined;
        if (restoredDraft && !viewingWorkflowResult) await postEngineeringResult(panel, activeLoop, editorSessionId, "engineer",
          { ...restoredEngineerView(restoredDraft, sessions.engineer?.recordedCommissioning()), modelAnalysis: restoredModel?.modelAnalysis, task: restoredModel?.task ?? restoredDraft.task, stageEvidence: sessions.engineer?.currentStageEvidence(), relayCycle: sessions.engineer?.relayCycle(), ...sessions.engineer?.restoredOwnerView() });
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
        // A8.6 and B6.3: an Apply observation that was running when VS Code closed is reattached and completed. It can last
        // as long as the test duration, so reopening does not wait for it; its progress and result arrive as they come.
        void sessions.engineer?.recoverApplyObservation(async payload => {
          if (stillCurrent()) await postEngineeringResult(ownerPanel, loop, expected.editorSessionId, "engineer", payload);
        }).catch(error => {
          void vscode.window.showWarningMessage(`The Apply observation could not be reattached: ${String(error)}`);
        });
        await bindingRoute?.publish(payload => postEngineeringResult(ownerPanel, loop, expected.editorSessionId, "bindings", payload));
        // The engine's open answer names the page of an engine-served loop; the TypeScript view restore is then not sent.
        const engineServes = route ? await route.open(loop, expected) : false;
        // A loop the engine does not serve keeps the controller family its binding chose in the TypeScript session (B1.3).
        const boundFamily = bindingFamilyToAdopt;
        bindingFamilyToAdopt = undefined;
        if (!engineServes && boundFamily) await adoptStudioFamily(sessions, expected, boundFamily);
        if (!engineServes) await ownerPanel?.webview.postMessage({ schemaVersion: 1, kind: "control-studio.navigation.restore", ...expected,
          stage: restoredEngineerStage(loop), view: restoredEngineerViewSelection(loop) });
        if (currentExperimentId) void refreshMonitor();
      } else if (experimentCommand) {
        void handleExperimentCommand(experimentCommand, loop);
      } else if (mimoCommand && ownerPanel) {
        void handleMimoAnalysisCommand({ context, loop, recording: completedRecording, sessions, command: mimoCommand,
          retainWorkflowDraft: () => retainCurrentWorkflowDraft(context, loop),
          post: payload => postEngineeringResult(ownerPanel, loop, expected.editorSessionId, "mimo", payload) });
      } else if (engineeringCommand) {
        void dispatchManualWorkflowCommand(engineeringCommand, loop, context, ownerPanel, expected.editorSessionId, manualWorkflowState);
      }
    }
  }, undefined, context.subscriptions);
  panel.onDidDispose(() => {
    openRequestGeneration++;
    reattachedIdentificationStatus = undefined;
    disposeCurrentAnalysisSessions(context);
    if (refreshTimer) clearTimeout(refreshTimer);
    experimentStatusPoller.dispose();
    targetLabelSubscription?.dispose();
    targetLabelSubscription = undefined;
    refreshTimer = undefined;
    activeHandoff = undefined;
    activeLoop = undefined;
    activeRuntimeTarget = undefined;
    activeScopeKey = "";
    editorSessionId = "";
    draftSessions = undefined;
    workflowResultUnavailableReason = undefined;
    viewingWorkflowResult = false;
    panel = undefined;
  }, undefined, context.subscriptions);
}

/**
 * The page's host commands (A11.2) run the commands truST already has. Attaching to a runtime that is already running has
 * no command of its own yet, so connecting opens Devices & Connections, where that is done.
 */
async function runControlStudioHostCommand(command: ControlStudioHostCommand): Promise<void> {
  if (command === "connect-runtime") await vscode.commands.executeCommand("trust-lsp.networkCanvas.open");
  else await vscode.commands.executeCommand("trust-lsp.controlStudio.configure");
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
  if (command.action === "review") {
    const ownerPanel = panel;
    await refreshLoopMonitor();
    if (panel !== ownerPanel || activeLoop !== loop) return;
    let holdSeconds: number | undefined;
    try {
      holdSeconds = await appliedProjectHoldSeconds(loop, () => connectRuntimeParameters(loop));
    } catch (error) {
      if (panel !== ownerPanel || activeLoop !== loop) return;
      pendingExperiment = undefined;
      await postExperimentResult(panel, activeLoop, editorSessionId, "review",
        { kind: "rejected", reason: error instanceof Error ? error.message : String(error) });
      return;
    }
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
      ...(holdSeconds === undefined ? {} : { holdSeconds }),
    });
    if (result.kind === "accepted") {
      const records = experimentIdentification.records;
      const previous = records[records.length - 1];
      const plan = previous?.outcome === "failed"
        ? experimentIdentification.retry(previous.experimentId, result.plan)
        : result.plan;
      if (unconfirmedTestStarts.isUnconfirmed(plan.id)) {
        // A start whose outcome is unknown is never sent again under its identity (A8.5).
        pendingExperiment = undefined;
        await postExperimentResult(panel, activeLoop, editorSessionId, "failed", { reason: "This Test identity has a start that the runtime has not confirmed. Its status is reconciled; it is not reviewed or sent again." });
        return;
      }
      pendingExperiment = plan;
      await postExperimentResult(panel, activeLoop, editorSessionId, "review", { kind: "accepted", plan });
    } else {
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
      unconfirmedTestStarts.assertSendable(plan.id);
    } catch (error) {
      await postExperimentResult(panel, activeLoop, editorSessionId, "failed", { reason: error instanceof Error ? error.message : String(error) });
      return;
    }
    const dispatch = { started: false };
    try {
      if (currentExperimentId || experimentRecorder.recording) {
        throw new Error("Stop the active runtime recording before starting a Test experiment.");
      }
      if (landingTestStart !== undefined) throw new Error("Another Test start is still landing. Its outcome is reconciled by status first.");
      if (!extensionContext) throw new Error("The Test start cannot be journalled without Control Studio's workspace state.");
      const experimentContext = extensionContext;
      // A2.7 and A8.5: the start carries its pinned target and the Test journal, or nothing is sent.
      const ports = testStartPorts(await pinnedRuntime(), experimentRecordingJournal({
        persist: owner => storeExperimentRecovery(experimentContext, loop, plan, owner),
        clearOwner: () => storeExperimentRecovery(experimentContext, loop, plan.id),
      }));
      const client = runtimeExperimentClient(ports.target);
      // A8.5: the identity is journalled before the recording or the experiment is dispatched.
      // The identity is journalled from here until the start is answered or has failed; status reads meanwhile keep it.
      landingTestStart = plan.id;
      const result = await journalBeforeDispatch(
        () => storeExperimentRecovery(experimentContext, loop, plan.id),
        () => startExperimentWithRecording(experimentRecorder, loop,
          () => { dispatch.started = true; return client.start(plan, command.physicalApproved); }, ports.journal,
          owner => storeExperimentRecovery(experimentContext, loop, plan, owner),
          experimentRecordingWindowMs(plan.durationMs)))
        .finally(() => { landingTestStart = undefined; });
      currentExperimentId = plan.id;
      currentExperimentPlan = plan;
      currentExperimentTerminal = undefined;
      loopMonitor.setRuntimeRecording(experimentRecorder.recording);
      pendingExperiment = undefined;
      experimentStatusPoller.schedule();
      await postExperimentResult(panel, activeLoop, editorSessionId, "running", result);
    } catch (error) {
      // A failed start keeps no reviewed plan to send again (A8.5). Before dispatch nothing reached the experiment, so its
      // journal entry is retired. After dispatch the outcome is unknown: the identity is reconciled by status, never replayed.
      pendingExperiment = undefined;
      const reason = error instanceof Error ? error.message : String(error);
      if (!dispatch.started) {
        if (extensionContext) await clearExperimentRecovery(extensionContext, loop);
        await postExperimentResult(panel, activeLoop, editorSessionId, "failed", { reason });
        return;
      }
      unconfirmedTestStarts.markUnconfirmed(plan.id);
      const reconciled = await reconcileStartByStatus(plan).catch(() => undefined);
      if (reconciled) {
        await postExperimentResult(panel, activeLoop, editorSessionId, reconciled.phase, reconciled.value);
        return;
      }
      await postExperimentResult(panel, activeLoop, editorSessionId, "failed", { reason: `${reason} The runtime has not confirmed this start. Its identity is kept for status reconciliation and is not sent again.` });
    }
    return;
  }
  try {
    const result = await runtimeExperimentClient(requirePinnedRuntime()).abort(command.experimentId);
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
        const context = extensionContext;
        if (!context) throw new Error("The Test recording cannot be kept without Control Studio's workspace state.");
        // The study keeps the samples before the runtime buffer is released (E4.7, E11.6). A failed keep leaves the buffer for the next Stop.
        return stopExperimentRecording(experimentRecorder, loop, async kept => {
          completedRecording = kept;
          await retainCurrentWorkflowDraft(context, loop);
        });
      },
      recordIdentification: (reviewedPlan, terminal, samples, evidence) =>
        experimentIdentification.record(reviewedPlan, terminal, samples, evidence),
    });
    loopMonitor.setRuntimeRecording(experimentRecorder.recording);
    return payload;
  } catch (error) {
    loopMonitor.setRuntimeRecording(experimentRecorder.recording);
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
    startLanding: experimentId => landingTestStart === experimentId,
    recoverOwner: (loop, owner) => restoreExperimentRecordingOwner(experimentRecorder, loop, owner, RuntimeRecordingSession.recover),
    recovered: plan => { currentExperimentPlan ??= plan; loopMonitor.setRuntimeRecording(true); },
    status: experimentId => runtimeExperimentClient(requirePinnedRuntime()).status(experimentId),
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
  if (!currentExperimentId) void reattachIdentification();
}

/**
 * A8.5, A8.6, A10.4: reopening reattaches an interrupted identification and completes it as it would have completed (follow the
 * held step to its settled end, return the mode, fit). That completion belongs to the journaled operation; reopening starts nothing
 * new and never aborts a running experiment. It runs beside the page, so the reopened page never waits for it to end, and the page
 * shows it as running, with the campaign's progress, until it ends. Its reports go only to the panel that reattached it.
 */
async function reattachIdentification(): Promise<void> {
  const owner = panel, ownerLoop = activeLoop, ownerSession = editorSessionId, ownerSessions = analysisSessions;
  const current = () => panel === owner && analysisSessions === ownerSessions;
  const follow = async (payload: Record<string, unknown>) => {
    if (current()) await postEngineeringResult(owner, ownerLoop, ownerSession, "flow", payload);
  };
  let following = false;
  // The latest progress this reattachment showed; a page that reopens meanwhile is sent it once it is ready.
  let shown: Record<string, unknown> | undefined;
  const recovered = await ownerSessions.engineer?.recoverIdentificationMode(async reason => {
    following = true;
    shown = { flowStage: "Identify", status: "running", reason };
    if (current()) reattachedIdentificationStatus = shown;
    await follow(shown);
  }).catch(async error => {
    if (current()) void vscode.window.showWarningMessage(`Identification recovery remains pending: ${String(error)}`);
    if (following) await follow({ flowStage: "Identify", status: "paused", reason: `Identification recovery remains pending: ${String(error)}` });
    following = false;
    return undefined;
  }).finally(() => { if (reattachedIdentificationStatus === shown) reattachedIdentificationStatus = undefined; });
  // A resumed campaign that fitted a model shows that model as a finished Identify does.
  if (recovered && current()) await postEngineeringResult(owner, ownerLoop, ownerSession, "engineer", recovered);
  if (following) await follow({ flowStage: "Identify", status: "completed", state: "completed", reason: recovered ? "Model ready for review." : "" });
}

/**
 * The runtime the study is pinned to (A2.7): the runtime connected when the loop opened, or, when none was, the one connected at
 * the first live action. Once pinned it never changes. Without a connected runtime the action explains that one must be
 * connected and offers to connect it.
 */
async function pinnedRuntime(): Promise<StudyRuntimeTarget> {
  if (!activeRuntimeTarget && activeLoop) {
    activeRuntimeTarget = await resolveStudyRuntimeTarget(activeLoop);
    pinTargetLabel(await readTargetLabel(activeRuntimeTarget));
  }
  return requirePinnedRuntime();
}

/** The runtime the study is pinned to, once a live action has pinned it (A2.7). A live Test request never picks up another runtime. */
function requirePinnedRuntime(): StudyRuntimeTarget {
  if (!activeRuntimeTarget) throw new Error(UNPINNED_RUNTIME_MESSAGE);
  return activeRuntimeTarget;
}

/** Reconciles a start whose outcome is unknown by its identity (A8.5). The experiment is attached only when the runtime reports it; nothing is sent. */
async function reconcileStartByStatus(plan: ControlExperimentPlan):
  Promise<{ phase: "running" | "restoring" | "terminal"; value: unknown } | undefined> {
  const value = await runtimeExperimentClient(requirePinnedRuntime()).status(plan.id);
  const runtimePhase = classifyControlExperimentStatus(value);
  if (runtimePhase !== "running" && runtimePhase !== "awaiting_restoration" && runtimePhase !== "terminal") return undefined;
  unconfirmedTestStarts.confirm(plan.id);
  currentExperimentId = plan.id;
  currentExperimentPlan = plan;
  currentExperimentTerminal = runtimePhase === "terminal" ? value : undefined;
  loopMonitor.setRuntimeRecording(experimentRecorder.recording);
  experimentStatusPoller.schedule();
  return { phase: runtimePhase === "awaiting_restoration" ? "restoring" : runtimePhase, value };
}

/** The pinned runtime's target label from its status report (specification 35, B9.2); undefined when that status cannot be read. */
async function readTargetLabel(target: StudyRuntimeTarget | undefined): Promise<string | undefined> {
  if (!target) return undefined;
  try {
    const authToken = await runtimeLifecycleService.controlAuthToken(target.endpoint);
    const report = await sendRuntimeControlRequest(target.endpoint, authToken, "status", {}, { timeoutMs: 750 });
    return simulationTargetLabel(runtimeTargetFromStatus(report));
  } catch {
    return undefined;
  }
}

/** Keeps the page's target label current for the pinned runtime. The subscription ends when the loop changes or the panel closes. */
function pinTargetLabel(initial: string | undefined): void {
  targetLabelSubscription?.dispose();
  currentTargetLabel = initial;
  const target = activeRuntimeTarget;
  const loop = activeLoop;
  targetLabelSubscription = target && loop
    ? onDidReadSimulationSpeed(targetLabelUpdates(target.runtimeId, initial, label => {
      if (activeLoop === loop && activeRuntimeTarget === target) { currentTargetLabel = label; void postTargetLabel(loop, label); }
    }))
    : undefined;
}

/** Posts the target label in the envelope the other host-to-page messages use; the page checks that envelope's scope. */
async function postTargetLabel(loop: ControlLoopDefinition, label: string): Promise<void> {
  await panel?.webview.postMessage({ schemaVersion: 1, kind: "control-studio.target-label", messageId: randomUUID(),
    projectId: loop.projectId, editorSessionId, loopId: loop.id, expectedLoopRevision: loop.revision, label });
}

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

/** Exports the current result as JSON; the Control Studio export command is its only caller. */
async function exportWorkflowResultJson(context: vscode.ExtensionContext, loop: ControlLoopDefinition): Promise<void> {
  const owner = editorSessionId;
  const postOwned = (phase: string, payload: unknown): Promise<void> =>
    editorSessionId === owner ? postWorkflowResult(phase, payload) : Promise.resolve();
  try {
    const result = await workflowResultForExport(
      currentWorkflowResult,
      () => buildCurrentWorkflowResult(context, loop), viewingWorkflowResult
    );
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
      void vscode.window.showWarningMessage(`Control Studio export-json for ${loop.name} failed: ${reason}`);
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
    const mimo = restoreMimoAnalysis(activeLoop);
    analysisSessions.restoreMimo(completedRecording, mimo);
  }
  return true;
}

function disposeCurrentAnalysisSessions(context: vscode.ExtensionContext): void {
  engineRoute?.dispose();
  engineRoute = undefined;
  draftRetention.cancel();
  try {
    if (activeLoop) void retainCurrentWorkflowDraft(context, activeLoop)
      .catch(error => vscode.window.showWarningMessage(`Could not retain the workflow draft: ${String(error)}`));
    // Closing during an Apply observation detaches it: the runtime keeps recording, and reopening reattaches at once (A8.5, A10.4).
    if (activeLoop && analysisSessions.engineer?.detachObservationForClose()) { analysisSessions.dispose(); return; }
    if (activeLoop && (preserveUnresolvedRecovery(context, activeLoop, analysisSessions, currentWorkflowSnapshotState)
      || preservePendingManualApplication(context, activeLoop, analysisSessions)
      || preservePendingIdentification(activeLoop, analysisSessions))) return;
  } catch (error) { void vscode.window.showWarningMessage(`Could not capture Control Studio recovery: ${String(error)}`); }
  analysisSessions.dispose();
}

function buildCurrentWorkflowResult(context: vscode.ExtensionContext, loop: ControlLoopDefinition): Promise<ControlStudioWorkflowResult> {
  const revision = Math.max(0, ...currentWorkflowHistory.map(result => result.resultRevision)) + 1;
  return buildWorkflowResultSnapshot(context, loop, currentWorkflowSnapshotState())
    .then(result => ({ ...result, resultRevision: revision }));
}

/** Coalesces the draft retention of settings edits; results and resets are retained at once. */
const draftRetention = new CoalescedTask(750);

function retainCurrentWorkflowDraft(context: vscode.ExtensionContext, loop: ControlLoopDefinition): Promise<void> {
  if (draftSessions !== analysisSessions) return Promise.resolve();
  if (workflowResultUnavailableReason) return Promise.reject(new Error(workflowResultUnavailableReason));
  const owner = panel, ownerSession = editorSessionId;
  const study = analysisSessions.engineer?.snapshotDraft() ?? { campaigns: [] };
  const workflow = buildWorkflowDraftSnapshot(loop, currentWorkflowSnapshotState(study));
  workflow.software.extensionVersion = text(context.extension.packageJSON.version, "Unavailable");
  return storeEngineerDraft(loop, study, workflow).then(async stored => {
    if (!owner || panel !== owner || editorSessionId !== ownerSession || activeLoop !== loop) return;
    currentWorkflowResult = stored ?? workflow;
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
    candidate: manualWorkflowState.candidate, tuningComparison: manualWorkflowState.tuningComparison,
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
      const mimo = restoreMimoAnalysis(activeLoop);
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
