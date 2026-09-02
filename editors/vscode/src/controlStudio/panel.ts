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
  );
  const workflowScript = webview.asWebviewUri(
    vscode.Uri.file(path.join(context.extensionPath, "media", "controlStudioWorkflow.js"))
  );
  return `<!doctype html>
<html lang="en"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src ${webview.cspSource} 'nonce-${nonce}'">
<style>
button:disabled{color:var(--vscode-disabledForeground);border-color:var(--vscode-disabledForeground);opacity:.62;cursor:not-allowed}
.workspace{position:relative}.workspace .auto-plan{inset:0}
:root{color-scheme:light dark}*{box-sizing:border-box;scrollbar-width:thin;scrollbar-color:var(--vscode-scrollbarSlider-background) transparent}*::-webkit-scrollbar{width:8px;height:8px}*::-webkit-scrollbar-thumb{background:var(--vscode-scrollbarSlider-background)}*::-webkit-scrollbar-thumb:hover{background:var(--vscode-scrollbarSlider-hoverBackground)}body{margin:0;background:var(--vscode-editor-background);color:var(--vscode-editor-foreground);font-family:var(--vscode-font-family);font-size:var(--vscode-font-size);height:100vh;overflow:hidden}
.studio{height:100%;display:grid;grid-template-rows:auto auto minmax(420px,52vh) minmax(0,1fr)}
.header{display:flex;align-items:center;gap:16px;padding:12px 18px;border-bottom:1px solid var(--vscode-panel-border);min-width:0}.identity{min-width:0;flex:1}.eyebrow{font-size:11px;color:var(--vscode-descriptionForeground);margin-bottom:2px}.title{font-size:18px;font-weight:600;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.status{display:flex;align-items:center;justify-content:flex-end;flex-wrap:wrap;gap:5px 10px;color:var(--vscode-descriptionForeground);font-size:11px;min-width:0}.status code{font-family:var(--vscode-editor-font-family);color:inherit;overflow-wrap:anywhere}.runtime{display:block;max-width:260px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.source{border:1px solid var(--vscode-badge-background);color:var(--vscode-badge-foreground);background:var(--vscode-badge-background);border-radius:10px;padding:2px 8px;font-weight:600}.automatic-tune{border:1px solid var(--vscode-button-border,transparent);border-radius:2px;background:var(--vscode-button-background);color:var(--vscode-button-foreground);font:inherit;font-weight:600;padding:6px 10px;white-space:nowrap}.automatic-tune:disabled{background:var(--vscode-button-secondaryBackground);color:var(--vscode-disabledForeground);cursor:not-allowed}
.modes{display:flex;gap:2px;padding:0 18px;border-bottom:1px solid var(--vscode-panel-border);overflow-x:auto}.mode{appearance:none;border:0;border-bottom:2px solid transparent;background:transparent;color:var(--vscode-descriptionForeground);font:inherit;font-weight:600;padding:10px 14px;white-space:nowrap}.mode[data-status=incomplete]:after{content:"";display:inline-block;width:5px;height:5px;margin-left:7px;border:1px solid currentColor;border-radius:50%;vertical-align:middle}.mode[aria-selected=true]{color:var(--vscode-foreground);border-bottom-color:var(--vscode-focusBorder)}
.monitor{display:grid;grid-template-columns:minmax(0,1fr) minmax(240px,320px);gap:16px;padding:8px 18px 12px;border-bottom:1px solid var(--vscode-panel-border);min-height:0}.plot-column{display:grid;grid-template-rows:auto auto minmax(0,1fr);min-width:0;min-height:0}.plot-toolbar{grid-row:1;display:flex;align-items:center;justify-content:space-between;gap:8px;min-width:0;border-bottom:1px solid var(--vscode-panel-border)}.plot-tabs,.plot-actions{display:flex;align-items:center;gap:2px;min-width:0}.plot-tabs{overflow-x:auto}.plot-actions{flex:none;border-left:1px solid var(--vscode-panel-border);padding-left:6px}.plot-toolbar button{appearance:none;border:0;border-bottom:2px solid transparent;background:transparent;color:var(--vscode-descriptionForeground);font:inherit;font-size:11px;padding:6px 8px;white-space:nowrap}.plot-toolbar button[aria-selected=true]{color:var(--vscode-foreground);border-bottom-color:var(--vscode-focusBorder)}.plot-toolbar button:focus-visible{outline:1px solid var(--vscode-focusBorder);outline-offset:-2px}.plot-toolbar button:disabled{border-color:transparent}.design-row{grid-row:2;display:flex;align-items:center;gap:8px;min-height:32px;overflow-x:auto;border-bottom:1px solid var(--vscode-panel-border);font-variant-numeric:tabular-nums}.design-row[hidden]{display:none}.design-chip{display:flex;align-items:center;gap:5px;padding:2px 6px;border-left:2px solid var(--vscode-charts-blue);white-space:nowrap;font-size:11px}.design-metrics{display:flex;gap:10px;white-space:nowrap;color:var(--vscode-descriptionForeground);font-size:11px}.plot-wrap{grid-row:3;position:relative;min-height:0}.plot-view{position:absolute;inset:0;min-width:0;min-height:0}.plot-view[hidden]{display:none}.plot{position:absolute;inset:0;background:var(--vscode-editor-background);overflow:hidden;outline:none}.plot:focus-visible{outline:1px solid var(--vscode-focusBorder);outline-offset:-1px}.plot-empty{position:absolute;inset:0;display:grid;place-items:center;text-align:center;color:var(--vscode-descriptionForeground);pointer-events:none}.plot-empty button{pointer-events:auto}.chart-annotations{position:absolute;inset:0;pointer-events:none}.signal-list{position:absolute;inset:0;overflow:auto;padding:8px}.signal-row{display:grid;grid-template-columns:auto minmax(130px,1fr) auto auto auto;gap:8px;align-items:center;padding:6px 4px;border-bottom:1px solid var(--vscode-panel-border)}.signal-row input{width:auto}.signal-secondary{color:var(--vscode-descriptionForeground);font-size:11px}.analysis-grid{position:absolute;inset:0;display:grid;grid-template-columns:1fr 1fr;gap:8px;min-height:0}.analysis-grid.single{grid-template-columns:1fr}.bode-grid{grid-template-columns:1fr;grid-template-rows:minmax(0,1fr) minmax(0,1fr);gap:0}.bode-grid .analysis-plot:first-child{border-bottom:1px solid var(--vscode-panel-border)}.analysis-plot{min-width:0;min-height:0;outline:none}.analysis-plot:focus-visible{outline:1px solid var(--vscode-focusBorder);outline-offset:-1px}.design-handle{position:absolute;z-index:2;left:50%;bottom:8px;transform:translateX(-50%);display:flex;align-items:center;gap:6px;padding:4px 8px;background:var(--vscode-editorWidget-background);border:1px solid var(--vscode-widget-border,var(--vscode-panel-border));font-size:11px}.design-handle input[type=range]{width:128px}.design-exact{width:66px;font:inherit;font-family:var(--vscode-editor-font-family);font-variant-numeric:tabular-nums}.design-unit{color:var(--vscode-descriptionForeground)}.design-error:not(:empty){color:var(--vscode-errorForeground)}.chart-table{grid-column:1/-1;min-width:0;max-width:100%;max-height:160px;overflow:auto;border:1px solid var(--vscode-panel-border);margin-top:4px}.chart-table[hidden]{display:none}.chart-table table{width:100%;border-collapse:collapse;font-variant-numeric:tabular-nums}.chart-table th,.chart-table td{text-align:right;padding:3px 8px;border-bottom:1px solid var(--vscode-panel-border)}.chart-table th:first-child,.chart-table td:first-child{text-align:left}.sr-only{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}.values{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:8px;align-content:start;min-width:0;max-width:320px}.value{min-width:0;border-left:2px solid var(--vscode-descriptionForeground);padding:4px 8px}.value-name{font-size:11px;color:var(--vscode-descriptionForeground);white-space:nowrap}.value-reading{font-family:var(--vscode-editor-font-family);font-size:18px;margin-top:3px;font-variant-numeric:tabular-nums;white-space:nowrap}.quality{grid-column:1/-1;margin-top:6px;color:var(--vscode-descriptionForeground);font-size:11px;overflow-wrap:anywhere}
.design-handle[hidden]{display:none}.plot-empty[hidden]{display:none}.chart-annotation{position:absolute;top:34px;bottom:34px;border-left:1px dashed var(--vscode-charts-yellow);transform:translateX(-1px)}.plant-summary{grid-column:1/-1;border-top:1px solid var(--vscode-panel-border);margin-top:6px;padding-top:8px;color:var(--vscode-descriptionForeground);font-size:11px}.plant-summary[hidden]{display:none}.plant-summary-line{display:flex;flex-wrap:wrap;gap:5px 10px}.plant-summary code,.plant-evidence dd{font-family:var(--vscode-editor-font-family);font-variant-numeric:tabular-nums;color:var(--vscode-editor-foreground)}.plant-evidence summary{cursor:pointer;margin-top:6px}.plant-evidence dl{display:grid;grid-template-columns:auto minmax(0,1fr);gap:3px 10px;margin:7px 0 0}.plant-evidence dt{color:var(--vscode-descriptionForeground)}.plant-evidence dd{margin:0;overflow-wrap:anywhere}
.workspace{min-height:0;min-width:0;overflow:auto;padding:18px}.workspace h2{font-size:16px;margin:0 0 6px}.workspace p{color:var(--vscode-descriptionForeground);max-width:78ch;margin:0}.workflow-pane[hidden]{display:none}.workflow-grid{display:grid;grid-template-columns:minmax(220px,360px) minmax(0,1fr);gap:18px;margin-top:14px;min-width:0}.control-group{display:grid;gap:9px;align-content:start;min-width:0}.field{display:grid;gap:4px;min-width:0}.field label,.section-label{font-size:11px;font-weight:600;color:var(--vscode-descriptionForeground)}select,input{width:100%;border:1px solid var(--vscode-input-border,var(--vscode-panel-border));background:var(--vscode-input-background);color:var(--vscode-input-foreground);padding:5px 7px;font:inherit}.physical-review{display:flex;align-items:flex-start;gap:7px;color:var(--vscode-descriptionForeground);font-size:11px}.physical-review[hidden]{display:none}.physical-review input{width:auto;margin-top:2px}.summary-panel{border-left:1px solid var(--vscode-panel-border);padding-left:18px;min-width:0}.summary-panel h3{font-size:13px;margin:0 0 8px}.metrics{display:grid;grid-template-columns:repeat(auto-fit,minmax(120px,1fr));gap:8px}.metric{padding:8px;border:1px solid var(--vscode-panel-border);min-width:0}.metric strong{display:block;font-family:var(--vscode-editor-font-family);font-size:15px;margin-top:3px;overflow-wrap:anywhere}.workspace-actions{display:flex;align-items:center;flex-wrap:wrap;gap:8px;margin-top:14px}.primary,.secondary,.danger,.configure-plant,.next{border:1px solid var(--vscode-button-border,transparent);padding:6px 10px;border-radius:2px;font:inherit}.primary{background:var(--vscode-button-background);color:var(--vscode-button-foreground)}.secondary,.next{background:var(--vscode-button-secondaryBackground);color:var(--vscode-button-secondaryForeground)}.danger{background:transparent;color:var(--vscode-errorForeground);border-color:var(--vscode-errorForeground)}.configure-plant{background:transparent;color:var(--vscode-textLink-foreground)}.advanced{margin-top:12px;min-width:0}.advanced summary{cursor:pointer;color:var(--vscode-textLink-foreground)}.parameter-grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:7px;margin-top:8px}.engineering-plots{display:grid;grid-template-columns:2fr 1fr;gap:10px;margin-top:14px;min-width:0}.engineering-plots[hidden],.identification-fit[hidden]{display:none}.engineering-plot{height:230px;min-width:0;border:1px solid var(--vscode-panel-border)}.engineering-plot.time{grid-row:span 2;height:470px}.identification-fit{margin-top:14px}.identification-fit-plot{height:260px;border:1px solid var(--vscode-panel-border)}.plot-summary{grid-column:1/-1;color:var(--vscode-descriptionForeground)}.workflow-state{padding:8px 10px;border-left:2px solid var(--vscode-focusBorder);background:var(--vscode-textBlockQuote-background);margin-top:12px;overflow-wrap:anywhere}.auto-plan{position:absolute;inset:54px 18px 18px;z-index:5;overflow:auto;background:var(--vscode-editor-background);border:1px solid var(--vscode-panel-border);box-shadow:0 4px 16px var(--vscode-widget-shadow);padding:18px}.auto-plan[hidden]{display:none}.auto-header{display:flex;justify-content:space-between;gap:16px;align-items:start}.stage-list{display:grid;grid-template-columns:repeat(4,1fr);gap:6px;margin:16px 0}.stage{padding:8px;border-top:2px solid var(--vscode-descriptionForeground);background:var(--vscode-sideBar-background)}.stage[data-status=complete]{border-color:var(--vscode-testing-iconPassed)}.candidate{border:1px solid var(--vscode-panel-border);padding:12px;margin-top:12px;min-width:0;max-width:100%}.candidate[hidden]{display:none}.candidate h3{margin:0 0 8px}.candidate pre,.candidate p{max-width:100%;white-space:pre-wrap;overflow-wrap:anywhere}.error{color:var(--vscode-errorForeground)}
.plot-view>.analysis-plot{position:absolute;inset:0}
.studio{grid-template-rows:auto auto minmax(460px,56vh) minmax(0,1fr)}
.plot-empty{z-index:3;padding:24px;background:var(--vscode-editor-background)}
.design-handle{position:static;left:auto;bottom:auto;transform:none;margin-left:auto;padding:2px 6px;white-space:nowrap;background:transparent;border:0}
@media(max-width:720px){.studio{height:100%;min-height:0;display:block;overflow-y:auto;overflow-x:hidden}.monitor{min-height:600px;grid-template-rows:minmax(420px,1fr) auto}.values{max-width:none}}
.studio[data-plot-expanded=true]{grid-template-rows:auto auto minmax(0,1fr)}.studio[data-plot-expanded=true] .monitor{grid-row:3;}.studio[data-plot-expanded=true] .workspace{display:none}
@media(max-width:720px){.header{align-items:flex-start;flex-wrap:wrap}.status{width:100%;white-space:normal}.monitor{grid-template-columns:1fr}.values{grid-template-columns:repeat(3,1fr)}.studio{grid-template-rows:auto auto minmax(360px,52vh) minmax(0,1fr)}.workflow-grid,.engineering-plots{grid-template-columns:1fr}.analysis-grid{grid-template-columns:1fr}.engineering-plot.time{grid-row:auto;height:300px}.parameter-grid{grid-template-columns:1fr}.summary-panel{border-left:0;border-top:1px solid var(--vscode-panel-border);padding:12px 0 0}.stage-list{grid-template-columns:1fr 1fr}.auto-plan{inset:8px}}
@media(max-width:600px){body{height:100vh;min-height:0;overflow:hidden}.studio{height:100%;min-height:0;display:block;overflow-y:auto;overflow-x:hidden}.modes{position:sticky;top:0;z-index:4;background:var(--vscode-editor-background)}.monitor{min-height:260px}.workspace{overflow:visible;min-height:180px}.workflow-pane{scroll-margin-top:56px}.runtime{max-width:150px}}
@media(max-width:600px){.chart-table{width:100%;min-width:0;contain:inline-size paint}}
@media(max-width:600px){.workspace{overflow-x:clip}}
@media(max-width:600px){.modes{flex-wrap:wrap;overflow-x:hidden}.mode{flex:1 1 auto;padding:8px 9px}.title{white-space:normal}.workflow-pane{scroll-margin-top:72px}}
@media(forced-colors:active){.source,.plot,.value{forced-color-adjust:auto}.mode[aria-selected=true]{outline:1px solid Highlight}}
.header{display:grid;grid-template-columns:minmax(180px,1fr) auto}.identity{grid-column:1;grid-row:1}.automatic-tune{grid-column:2;grid-row:1}.status{grid-column:1/-1;grid-row:2;justify-content:flex-start}
@media(max-width:720px){.header{display:flex}.identity,.automatic-tune,.status{grid-column:auto;grid-row:auto}}
@media(max-width:720px){.studio{height:100%;min-height:0;display:block;overflow-y:auto;overflow-x:hidden}.monitor{min-height:600px;grid-template-rows:minmax(420px,1fr) auto}.values{max-width:none}}
@media(max-width:600px){.plot-toolbar{flex-direction:column;align-items:stretch}.plot-tabs,.plot-actions{flex-wrap:wrap;overflow-x:visible}.design-row{flex-wrap:wrap;overflow-x:hidden}.design-handle{width:100%;margin-left:0}.design-handle input{min-width:120px;flex:1}.monitor{min-height:820px;grid-template-rows:minmax(620px,1fr) auto}}
@media(min-width:601px) and (max-width:720px){.analysis-grid:not(.bode-grid){grid-template-columns:1fr 1fr}}
/* Keep the live chart large while leaving the active workflow and its first
   action visible in a standard editor viewport. Expand remains available when
   the chart itself needs the full editor. */
.studio{grid-template-rows:auto auto minmax(440px,54vh) minmax(0,1fr)}
.studio[data-plot-available=false]{grid-template-rows:auto auto minmax(180px,24vh) minmax(0,1fr)}
.monitor{display:grid;grid-template-columns:minmax(0,1fr);grid-template-rows:auto minmax(0,1fr);gap:6px}
.plot-column{grid-row:2}
.values{grid-row:1;max-width:none}
.design-row{flex-wrap:wrap;overflow-x:hidden;padding:3px 0;row-gap:4px}
.design-row button{white-space:nowrap}
.design-handle{margin-left:0}
@media(max-width:600px){.plot-actions{border-left:0;padding-left:0}.design-metrics{width:auto}.design-handle input[type=range]{min-width:96px}}
@media(max-width:600px){.design-handle{display:grid;grid-template-columns:minmax(0,1fr) 68px auto;gap:4px 6px}.design-label{grid-column:1/-1}.design-handle input{min-width:0;flex:none}.design-handle input[type=range]{width:100%;min-width:0}.design-exact{width:68px}.design-error{grid-column:1/-1}}
.event-row{display:grid;grid-template-columns:72px 96px minmax(0,1fr);gap:8px;align-items:baseline;padding:6px 4px;border-bottom:1px solid var(--vscode-panel-border)}
.event-row time{font-family:var(--vscode-editor-font-family);font-variant-numeric:tabular-nums}
.event-kind{color:var(--vscode-descriptionForeground);font-size:11px;text-transform:capitalize}
</style></head><body>
<main class="studio" data-control-studio data-project-id="${project}" data-editor-session-id="${session}" data-plant-id="${escapeHtml(handoff?.plantId ?? loopDefinition?.plantId ?? "")}" data-plant-revision="${revision}" data-loop-id="${loop}" data-loop-revision="${loopRevision}" data-process-unit="${processUnit}" data-output-unit="${outputUnit}">
  <header class="header"><div class="identity"><div class="eyebrow">Control Studio</div><div class="title" data-control-studio-loop>${plant}</div></div><div class="status"><span data-control-studio-source class="source">${sourceLabel}</span><code class="runtime" data-control-studio-runtime title="${runtime}">${runtime}</code><span data-control-studio-connection>Stopped</span><span>Mode <code data-control-studio-controller-mode>Unavailable</code></span><span>Controller <code data-control-studio-controller-revision>Unavailable</code></span><span>Candidate <code data-control-studio-candidate-revision>None</code></span><span data-control-studio-freshness>No current samples</span></div><button class="automatic-tune" data-control-studio-automatic-tune${automaticTuneDisabled ? ' disabled title="A saved simulated plant is required before Automatic Tune can build a plan"' : ' title="Build a complete tuning plan"'}>Automatic Tune</button></header>
  <nav class="modes" role="tablist" aria-label="Control workflow">${["Monitor","Test","Identify","Tune","Validate"].map((mode, index) => {
    const status = index === 0 ? "" : "incomplete";
    return `<button class="mode" data-control-studio-mode role="tab" aria-selected="${index === 0}" tabindex="${index === 0 ? 0 : -1}"${status ? ` data-status="${status}" aria-label="${mode}, incomplete"` : ""}>${mode}</button>`;
  }).join("")}</nav>
  <section class="monitor" data-control-studio-monitor aria-label="Control analysis workspace"><div class="plot-column"><div class="plot-toolbar" role="toolbar" aria-label="Direct plot and inspection controls"><div class="plot-tabs" data-control-plot-tabs role="tablist" aria-label="Plots for Monitor"></div><div class="plot-actions"><button data-chart-action="undo" hidden disabled title="Undo graphical controller edit">Undo</button><button data-chart-action="redo" hidden disabled title="Redo graphical controller edit">Redo</button><button data-chart-action="inspect" title="Inspect samples with Left and Right Arrow">Inspect</button><button data-chart-action="zoom-out" title="Zoom out">Zoom −</button><button data-chart-action="zoom-in" title="Zoom in">Zoom +</button><button data-chart-action="reset" title="Fit all evidence">Fit</button><button data-chart-action="expand" aria-pressed="false" title="Expand the chart workspace">Expand</button><button data-chart-action="export-figure" title="Export the visible plot">Export</button></div></div><div class="design-row" data-control-designs hidden aria-label="Controller design comparison"><span class="design-chip">Current controller</span><span class="design-chip" data-active-design>Candidate unavailable</span><button data-chart-action="pin" disabled>Pin design</button><span data-pinned-designs></span><span class="design-metrics"><span>GM <output data-design-gain-margin>—</output></span><span>PM <output data-design-phase-margin>—</output></span></span></div><div class="plot-wrap"><div class="plot-view" data-plot-view="response"><div class="plot" data-control-studio-chart data-chart-adapter="echarts" role="img" tabindex="0" aria-label="SP, PV, and MV time plot; no live samples"></div><span class="plot-empty" data-live-empty>No live samples. Start the simulator to receive current samples.</span><div class="chart-annotations" data-chart-annotations aria-hidden="true"></div></div><div class="plot-view" data-plot-view="signals" hidden><div class="signal-list" data-control-signals aria-label="Available control signals"></div></div><div class="plot-view" data-plot-view="events" hidden><div class="signal-list" data-control-events>No recorded events. Run or open an experiment to create event evidence.</div></div><div class="plot-view" data-plot-view="engineering-response" hidden><div class="analysis-plot" data-engineering-time role="img" tabindex="0" aria-label="Current and candidate model-predicted response"></div></div><div class="plot-view" data-plot-view="bode" hidden><div class="analysis-grid bode-grid"><div class="analysis-plot" data-engineering-bode-magnitude role="img" tabindex="0" aria-label="Open-loop Bode magnitude"></div><div class="analysis-plot" data-engineering-bode-phase role="img" tabindex="0" aria-label="Open-loop Bode phase"></div></div><label class="design-handle">Proportional gain (Kp)<input data-design-handle="proportionalGain" type="range" min="0.01" max="20" step="0.01" value="1" aria-label="Graphical proportional gain"><input class="design-exact" data-design-exact="proportionalGain" type="number" min="0.01" max="20" step="0.01" value="1" inputmode="decimal" aria-label="Exact proportional gain"><span class="design-unit" data-design-unit>output/process</span><span class="design-error" data-design-error role="status"></span></label></div><div class="plot-view" data-plot-view="root-locus" hidden><div class="analysis-plot" data-engineering-root-locus role="img" tabindex="0" aria-label="Root locus"></div></div><div class="plot-view" data-plot-view="nyquist" hidden><div class="analysis-plot" data-engineering-nyquist role="img" tabindex="0" aria-label="Nyquist stability plot"></div></div><div class="plot-view" data-plot-view="nichols" hidden><div class="analysis-plot" data-engineering-nichols role="img" tabindex="0" aria-label="Nichols plot"></div></div><div class="plot-view" data-plot-view="sensitivity" hidden><div class="analysis-plot" data-engineering-sensitivity role="img" tabindex="0" aria-label="Sensitivity functions"></div></div><div class="plot-view" data-plot-view="requirements" hidden><div class="analysis-plot" data-engineering-requirements role="img" tabindex="0" aria-label="Controller requirements and uncertainty"></div></div><div class="plot-view" data-plot-view="unavailable" hidden><div class="plot-empty" data-plot-unavailable></div></div></div><p class="sr-only" data-chart-status aria-live="polite">No samples available for inspection</p><div class="chart-table" data-chart-table hidden><table><caption class="sr-only">Displayed SP, PV, and MV samples</caption><thead><tr><th scope="col">Time (s)</th><th scope="col">SP</th><th scope="col">PV</th><th scope="col">MV (%)</th></tr></thead><tbody></tbody></table></div></div><div class="values"><div class="value"><div class="value-name">SP · Setpoint</div><div class="value-reading" data-control-studio-sp>—</div></div><div class="value"><div class="value-name">PV · Process value</div><div class="value-reading" data-control-studio-pv>—</div></div><div class="value"><div class="value-name">MV · Controller output</div><div class="value-reading" data-control-studio-mv>—</div></div><div class="quality" data-control-studio-quality>Unavailable · Waiting for a current runtime sample</div><div class="plant-summary" data-plant-summary hidden><div class="plant-summary-line"><span>Plant <code data-plant-state>Unavailable</code></span><span>Command <code data-plant-command>—</code></span><span>Measured <code data-plant-measured>—</code></span><span>Published <code data-plant-published>—</code></span><span>Fault <code data-plant-fault>None</code></span></div><details class="plant-evidence"><summary>Plant evidence</summary><dl><dt>Model</dt><dd data-plant-model>—</dd><dt>Applied input</dt><dd data-plant-applied>—</dd><dt>Ideal output</dt><dd data-plant-ideal>—</dd><dt>State</dt><dd data-plant-vector>—</dd><dt>Time scale</dt><dd data-plant-time-scale>—</dd><dt>Seed</dt><dd data-plant-seed>—</dd><dt>Random algorithm</dt><dd data-plant-random>—</dd><dt>Active effects</dt><dd data-plant-effects>None</dd><dt>Active dropouts</dt><dd data-plant-dropouts>None</dd><dt>Pending events</dt><dd data-plant-events>0</dd><dt>Delay buffer</dt><dd data-plant-delay>—</dd><dt>Last substeps</dt><dd data-plant-substeps>—</dd><dt>Runtime</dt><dd data-plant-runtime>—</dd></dl></details></div></div></section>
  <section class="workspace" aria-live="polite">
    <div class="workflow-pane" data-workflow-pane="Monitor"><h2>Monitor</h2><p>Observe the loop continuously. Live SP, PV, MV, limits, operating state, safety state, and data quality remain above every engineering workflow.</p><div class="workspace-actions"><button class="secondary next" data-control-studio-record-start${loopDefinition ? "" : " disabled title=\"Bind a controller loop before recording\""}>Start recording</button><button class="secondary" data-control-studio-record-stop disabled>Stop recording</button>${handoff ? '<button class="configure-plant" data-control-studio-configure-plant>Configure plant</button>' : ""}</div></div>
    <div class="workflow-pane" data-workflow-pane="Test" hidden><h2>Test</h2><p>Run a bounded experiment with continuous live values and an always-available abort. Physical actions require a reviewed topology and immediate approval.</p><div class="workflow-grid"><div class="control-group"><div class="field"><label for="test-kind">Experiment</label><select id="test-kind"${experimentDefaults.execution === "project-scripted" ? " disabled" : ""}>${experimentOptions}</select></div><div class="field"><label for="test-amplitude">Amplitude</label><input id="test-amplitude" type="number" value="${experimentDefaults.amplitude}" step="0.1"${experimentDefaults.execution === "project-scripted" ? " disabled" : ""}></div><div class="field"><label for="test-duration">Maximum duration (s)</label><input id="test-duration" type="number" value="${experimentDefaults.durationSeconds}" min="1" max="3600"></div>${experimentDefaults.execution === "project-scripted" ? `<p data-test-scripted-note>The runnable lesson owns the scenario. Control Studio records it without writing a second excitation.</p>` : ""}<label class="physical-review" data-physical-topology hidden><input type="checkbox" data-physical-topology-reviewed>Independent topology review is complete for this physical loop.</label><div class="workspace-actions"><button class="primary" data-test-review${loopDefinition ? "" : " disabled"}>Review test</button><button class="primary" data-test-approve hidden>Confirm and start</button><button class="danger" data-test-abort disabled>Abort test</button></div></div><div class="summary-panel"><h3>Safety envelope</h3><div class="workflow-state" data-test-state>No experiment is running. Existing samples remain visible above.</div><div class="metrics" data-test-preview hidden><div class="metric">Signal<strong data-test-signal>—</strong></div><div class="metric">Requested bound<strong data-test-bound>—</strong></div><div class="metric">Duration<strong data-test-timeout>—</strong></div><div class="metric">Restoration<strong data-test-restoration>—</strong></div><div class="metric">Deterministic seed<strong>${experimentDefaults.seed}</strong></div></div><p data-test-stops></p></div></div></div>
    <div class="workflow-pane" data-workflow-pane="Identify" hidden><h2>Identify</h2><p>Use one recording to estimate the model and a distinct recording to check it. Tune remains unavailable until you accept the reviewed model.</p><div class="workflow-grid"><div class="control-group"><div class="field"><label for="identify-model">Model selection</label><select id="identify-model"><option value="automatic">Automatic · simplest acceptable</option><option value="integrating">Integrating</option><option value="first_order">First order</option><option value="fopdt">FOPDT</option><option value="second_order">Second order</option><option value="sopdt">SOPDT</option></select></div><details class="advanced"><summary>Exclude one disturbed interval</summary><div class="parameter-grid"><label>Start (s)<input id="identify-exclude-start" type="number" min="0" step="any" placeholder="None"></label><label>End (s)<input id="identify-exclude-end" type="number" min="0" step="any" placeholder="None"></label></div></details><div class="workspace-actions"><button class="primary" data-identify-run disabled title="Stop a recording before identification">Qualify recording</button><button class="primary" data-identify-fit hidden>Fit estimation run</button><button class="primary" data-identify-accept hidden>Accept model for design</button></div></div><div class="summary-panel"><h3>Model qualification</h3><div class="workflow-state" data-identify-state>Stop a recording after a bounded test, then qualify it.</div><div class="metrics" data-identify-qualification hidden><div class="metric">Usable samples<strong data-identify-samples>—</strong></div><div class="metric">Duration<strong data-identify-duration>—</strong></div><div class="metric">Sample interval<strong data-identify-interval>—</strong></div><div class="metric">Signal / noise<strong data-identify-snr>—</strong></div></div><details class="advanced" data-identify-details hidden><summary>Qualification details</summary><div class="chart-table"><table><caption class="sr-only">Identification data qualification</caption><tbody data-identify-detail-rows></tbody></table></div></details><details class="advanced" data-identify-model-details hidden><summary>Model evidence</summary><div class="chart-table"><table><caption class="sr-only">Identified model evidence</caption><tbody data-identify-model-rows></tbody></table></div></details></div></div><section class="identification-fit" data-identification-fit hidden aria-label="Identification fit evidence"><div class="identification-fit-plot" data-identification-fit-plot role="img" tabindex="0" aria-label="Measured and predicted process response with residual trace"></div><p class="plot-summary" data-identification-fit-summary aria-live="polite">No model fit is available.</p></section></div>
    <div class="workflow-pane" data-workflow-pane="Tune" hidden><h2>Tune</h2><p>${escapeHtml(tuneDescription)}</p><div class="workflow-grid"><div class="control-group"><div class="field"><label for="tune-method">Design method</label><select id="tune-method"${pidDesign.available ? "" : " disabled"}><option value="SIMC">SIMC</option><option value="AMIGO">AMIGO</option><option value="Manual">Manual Control.PID</option><option value="Imported">Import Control.PID JSON</option></select></div><div class="field" data-tune-objective-field><label for="tune-objective">Response</label><select id="tune-objective"${pidDesign.available ? "" : " disabled"}><option value="robust">More robust</option><option value="balanced" selected>Balanced</option><option value="fast">Faster</option></select></div><div class="field" data-tune-import hidden><label for="tune-import-file">Control.PID JSON</label><input id="tune-import-file" type="file" accept="application/json,.json"><span data-tune-import-state>No file selected</span></div><details class="advanced" data-tune-settings hidden><summary>Exact Control.PID settings</summary><div class="parameter-grid"><label>Kp<input data-pid-setting="proportionalGain" type="number" value="1" step="any"></label><label>Ki<input data-pid-setting="integralGain" type="number" value="0.1" step="any"></label><label>Kd<input data-pid-setting="derivativeGain" type="number" value="0" step="any"></label><label>SP weight<input data-pid-setting="proportionalSetpointWeight" type="number" value="1" step="any"></label><label>D SP weight<input data-pid-setting="derivativeSetpointWeight" type="number" value="0" step="any"></label><label>Bias<input data-pid-setting="bias" type="number" value="0" step="any"></label><label>Output min<input data-pid-setting="outputMinimum" type="number" value="0" step="any"></label><label>Output max<input data-pid-setting="outputMaximum" type="number" value="100" step="any"></label><label>Rate min<input data-pid-setting="rateMinimum" type="number" value="-100" step="any"></label><label>Rate max<input data-pid-setting="rateMaximum" type="number" value="100" step="any"></label><label>Tracking time (s)<input data-pid-setting="trackingTime" type="number" value="1" min="0" step="any"></label><label>Anti-windup<select data-pid-setting="antiWindup"><option value="BackCalculation">Back calculation</option><option value="Combined">Combined</option></select></label></div></details><button class="primary" data-tune-generate disabled title="${escapeHtml(tuneDisabledReason)}">Generate candidate</button></div><div class="summary-panel"><h3>Candidate comparison</h3><p>Current and candidate response, control effort, limits, plant/open-loop Bode response and stability margins use the same accepted model.</p><div class="workflow-state" data-tune-state>${escapeHtml(tuneStateText)}</div><details class="advanced" data-tune-method-evidence hidden><summary>Method evidence</summary><div class="chart-table"><table><caption class="sr-only">Qualified tuning method evidence</caption><tbody data-tune-method-rows></tbody></table></div></details></div></div><section class="engineering-plots" data-engineering-plots hidden aria-label="Controller engineering plots"><div class="engineering-plot time" data-engineering-time role="img" tabindex="0" aria-label="Current and candidate model-predicted time response and controller output"></div><div class="engineering-plot" data-engineering-bode-magnitude role="img" tabindex="0" aria-label="Plant and candidate open-loop Bode magnitude"></div><div class="engineering-plot" data-engineering-bode-phase role="img" tabindex="0" aria-label="Plant and candidate open-loop Bode phase"></div><p class="plot-summary" data-engineering-plot-summary aria-live="polite">No engineering prediction is available.</p><details class="advanced"><summary>Engineering plot data</summary><div class="chart-table"><table><caption>Selected model-predicted response points</caption><thead><tr><th>Time (s)</th><th>Current PV</th><th>Candidate PV</th><th>Current MV</th><th>Candidate MV</th></tr></thead><tbody data-engineering-table></tbody></table></div></details></section></div>
    <div class="workflow-pane" data-workflow-pane="Validate" hidden><h2>Validate</h2><p>Compare current and candidate controllers under identical required scenarios. Validation never applies gains.</p><div class="workflow-grid"><div class="control-group"><button class="primary" data-validate-run disabled title="Generate a candidate before validation">Run required validation</button><button class="primary" data-candidate-review hidden>Review application</button><div class="workspace-actions"><button class="secondary" data-result-save disabled>Save result</button><button class="secondary" data-result-export-csv disabled>Export samples</button><button class="secondary" data-result-export-json disabled>Export result</button></div><details class="advanced"><summary>Saved results</summary><div class="parameter-grid"><label>Open revision<select data-result-history aria-label="Saved result revision"><option value="">No saved results</option></select></label><label>Compare with<select data-result-compare aria-label="Comparison result revision"><option value="">Select revision</option></select></label></div><div class="workspace-actions"><button class="secondary" data-result-open disabled>Open</button><button class="secondary" data-result-compare-run disabled>Compare</button></div><div class="workflow-state" data-result-compare-state>Choose two revisions to compare their saved evidence.</div></details></div><div class="summary-panel"><h3>Acceptance evidence</h3><p>Every requirement reports Pass, Fail or Inconclusive with value, limit, unit, source and reason.</p><div class="workflow-state" data-validate-state>Generate a candidate first.</div><details class="advanced"><summary>Requirement details</summary><div class="chart-table"><table><caption class="sr-only">Complete validation requirement evidence</caption><thead><tr><th scope="col">Requirement</th><th scope="col">Status</th><th scope="col">Value</th><th scope="col">Limit</th><th scope="col">Source</th><th scope="col">Reason</th></tr></thead><tbody data-validation-requirements></tbody></table></div></details><details class="advanced" data-saved-tuning-comparison hidden><summary>Tuning comparison</summary><div class="chart-table" data-saved-tuning-attempts></div></details><div class="workflow-state" data-result-state>Nothing saved in this workspace yet.</div></div></div></div>
    <section class="candidate" data-shared-application-review hidden aria-label="Controller application review"><h2>Apply reviewed candidate</h2><p>Application is separate from validation. Confirm the active controller revision and exact unapplied settings before writing.</p><div class="metrics"><div class="metric">Source<strong data-application-source>—</strong></div><div class="metric">Structure<strong data-application-structure>—</strong></div><div class="metric">Kp / Ki / Kd<strong data-application-gains>—</strong></div><div class="metric">Output limits<strong data-application-limits>—</strong></div><div class="metric">Validation<strong data-application-validation>Not validated</strong></div></div><details class="advanced"><summary>Exact Control.PID settings</summary><pre data-application-settings>—</pre></details><div class="workspace-actions"><button class="primary" data-shared-application-confirm>Apply reviewed candidate</button><button class="danger" data-shared-application-rollback hidden>Rollback</button></div><p data-shared-application-state>Not applied</p></section>
  <section class="auto-plan" data-auto-tune-plan hidden aria-label="Automatic Tune plan"><div class="auto-header"><div><h2>Automatic Tune</h2><p>Review Test → Identify → Tune → Validate. Completion creates an unapplied candidate.</p></div><button class="secondary" data-auto-tune-close aria-label="Close Automatic Tune">Close</button></div><div class="workflow-grid"><div class="control-group"><div class="field"><label for="auto-objective">Response</label><select id="auto-objective" data-auto-tune-objective><option value="robust">More robust</option><option value="balanced" selected>Balanced</option><option value="fast">Faster</option></select></div><div><span class="section-label">Evidence source</span><p data-auto-tune-source>Simulation · exact saved model</p></div></div><div><div class="stage-list"><div class="stage" data-auto-tune-stage="Test" data-status="waiting">Test · Waiting</div><div class="stage" data-auto-tune-stage="Identify" data-status="waiting">Identify · Waiting</div><div class="stage" data-auto-tune-stage="Tune" data-status="waiting">Tune · Waiting</div><div class="stage" data-auto-tune-stage="Validate" data-status="waiting">Validate · Waiting</div></div><p>The exact saved simulation model is reused; no excitation is needed. No gains are applied automatically.</p><div class="workspace-actions"><button class="primary" data-auto-tune-approve>Confirm and run</button></div><div class="candidate" data-auto-tune-result hidden><h3>Recommended <span data-auto-tune-structure>—</span></h3><div class="metrics"><div class="metric">Method<strong data-auto-tune-method>—</strong></div><div class="metric">Kp<strong data-auto-tune-kp>—</strong></div><div class="metric">Ti<strong data-auto-tune-ti>—</strong></div><div class="metric">Model<strong data-auto-tune-model>—</strong></div><div class="metric">Gain margin<strong data-auto-tune-gain-margin>—</strong></div><div class="metric">Phase margin<strong data-auto-tune-phase-margin>—</strong></div><div class="metric">Validation<strong data-auto-tune-validation>—</strong></div></div><p data-auto-tune-rationale></p><details class="advanced"><summary>Method comparison</summary><div class="chart-table" data-auto-tune-attempts aria-live="polite"></div></details><div class="workspace-actions"><button class="primary" data-auto-tune-apply${loopDefinition ? "" : " disabled title=\"Bind a complete controller loop before applying\""}>Review application</button><button class="secondary" data-auto-tune-export>Export candidate</button></div></div><p class="error" data-auto-tune-error hidden></p></div></div></section>
  </section>
</main><script nonce="${nonce}" src="${chartScript}"></script><script nonce="${nonce}" src="${workflowScript}"></script></body></html>`;
}

function text(value: unknown, fallback: string): string {
  return typeof value === "string" && value.trim() ? value.trim() : fallback;
}

function escapeHtml(value: string): string {
  return value.replace(/[&<>"']/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[character] ?? character));
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
        ? `lambda ${currentTuning.closedLoopTimeSeconds} s`
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
