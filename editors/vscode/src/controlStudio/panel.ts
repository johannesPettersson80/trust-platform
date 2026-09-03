import * as vscode from "vscode";
import * as path from "path";
import { randomUUID } from "crypto";

import type { PlantControlStudioHandoff } from "./handoff";
import type { ControlLoopDefinition, ControlLoopScope } from "./loopDefinition";
import { sharedPidDesignAvailability, validateControlLoopDefinition } from "./loopDefinition";
import { ControlLoopMonitor, controlLoopStaleAfterMs, type AvailableLoopSample } from "./loopMonitor";
import {
  buildControlExperimentPlan,
  classifyControlExperimentStatus,
  controlExperimentChoices,
  controlExperimentDefaults,
  ControlExperimentClient,
  decodeControlExperimentRecovery,
  type ControlExperimentPlan,
} from "./experiment";
import { runtimeLifecycleService } from "../runtimeLifecycle";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import {
  automaticTuneResultEnvelope,
  decodeAutomaticTuneApprove,
  decodeControllerApplicationCommand,
  decodeWorkflowResultCommand,
  decodeLoopAutomaticTuneApprove,
  decodeEngineeringCommand,
  decodeControlStudioNavigatePlant,
  decodeControlStudioReady,
  decodeControlExperimentCommand,
  decodeLoopRecordingCommand,
  loopMonitorEnvelope,
  plantMonitorEnvelope,
} from "./messages";
import { loadPlantMonitor } from "./plantMonitor";
import { readProjectPlants } from "../networkCanvas/canvasDataService";
import { runSavedSimulationAutomaticTune } from "./automaticTune";
import { ControllerApplicationClient } from "./controllerApplication";
import {
  IdentificationWorkflow,
  identifiedModelPresentation,
  type ExcludedInterval,
  type IdentificationModelSelection,
  type IdentificationSample,
  type IdentifiedModel,
  type ModelFitEvidence,
} from "./identification";
import { describeTuningMethod, tuneController, type TuningResult } from "./tuning";
import {
  compareControlPidCandidates,
  validationRequirementRows,
  type ValidationResult,
} from "./validation";
import {
  candidateFromControlPid,
  type ControlPidCandidate,
  type ControlPidParameters,
} from "./controllerCandidate";
import { buildEngineeringPlotEvidence, engineeringPlotMessageEvidence } from "./engineeringPlots";
import {
  createWorkflowResult,
  decodeWorkflowResultHistoryState,
  saveWorkflowResultRevision,
  workflowResultHistoryItems,
  compareWorkflowResultRevisions,
  excludedIntervalsFromSelection,
  workflowExperimentContext,
  workflowSimulationPlantContext,
  workflowResultForExport,
  exportSamplesCsv,
  exportWorkflowJson,
  type ControlStudioWorkflowResult,
  type WorkflowTuningComparison,
  type WorkflowResultSample,
} from "./workflowResult";
import { escapeHtml, renderControlStudioHtml, type AutomaticTuneButtonView } from "./panelTemplate";
import { controlStudioAutomaticTuneAvailability, controlStudioStageAvailability } from "./stageAvailability";
import { formatEngineeringNumber } from "./chartContract";

export const CONTROL_STUDIO_OPEN_COMMAND = "trust-lsp.controlStudio.open";
const CONTROL_STUDIO_VIEW_TYPE = "trust-control-studio";

let panel: vscode.WebviewPanel | undefined;
let extensionContext: vscode.ExtensionContext | undefined;
let activeHandoff: PlantControlStudioHandoff | undefined;
let activeLoop: ControlLoopDefinition | undefined;
let editorSessionId = "";
let refreshTimer: NodeJS.Timeout | undefined;
let loopMonitor = new ControlLoopMonitor({ staleAfterMs: 1_500, recordingCapacity: 12_000 });
let completedRecording: AvailableLoopSample[] = [];
let currentLoopSample: ReturnType<ControlLoopMonitor["observe"]> | undefined;
let pendingExperiment: ControlExperimentPlan | undefined;
let currentExperimentId: string | undefined;
let currentExperimentPlan: ControlExperimentPlan | undefined;
let currentExperimentTerminal: unknown;
let currentCandidate: ControlPidCandidate | undefined;
let currentModel: IdentifiedModel | undefined;
let currentModelFit: ModelFitEvidence | undefined;
let currentModelEvidence: ControlStudioWorkflowResult["evidence"]["model"] | undefined;
let identificationWorkflow = new IdentificationWorkflow();
let currentExcludedIntervals: ExcludedInterval[] = [];
let currentTuning: TuningResult | undefined;
let currentTuningComparison: WorkflowTuningComparison | undefined;
let currentValidation: ValidationResult | undefined;
let currentRuntimeController: ControlPidCandidate | undefined;
let currentWorkflowResult: ControlStudioWorkflowResult | undefined;
let currentWorkflowHistory: ControlStudioWorkflowResult[] = [];
let workflowResultUnavailableReason: string | undefined;
let viewingWorkflowResult = false;
let appliedController: { instancePath: string; previousRevision: string; activeRevision: string } | undefined;
let activeScopeKey = "";

interface ControlStudioLoopOpenRequest {
  schemaVersion: 1;
  source: "ConfiguredLoop" | "PidInstance";
  scope: ControlLoopScope;
  loopDefinition: unknown;
}

export function registerControlStudioPanel(context: vscode.ExtensionContext): void {
  extensionContext = context;
  context.subscriptions.push(
    vscode.commands.registerCommand(
      CONTROL_STUDIO_OPEN_COMMAND,
      (request?: unknown) => showControlStudio(context, request)
    )
  );
  context.subscriptions.push(
    runtimeLifecycleService.onDidChange(() => scheduleMonitorRefresh())
  );
}

function showControlStudio(
  context: vscode.ExtensionContext,
  request?: unknown
): void {
  const loopRequest = completeLoopRequest(request);
  const handoff = completePlantHandoff(request);
  if (isLoopRequest(request) && !loopRequest) {
    void vscode.window.showErrorMessage("Control Studio could not open: the loop binding is invalid or outside the requested runtime.");
    return;
  }
  const nextScopeKey = controlStudioScopeKey(loopRequest, handoff);
  if (panel && nextScopeKey === activeScopeKey) {
    panel.reveal(vscode.ViewColumn.Active);
    scheduleMonitorRefresh();
    return;
  }
  activeLoop = loopRequest;
  activeHandoff = handoff;
  activeScopeKey = nextScopeKey;
  loopMonitor = new ControlLoopMonitor({
    staleAfterMs: activeLoop ? controlLoopStaleAfterMs(activeLoop) : 1_500,
    recordingCapacity: 12_000,
  });
  completedRecording = [];
  currentLoopSample = undefined;
  pendingExperiment = undefined;
  currentExperimentId = restoreExperimentRecovery(context, activeLoop);
  currentExperimentPlan = undefined;
  currentExperimentTerminal = undefined;
  currentCandidate = undefined;
  currentModel = undefined;
  currentModelFit = undefined;
  currentModelEvidence = undefined;
  identificationWorkflow = new IdentificationWorkflow();
  currentExcludedIntervals = [];
  currentTuning = undefined;
  currentTuningComparison = undefined;
  currentValidation = undefined;
  currentRuntimeController = undefined;
  const restoredWorkflow = restoreWorkflowResultHistory(context, activeLoop);
  currentWorkflowHistory = restoredWorkflow.history;
  workflowResultUnavailableReason = restoredWorkflow.unavailableReason;
  currentWorkflowResult = activeLoop
    ? [...currentWorkflowHistory].reverse().find((result) => result.loop.revision === activeLoop?.revision)
    : undefined;
  viewingWorkflowResult = Boolean(currentWorkflowResult);
  if (currentWorkflowResult) activateWorkflowResult(currentWorkflowResult);
  appliedController = undefined;
  const title = activeLoop?.name ?? text(activeHandoff?.plantId, "Unbound loop");
  if (panel) {
    panel.title = `Control Studio · ${title}`;
    panel.webview.html = controlStudioHtml(
      panel.webview, context, title, activeLoop?.runtimeId ?? activeHandoff?.runtimeId,
      activeHandoff, activeLoop, editorSessionId
    );
    panel.reveal(vscode.ViewColumn.Active);
    return;
  }
  editorSessionId = randomUUID();
  panel = vscode.window.createWebviewPanel(
    CONTROL_STUDIO_VIEW_TYPE,
    `Control Studio · ${title}`,
    vscode.ViewColumn.Active,
    {
      enableScripts: true,
      retainContextWhenHidden: true,
      localResourceRoots: [vscode.Uri.file(path.join(context.extensionPath, "media"))],
    }
  );
  panel.webview.html = controlStudioHtml(
    panel.webview, context, title, activeLoop?.runtimeId ?? activeHandoff?.runtimeId,
    activeHandoff, activeLoop, editorSessionId
  );
  panel.webview.onDidReceiveMessage((message) => {
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
      const expected = loopMessageScope(loop);
      const command = decodeLoopRecordingCommand(message, expected);
      const experimentCommand = decodeControlExperimentCommand(message, expected);
      const applicationCommand = decodeControllerApplicationCommand(message, expected);
      const workflowResultCommand = decodeWorkflowResultCommand(message, expected);
      const automaticCommand = decodeLoopAutomaticTuneApprove(message, expected);
      const engineeringCommand = decodeEngineeringCommand(message, expected);
      if (command?.action === "start") {
        viewingWorkflowResult = false;
        loopMonitor.startRecording();
        completedRecording = [];
        void refreshLoopMonitor();
      } else if (command?.action === "stop") {
        completedRecording = loopMonitor.stopRecording();
        void refreshLoopMonitor();
      } else if (isLoopReady(message, expected)) {
        if (workflowResultUnavailableReason) void postWorkflowResult("failed", {
          reason: workflowResultUnavailableReason,
          history: workflowResultHistoryItems(currentWorkflowHistory),
        });
        else if (currentWorkflowResult) void postWorkflowResult("restored", {
          result: currentWorkflowResult,
          history: workflowResultHistoryItems(currentWorkflowHistory),
        });
        else {
          void refreshLoopMonitor();
          void postWorkflowResult("history", { history: workflowResultHistoryItems(currentWorkflowHistory) });
        }
      } else if (experimentCommand) {
        void handleExperimentCommand(experimentCommand, loop);
      } else if (applicationCommand) {
        void handleControllerApplication(applicationCommand, loop);
      } else if (workflowResultCommand) {
        void handleWorkflowResultCommand(context, workflowResultCommand, loop);
      } else if (automaticCommand && loop.plantId) {
        void completeLoopAutomaticTune(context, loop, automaticCommand.messageId, automaticCommand.objective);
      } else if (engineeringCommand) {
        void handleEngineeringCommand(engineeringCommand, loop);
      }
    }
  }, undefined, context.subscriptions);
  panel.onDidDispose(() => {
    if (refreshTimer) clearTimeout(refreshTimer);
    refreshTimer = undefined;
    activeHandoff = undefined;
    activeLoop = undefined;
    activeScopeKey = "";
    editorSessionId = "";
    workflowResultUnavailableReason = undefined;
    viewingWorkflowResult = false;
    panel = undefined;
  }, undefined, context.subscriptions);
}

function controlStudioHtml(
  webview: vscode.Webview,
  context: vscode.ExtensionContext,
  plantId: string,
  runtimeId?: string,
  handoff?: PlantControlStudioHandoff,
  loopDefinition?: ControlLoopDefinition,
  sessionId = ""
): string {
  const plant = escapeHtml(plantId);
  const runtime = escapeHtml(text(runtimeId, "Runtime unavailable"));
  const project = escapeHtml(loopDefinition?.projectId ?? handoff?.projectId ?? "");
  const session = escapeHtml(sessionId);
  const revision = escapeHtml(handoff?.savedConfigurationRevision ?? "");
  const loop = escapeHtml(loopDefinition?.id ?? handoff?.loopId ?? "");
  const loopRevision = escapeHtml(loopDefinition?.revision ?? "");
  const processUnit = escapeHtml(loopDefinition?.signals.processVariable.unit ?? "");
  const outputUnit = escapeHtml(loopDefinition?.signals.manipulatedVariable.unit ?? "");
  const controllerInstance = escapeHtml(loopDefinition?.controller.instance ?? "Unbound");
  const experimentDefaults = loopDefinition ? controlExperimentDefaults(loopDefinition) : {
    execution: "control-studio", kind: "setpoint_step", amplitude: 5, durationSeconds: 30, seed: 1,
  } as const;
  const experimentOptions = controlExperimentChoices()
    .map((choice) => `<option value="${choice.value}"${choice.value === experimentDefaults.kind ? " selected" : ""}>${escapeHtml(choice.label)}</option>`)
    .join("");
  const pidDesign = loopDefinition ? sharedPidDesignAvailability(loopDefinition) : { available: true } as const;
  const automaticTuneDisabled = !pidDesign.available || (!handoff && !loopDefinition?.plantId);
  const tuneDescription = pidDesign.available
    ? "Generate or review an unapplied Control.PID candidate from the accepted model."
    : pidDesign.reason;
  const tuneDisabledReason = pidDesign.available ? "Accept a model before tuning" : pidDesign.reason;
  const tuneStateText = pidDesign.available ? `${tuneDisabledReason}.` : tuneDisabledReason;
  const sourceLabel = handoff?.source === "Simulation" ? "Simulation" : "Waiting";
  const nonce = webviewNonce();
  const chartScript = webview.asWebviewUri(
    vscode.Uri.file(path.join(context.extensionPath, "media", "controlStudioChart.js"))
  ).toString();
  const workflowScript = webview.asWebviewUri(
    vscode.Uri.file(path.join(context.extensionPath, "media", "controlStudioWorkflow.js"))
  ).toString();
  const automaticTuneMode = controlStudioAutomaticTuneAvailability(experimentDefaults.execution);
  const automaticTune: AutomaticTuneButtonView = {
    disabled: automaticTuneDisabled,
    primary: automaticTuneMode.primary,
    title: automaticTuneDisabled
      ? "A saved simulated plant is required before Automatic Tune can build a plan"
      : automaticTuneMode.reason,
  };
  const stageAvailability = controlStudioStageAvailability(experimentDefaults.execution, {
    loopBound: Boolean(loopDefinition),
    hasModel: Boolean(currentModel),
    hasCandidate: Boolean(currentCandidate),
    validationHasResult: Boolean(currentValidation),
    validationPassed: currentValidation
      ? !currentValidation.requirements.some((requirement) =>
          requirement.status === "Fail" || requirement.status === "Inconclusive")
      : undefined,
  });
  return renderControlStudioHtml({
    cspSource: webview.cspSource, nonce, chartScript, workflowScript,
    plant, sourceLabel, runtime, controllerInstance, project, session, revision,
    loop, loopRevision, processUnit, outputUnit, experimentDefaults, experimentOptions,
    pidDesign, tuneDescription, tuneDisabledReason, tuneStateText, automaticTune,
    stageAvailability, loopDefinition, handoff,
  });
}

function text(value: unknown, fallback: string): string {
  return typeof value === "string" && value.trim() ? value.trim() : fallback;
}

function webviewNonce(): string {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let value = "";
  for (let index = 0; index < 24; index += 1) value += alphabet[Math.floor(Math.random() * alphabet.length)];
  return value;
}

function completePlantHandoff(value: unknown): PlantControlStudioHandoff | undefined {
  if (!record(value) || !record(value.requestedContext)) return undefined;
  return value.schemaVersion === 1 &&
    typeof value.handoffId === "string" &&
    typeof value.projectId === "string" &&
    typeof value.runtimeId === "string" &&
    typeof value.plantId === "string" &&
    typeof value.savedConfigurationRevision === "string" &&
    value.source === "Simulation" &&
    value.requestedContext.mode === "Monitor" &&
    value.requestedContext.selection === "plant_summary"
    ? value as unknown as PlantControlStudioHandoff
    : undefined;
}

function isLoopRequest(value: unknown): value is ControlStudioLoopOpenRequest {
  return record(value) && value.schemaVersion === 1 &&
    (value.source === "ConfiguredLoop" || value.source === "PidInstance") &&
    record(value.scope) && "loopDefinition" in value;
}

function completeLoopRequest(value: unknown): ControlLoopDefinition | undefined {
  if (!isLoopRequest(value) || typeof value.scope.projectId !== "string" ||
      typeof value.scope.runtimeId !== "string") return undefined;
  const result = validateControlLoopDefinition(value.loopDefinition, {
    projectId: value.scope.projectId,
    runtimeId: value.scope.runtimeId,
  });
  return result.kind === "accepted" ? result.loop : undefined;
}

function loopMessageScope(loop: ControlLoopDefinition) {
  return {
    projectId: loop.projectId,
    editorSessionId,
    loopId: loop.id,
    loopRevision: loop.revision,
  };
}

function isLoopReady(value: unknown, expected: ReturnType<typeof loopMessageScope>): boolean {
  if (!record(value) || !record(value.payload)) return false;
  return Object.keys(value).sort().join(",") ===
      "editorSessionId,expectedLoopRevision,kind,loopId,messageId,payload,projectId,schemaVersion" &&
    value.schemaVersion === 1 && value.kind === "control-studio.ready" &&
    typeof value.messageId === "string" && value.messageId.length > 0 &&
    value.projectId === expected.projectId && value.editorSessionId === expected.editorSessionId &&
    value.loopId === expected.loopId && value.expectedLoopRevision === expected.loopRevision &&
    Object.keys(value.payload).length === 0;
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
  const snapshot = loopMonitor.observe(loop, await runtimeLifecycleService.snapshot(), Date.now());
  currentLoopSample = snapshot;
  // Opening saved evidence can race an already-running lifecycle request. Recheck
  // the view mode after the await so stale live data cannot replace that evidence.
  if (panel !== currentPanel || activeLoop !== loop || editorSessionId !== sessionId || viewingWorkflowResult) return;
  await currentPanel.webview.postMessage(
    loopMonitorEnvelope(loopMessageScope(loop), randomUUID(), snapshot)
  );
}

async function handleExperimentCommand(
  command: ReturnType<typeof decodeControlExperimentCommand> & {},
  loop: ControlLoopDefinition
): Promise<void> {
  if (!panel || !command) return;
  viewingWorkflowResult = false;
  if (command.action === "review") {
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
    pendingExperiment = result.kind === "accepted" ? result.plan : undefined;
    await postExperiment("review", result);
    return;
  }
  if (command.action === "approve") {
    const plan = pendingExperiment;
    if (!plan || plan.id !== command.planId) {
      await postExperiment("failed", { reason: "The reviewed plan changed. Review it again before starting." });
      return;
    }
    try {
      const result = await runtimeExperimentClient().start(plan, command.physicalApproved);
      currentExperimentId = plan.id;
      currentExperimentPlan = plan;
      currentExperimentTerminal = undefined;
      await storeExperimentRecovery(loop, plan.id);
      pendingExperiment = undefined;
      await postExperiment("running", result);
    } catch (error) {
      await postExperiment("failed", { reason: error instanceof Error ? error.message : String(error) });
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
      currentExperimentId = undefined;
      await clearExperimentRecovery(loop);
    }
    await postExperiment(phase === "awaiting_restoration" ? "restoring" : "terminal", result);
  } catch (error) {
    await postExperiment("failed", { reason: error instanceof Error ? error.message : String(error) });
  }
}

function runtimeExperimentClient(): ControlExperimentClient {
  return new ControlExperimentClient(async (operation, params) => {
    const lifecycle = await runtimeLifecycleService.snapshot();
    if (!lifecycle.status.running) {
      throw new Error("The runtime is not running.");
    }
    // Reachability is a cached UI probe and can lag a newly started debug
    // session. The existing control request is the authoritative transport
    // check and returns the actual authorization or connection failure.
    const token = await runtimeLifecycleService.controlAuthToken(lifecycle.status.endpoint);
    return sendRuntimeControlRequest(
      lifecycle.status.endpoint,
      token,
      operation,
      params,
      { timeoutMs: 2_000 }
    );
  });
}

async function postExperiment(phase: string, payload: unknown): Promise<void> {
  const current = panel;
  const loop = activeLoop;
  if (!current || !loop) return;
  await current.webview.postMessage({
    schemaVersion: 1,
    kind: "control-studio.experiment.result",
    messageId: randomUUID(),
    projectId: loop.projectId,
    editorSessionId,
    loopId: loop.id,
    expectedLoopRevision: loop.revision,
    phase,
    payload,
  });
}

async function refreshExperimentStatus(): Promise<void> {
  const experimentId = currentExperimentId;
  if (!experimentId) return;
  try {
    const result = await runtimeExperimentClient().status(experimentId);
    const phase = classifyControlExperimentStatus(result);
    if (phase === "invalid") throw new Error("The runtime returned an invalid experiment state.");
    if (phase === "terminal") {
      currentExperimentTerminal = result;
      currentExperimentId = undefined;
      if (activeLoop) await clearExperimentRecovery(activeLoop);
    }
    await postExperiment(phase === "awaiting_restoration" ? "restoring" : phase, result);
  } catch (error) {
    await postExperiment("failed", { reason: error instanceof Error ? error.message : String(error) });
  }
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
    currentCandidate = candidateFromControlPid(result.parameters, result.method);
    currentModel = { ...result.model, normalizedError: 0, stable: true };
    currentModelFit = undefined;
    currentModelEvidence = "Exact simulation model";
    currentValidation = result.validationResult;
    currentTuningComparison = {
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
    const runtimeCurrent = await currentRuntimeCandidate(loop);
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
    currentCandidate = candidateFromControlPid(result.parameters, result.method);
    currentModel = { ...result.model, normalizedError: 0, stable: true };
    currentModelFit = undefined;
    currentModelEvidence = "Exact simulation model";
    currentValidation = result.validationResult;
    currentTuningComparison = {
      objective,
      winnerMethod: result.method,
      ...(result.closedLoopTimeSeconds === undefined ? {} :
        { closedLoopTimeSeconds: result.closedLoopTimeSeconds }),
      attempts: result.attempts.map((attempt) => ({ ...attempt })),
    };
    currentRuntimeController = runtimeCurrent;
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
    const client = runtimeControllerClient();
    if (command.action === "apply") {
      const candidate = currentCandidate;
      if (!candidate) throw new Error("No validated controller candidate is available.");
      if (!currentValidation || currentValidation.requirements.some((requirement) =>
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
      await postApplication("applied", { ...result, observationSeconds: 30 });
      return;
    }
    const applied = appliedController;
    if (!applied) throw new Error("No applied candidate is available to roll back.");
    const result = await client.rollback({ instancePath: applied.instancePath,
      expectedActiveRevision: applied.activeRevision, previousRevision: applied.previousRevision });
    appliedController = undefined;
    await postApplication("rolled-back", result);
  } catch (error) {
    await postApplication("failed", { reason: error instanceof Error ? error.message : String(error) });
  }
}

async function handleWorkflowResultCommand(
  context: vscode.ExtensionContext,
  command: NonNullable<ReturnType<typeof decodeWorkflowResultCommand>>,
  loop: ControlLoopDefinition
): Promise<void> {
  try {
    if (command.action === "open") {
      const selected = currentWorkflowHistory.find((item) => item.resultRevision === command.resultRevision);
      if (!selected) throw new Error(`Saved result revision ${command.resultRevision} is unavailable.`);
      currentWorkflowResult = selected;
      viewingWorkflowResult = true;
      activateWorkflowResult(selected);
      await postWorkflowResult("opened", { result: selected,
        history: workflowResultHistoryItems(currentWorkflowHistory) });
      return;
    }
    if (command.action === "compare") {
      const left = currentWorkflowHistory.find((item) => item.resultRevision === command.leftRevision);
      const right = currentWorkflowHistory.find((item) => item.resultRevision === command.rightRevision);
      if (!left || !right) throw new Error("One of the selected saved result revisions is unavailable.");
      await postWorkflowResult("compared", { comparison: compareWorkflowResultRevisions(left, right),
        history: workflowResultHistoryItems(currentWorkflowHistory) });
      return;
    }
    if (command.action === "save") {
      const result = await buildCurrentWorkflowResult(context, loop, command.session);
      const nextHistory = await saveWorkflowResultRevision(
        context.workspaceState, workflowResultStorageKey(loop), result, 20
      );
      currentWorkflowHistory = nextHistory;
      currentWorkflowResult = result;
      await postWorkflowResult("saved", { createdAt: result.createdAt,
        resultRevision: result.resultRevision, history: workflowResultHistoryItems(currentWorkflowHistory) });
      return;
    }
    const result = await workflowResultForExport(
      currentWorkflowResult,
      () => buildCurrentWorkflowResult(context, loop)
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
      await postWorkflowResult("exported", { format: "CSV", path: uri.fsPath });
      return;
    }
    const uri = await vscode.window.showSaveDialog({
      defaultUri: vscode.Uri.file(path.join(vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? "",
        "control-studio-result.json")),
      filters: { "Control Studio result": ["json"] },
    });
    if (!uri) return;
    await vscode.workspace.fs.writeFile(uri, new TextEncoder().encode(exportWorkflowJson(result)));
    await postWorkflowResult("exported", { format: "JSON", path: uri.fsPath });
  } catch (error) {
    await postWorkflowResult("failed", { reason: error instanceof Error ? error.message : String(error) });
  }
}

function activateWorkflowResult(result: ControlStudioWorkflowResult): void {
  currentModel = result.model;
  currentModelFit = result.modelFit;
  currentModelEvidence = result.evidence.model;
  currentCandidate = result.candidate;
  currentValidation = result.validation;
  currentTuningComparison = result.tuningComparison;
  currentRuntimeController = result.currentController;
  const rawSamples = result.recording.rawSamples;
  const endSeconds = rawSamples.length > 0 ? rawSamples[rawSamples.length - 1].elapsedSeconds : 0;
  currentExcludedIntervals = excludedIntervalsFromSelection(endSeconds, result.recording.selectedIntervals);
}

async function buildCurrentWorkflowResult(
  context: vscode.ExtensionContext,
  loop: ControlLoopDefinition,
  chartSession?: ControlStudioWorkflowResult["chartSession"]
): Promise<ControlStudioWorkflowResult> {
  const retained = currentWorkflowResult?.recording.rawSamples ?? [];
  const firstObservedAt = completedRecording[0]?.observedAtMs ?? 0;
  const rawSamples: readonly WorkflowResultSample[] = completedRecording.length > 0
    ? completedRecording.map((sample, index) => ({
      elapsedSeconds: (sample.observedAtMs - firstObservedAt) / 1_000,
      observedAtMs: sample.observedAtMs,
      actualCycleSeconds: index === 0
        ? loop.observedSampleTiming.nominalPeriodMs / 1_000
        : ((sample.observedAtMs - completedRecording[index - 1].observedAtMs) > 0
          ? sample.observedAtMs - completedRecording[index - 1].observedAtMs
          : loop.observedSampleTiming.nominalPeriodMs) / 1_000,
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
      freshnessMs: 0,
      controllerMode: sample.controller.mode,
      outputLimits: { ...sample.outputLimits },
      rateLimits: { ...sample.rateLimits },
      safetyState: "Unavailable",
      events: [
        ...(index === 0 ? ["recording_started"] : []),
        ...(index === completedRecording.length - 1 ? ["recording_stopped"] : []),
      ],
    }))
    : retained;
  const source = currentLoopSample?.availability === "available"
    ? currentLoopSample.source : currentWorkflowResult?.source ?? "Simulation";
  const endSeconds = rawSamples.length > 0 ? rawSamples[rawSamples.length - 1].elapsedSeconds : 0;
  const experiment = workflowExperimentContext(currentExperimentPlan, currentExperimentTerminal);
  const plant = source === "Simulation" && loop.plantId
    ? await loadWorkflowSimulationPlant(context, loop)
    : undefined;
  return createWorkflowResult({
    resultId: currentWorkflowResult?.resultId ?? randomUUID(),
    resultRevision: (currentWorkflowResult?.resultRevision ?? 0) + 1,
    projectId: loop.projectId,
    source,
    loop: { id: loop.id, revision: loop.revision },
    controller: { instance: loop.controller.instance, revision: loop.controller.revision },
    software: {
      extensionVersion: text(context.extension.packageJSON.version, "Unavailable"),
      numericalAdapters: { tuning: "trust-control-tuning-v2" },
    },
    units: { sp: loop.signals.setpoint?.unit ?? "", pv: loop.signals.processVariable.unit,
      mv: loop.signals.manipulatedVariable.unit },
    recording: { rawSamples, selectedIntervals: selectedIntervals(endSeconds, currentExcludedIntervals) },
    ...(currentModel ? { model: currentModel } : {}),
    ...(currentModelFit ? { modelFit: currentModelFit } : {}),
    ...(currentRuntimeController ? { currentController: currentRuntimeController } : {}),
    ...(currentCandidate ? { candidate: currentCandidate } : {}),
    ...(currentValidation ? { validation: currentValidation } : {}),
    ...(currentTuningComparison ? { tuningComparison: currentTuningComparison } : {}),
    ...(currentCandidate?.source ? { method: currentCandidate.source } : {}),
    ...(experiment ? { experiment } : {}),
    ...(plant ? { plant } : {}),
    ...(chartSession ? { chartSession } : {}),
    evidence: { observation: "Observed", model: currentModel
      ? (currentModelEvidence ??
        (source === "Simulation" ? "Identified from simulation data" : "Identified from physical data"))
      : "Not evaluated" },
    conclusion: currentValidation
      ? "Validation result retained for review; no controller change was applied by saving."
      : "Workflow state retained for continued analysis.",
  }, new Date().toISOString());
}

async function loadWorkflowSimulationPlant(
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

function selectedIntervals(endSeconds: number, excluded: readonly ExcludedInterval[]): Array<{ startSeconds: number; endSeconds: number }> {
  if (endSeconds <= 0) return [];
  const ordered = excluded.filter((item) => item.endSeconds >= 0 && item.startSeconds <= endSeconds)
    .map((item) => ({ startSeconds: Math.max(0, item.startSeconds), endSeconds: Math.min(endSeconds, item.endSeconds) }))
    .sort((left, right) => left.startSeconds - right.startSeconds);
  const selected: Array<{ startSeconds: number; endSeconds: number }> = [];
  let cursor = 0;
  for (const item of ordered) {
    if (item.startSeconds > cursor) selected.push({ startSeconds: cursor, endSeconds: item.startSeconds });
    cursor = Math.max(cursor, item.endSeconds);
  }
  if (cursor < endSeconds) selected.push({ startSeconds: cursor, endSeconds });
  return selected;
}

function restoreWorkflowResultHistory(
  context: vscode.ExtensionContext,
  loop: ControlLoopDefinition | undefined
): { history: ControlStudioWorkflowResult[]; unavailableReason?: string } {
  if (!loop) return { history: [] };
  const decoded = decodeWorkflowResultHistoryState(context.workspaceState.get(workflowResultStorageKey(loop)));
  return {
    ...decoded,
    history: decoded.history.filter((result) => result.projectId === loop.projectId && result.loop.id === loop.id),
  };
}

function workflowResultStorageKey(loop: ControlLoopDefinition): string {
  return `trust.controlStudio.result.${loop.projectId}.${loop.id}`;
}

function restoreExperimentRecovery(
  context: vscode.ExtensionContext,
  loop: ControlLoopDefinition | undefined
): string | undefined {
  if (!loop) return undefined;
  return decodeControlExperimentRecovery(
    context.workspaceState.get(experimentRecoveryStorageKey(loop)),
    { projectId: loop.projectId, loopId: loop.id, loopRevision: loop.revision }
  )?.experimentId;
}

async function storeExperimentRecovery(loop: ControlLoopDefinition, experimentId: string): Promise<void> {
  await extensionContext?.workspaceState.update(experimentRecoveryStorageKey(loop), {
    schemaVersion: 1,
    experimentId,
    projectId: loop.projectId,
    loopId: loop.id,
    loopRevision: loop.revision,
  });
}

async function clearExperimentRecovery(loop: ControlLoopDefinition): Promise<void> {
  await extensionContext?.workspaceState.update(experimentRecoveryStorageKey(loop), undefined);
}

function experimentRecoveryStorageKey(loop: ControlLoopDefinition): string {
  return `trust.controlStudio.experiment.${loop.projectId}.${loop.id}`;
}

function controlStudioScopeKey(loop: ControlLoopDefinition | undefined, handoff: PlantControlStudioHandoff | undefined): string {
  if (loop) return `loop:${loop.projectId}:${loop.runtimeId}:${loop.id}`;
  if (handoff) return `plant:${handoff.projectId}:${handoff.runtimeId}:${handoff.plantId}`;
  return "unbound";
}

async function postWorkflowResult(phase: string, payload: unknown): Promise<void> {
  const current = panel; const loop = activeLoop;
  if (!current || !loop) return;
  await current.webview.postMessage({ schemaVersion: 1, kind: "control-studio.workflow-result",
    messageId: randomUUID(), projectId: loop.projectId, editorSessionId, loopId: loop.id,
    expectedLoopRevision: loop.revision, phase, payload });
  if ((phase === "restored" || phase === "opened") && record(payload) && record(payload.result)) {
    const result = payload.result as unknown as ControlStudioWorkflowResult;
    await postRestoredEngineeringPlots(result);
  }
}

async function postRestoredEngineeringPlots(result: ControlStudioWorkflowResult): Promise<void> {
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

async function handleEngineeringCommand(
  command: NonNullable<ReturnType<typeof decodeEngineeringCommand>>,
  loop: ControlLoopDefinition
): Promise<void> {
  try {
    if (command.action === "identify-qualify") {
      const samples = identificationSamples();
      currentExcludedIntervals = [...(command.excludedIntervals ?? [])];
      const source = currentLoopSample?.availability === "available"
        ? currentLoopSample.source : currentWorkflowResult?.source ?? "Simulation";
      const qualification = identificationWorkflow.qualify(samples, currentExcludedIntervals, source,
        command.model as IdentificationModelSelection);
      currentModel = undefined; currentModelFit = undefined; currentModelEvidence = undefined;
      currentTuning = undefined; currentTuningComparison = undefined;
      currentCandidate = undefined; currentValidation = undefined;
      await postEngineering("identify", { status: qualification.suitable ? "complete" : "failed",
        phase: "qualified", qualification, nextFitRole: identificationWorkflow.nextFitRole,
        ...(qualification.suitable ? {} : { reason: qualification.reason }) });
      return;
    }
    if (command.action === "identify-fit") {
      const review = identificationWorkflow.fit();
      const presentation = identifiedModelPresentation(review.selected, review.fit,
        loop.signals.manipulatedVariable.unit, loop.signals.processVariable.unit);
      if (review.stage === "validation_required") {
        await postEngineering("identify", { status: "complete", phase: review.stage,
          source: review.preview.source, model: review.selected, fit: review.fit, presentation,
          estimationQualification: review.estimationQualification });
      } else {
        await postEngineering("identify", { status: "complete", phase: review.stage,
          source: review.result.source, model: review.selected, fit: review.fit, presentation,
          estimationQualification: review.estimationQualification,
          validationQualification: review.validationQualification });
      }
      return;
    }
    if (command.action === "identify-accept") {
      const accepted = identificationWorkflow.accept();
      currentModel = accepted.model;
      currentModelFit = accepted.fit;
      currentModelEvidence = accepted.evidence;
      const presentation = identifiedModelPresentation(accepted.model, accepted.fit,
        loop.signals.manipulatedVariable.unit, loop.signals.processVariable.unit);
      await postEngineering("identify", { status: "complete", phase: "accepted",
        source: accepted.evidence, model: accepted.model, fit: accepted.fit, presentation,
        estimationQualification: accepted.review.estimationQualification,
        validationQualification: accepted.review.validationQualification });
      return;
    }
    if (command.action === "tune-settings") {
      if (!currentModel) throw new Error("Identify and accept a model before tuning.");
      currentCandidate = candidateFromControlPid(command.settings, command.method);
      currentTuning = undefined; currentTuningComparison = undefined; currentValidation = undefined;
      const current = await currentRuntimeCandidate(loop);
      currentRuntimeController = current;
      const plots = buildEngineeringPlotEvidence(currentModel, current, currentCandidate);
      await postEngineering("tune", { status: "complete", candidate: currentCandidate, current,
        plots: engineeringPlotMessageEvidence(plots),
        recommendation: `${command.method} settings retained exactly; the running controller is unchanged.` });
      return;
    }
    if (command.action === "tune") {
      if (!currentModel) throw new Error("Identify and accept a model before tuning.");
      const methodEvidence = describeTuningMethod(currentModel, command.method, command.objective,
        loop.signals.manipulatedVariable.unit, loop.signals.processVariable.unit);
      if (methodEvidence.status === "Unavailable") {
        await postEngineering("tune", { status: "unavailable", reason: methodEvidence.reason, methodEvidence });
        return;
      }
      currentTuning = tuneController(currentModel, { method: command.method, objective: command.objective });
      currentTuningComparison = {
        objective: command.objective,
        winnerMethod: command.method,
        ...(currentTuning.closedLoopTimeSeconds === undefined ? {} :
          { closedLoopTimeSeconds: currentTuning.closedLoopTimeSeconds }),
        attempts: [{ id: command.method.toLowerCase(), method: command.method,
          structure: "PI", status: "Feasible",
          ...(currentTuning.closedLoopTimeSeconds === undefined ? {} :
            { closedLoopTimeSeconds: currentTuning.closedLoopTimeSeconds }) }],
      };
      const tiSeconds = currentTuning.controller.tiSeconds;
      if (tiSeconds === undefined) throw new Error("The selected candidate is not a PI controller.");
      currentCandidate = generatedPiCandidate(currentTuning.controller.kp, tiSeconds, command.method, loop);
      currentValidation = undefined;
      const current = await currentRuntimeCandidate(loop);
      currentRuntimeController = current;
      const plots = buildEngineeringPlotEvidence(currentModel, current, currentCandidate);
      const methodDetail = command.method === "SIMC"
        ? `lambda ${formatEngineeringNumber(currentTuning.closedLoopTimeSeconds)} s`
        : "published AMIGO coefficients; Response affects comparison ranking, not the formula";
      await postEngineering("tune", { status: "complete", result: currentTuning,
        candidate: currentCandidate, current, plots: engineeringPlotMessageEvidence(plots), methodEvidence,
        recommendation: `${command.method} ${currentCandidate.structure} is unapplied · ${methodDetail}; compare the prediction before validation.` });
      return;
    }
    if (!currentModel || !currentCandidate) throw new Error("Generate a candidate before validation.");
    const current = await currentRuntimeCandidate(loop);
    currentRuntimeController = current;
    currentValidation = compareControlPidCandidates(currentModel, current, currentCandidate, {
      evidence: "Model-predicted",
      scenarios: [{ kind: "setpoint", amplitude: 1 }, { kind: "disturbance", amplitude: 0.2 },
        { kind: "noise", amplitude: 0.02 }],
      variationFractions: [-0.2, 0, 0.2],
      processUnit: loop.signals.processVariable.unit,
      outputUnit: loop.signals.manipulatedVariable.unit,
    });
    await postEngineering("validate", { status: "complete", result: {
      ...currentValidation,
      requirements: validationRequirementRows(currentValidation.requirements),
    },
      acceptable: !currentValidation.requirements.some((requirement) =>
        requirement.status === "Fail" || requirement.status === "Inconclusive") });
  } catch (error) {
    await postEngineering(command.action.startsWith("identify") ? "identify" : command.action,
      { status: "failed", reason: error instanceof Error ? error.message : String(error) });
  }
}

function identificationSamples(): IdentificationSample[] {
  const first = completedRecording[0]?.observedAtMs ?? 0;
  const liveSamples = completedRecording.map((sample) => ({
    timeSeconds: (sample.observedAtMs - first) / 1_000,
    input: sample.manipulatedVariable.value,
    output: sample.processVariable.value,
    quality: ([sample.manipulatedVariable, sample.processVariable].some((signal) => signal.quality === "forced")
      ? "bad" : "good") as "bad" | "good",
    inputUnit: sample.manipulatedVariable.unit,
    outputUnit: sample.processVariable.unit,
  }));
  if (liveSamples.length > 0) return liveSamples;
  return currentWorkflowResult?.recording.rawSamples.map((sample) => ({
    timeSeconds: sample.elapsedSeconds,
    input: sample.appliedMv ?? Number.NaN,
    output: sample.pv ?? Number.NaN,
    quality: sample.quality === "good" ? "good" as const : "bad" as const,
    inputUnit: sample.units.mv,
    outputUnit: sample.units.pv,
  })) ?? [];
}

function generatedPiCandidate(
  kp: number,
  tiSeconds: number,
  source: "SIMC" | "AMIGO",
  loop?: ControlLoopDefinition
): ControlPidCandidate {
  const rateUp = loop?.limits.rateUpPerS ?? 1_000_000;
  const rateDown = loop?.limits.rateDownPerS ?? 1_000_000;
  return candidateFromControlPid({
    proportionalGain: kp,
    integralGain: kp / tiSeconds,
    derivativeGain: 0,
    proportionalSetpointWeight: 1,
    derivativeSetpointWeight: 0,
    bias: 0,
    outputMinimum: loop?.limits.outputMinimum ?? 0,
    outputMaximum: loop?.limits.outputMaximum ?? 100,
    rateMinimum: -rateDown,
    rateMaximum: rateUp,
    trackingTime: Math.max(tiSeconds, (loop?.controller.taskPeriodMs ?? 10) / 1_000),
    antiWindup: "BackCalculation",
  }, source);
}

async function currentRuntimeCandidate(loop: ControlLoopDefinition): Promise<ControlPidCandidate> {
  const status = await runtimeControllerClient().status(loop.controller.instance);
  const value = status.parameters as Record<string, unknown>;
  const number = (name: string): number => {
    const result = Number(value[name]);
    if (!Number.isFinite(result)) throw new Error(`Runtime controller parameter ${name} is unavailable.`);
    return result;
  };
  return candidateFromControlPid({
    proportionalGain: number("ProportionalGain"),
    integralGain: number("IntegralGain"),
    derivativeGain: number("DerivativeGain"),
    proportionalSetpointWeight: number("ProportionalSetpointWeight"),
    derivativeSetpointWeight: number("DerivativeSetpointWeight"),
    bias: number("Bias"),
    outputMinimum: number("OutputMinimum"),
    outputMaximum: number("OutputMaximum"),
    rateMinimum: number("RateMinimum"),
    rateMaximum: number("RateMaximum"),
    trackingTime: number("TrackingTime"),
    antiWindup: value.AntiWindup === "Combined" ? "Combined" : "BackCalculation",
  }, "Imported");
}


async function postEngineering(stage: string, payload: unknown): Promise<void> {
  const current = panel; const loop = activeLoop;
  if (!current || !loop) return;
  await current.webview.postMessage({ schemaVersion: 1, kind: "control-studio.engineering.result", messageId: randomUUID(),
    projectId: loop.projectId, editorSessionId, loopId: loop.id, expectedLoopRevision: loop.revision, stage, payload });
}

function runtimeControllerClient(): ControllerApplicationClient {
  return new ControllerApplicationClient(async (operation, params) => {
    const lifecycle = await runtimeLifecycleService.snapshot();
    if (!lifecycle.status.running) throw new Error("The runtime is not running.");
    const token = await runtimeLifecycleService.controlAuthToken(lifecycle.status.endpoint);
    return sendRuntimeControlRequest(lifecycle.status.endpoint, token, operation, params, { timeoutMs: 2_000 });
  });
}

async function postApplication(phase: string, payload: unknown): Promise<void> {
  const current = panel; const loop = activeLoop;
  if (!current || !loop) return;
  await current.webview.postMessage({ schemaVersion: 1, kind: "control-studio.application.result",
    messageId: randomUUID(), projectId: loop.projectId, editorSessionId, loopId: loop.id,
    expectedLoopRevision: loop.revision, phase, payload });
}
