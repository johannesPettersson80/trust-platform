import * as vscode from "vscode";
import * as path from "path";
import { controllerAdapter } from "./controllerAdapters";
import type { PlantControlStudioHandoff } from "./handoff";
import { sharedPidDesignAvailability, type ControlLoopDefinition } from "./loopDefinition";
import { controlExperimentChoices, controlExperimentDefaults } from "./experiment";
import { escapeHtml, renderControlStudioHtml, type AutomaticTuneButtonView } from "./panelTemplate";
import { controlStudioAutomaticTuneAvailability, controlStudioStageAvailability, type ControlStudioStageEvidence } from "./stageAvailability";
import { text } from "./panelBindings";

export function controlStudioHtml(
  webview: vscode.Webview,
  context: vscode.ExtensionContext,
  plantId: string,
  runtimeId?: string,
  handoff?: PlantControlStudioHandoff,
  loopDefinition?: ControlLoopDefinition,
  sessionId = "",
  evidence: ControlStudioStageEvidence = {}
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
    execution: "control-studio", kind: "automatic", amplitude: 5, durationSeconds: 30, seed: 1,
  } as const;
  const experimentOptions = controlExperimentChoices()
    .map((choice) => `<option value="${choice.value}"${choice.value === experimentDefaults.kind ? " selected" : ""}>${escapeHtml(choice.label)}</option>`)
    .join("");
  const pidDesign = loopDefinition ? sharedPidDesignAvailability(loopDefinition) : { available: true } as const;
  const automaticTuneDisabled = !pidDesign.available || (!handoff && !loopDefinition?.plantId);
  const tuneDescription = loopDefinition?.analysis
    ? "Analyze the provided controller and plant model, then select a candidate for testing."
    : pidDesign.available
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
  let connectedWorkflow = false;
  if (loopDefinition) {
    try { controllerAdapter(loopDefinition); connectedWorkflow = true; } catch { /* Keep the existing manual adapter. */ }
  }
  const automaticTune: AutomaticTuneButtonView = {
    hidden: connectedWorkflow || !pidDesign.available,
    disabled: automaticTuneDisabled,
    primary: automaticTuneMode.primary,
    title: automaticTuneDisabled
      ? "A saved simulated plant is required before Automatic Tune can build a plan"
      : automaticTuneMode.reason,
  };
  const stageAvailability = controlStudioStageAvailability(experimentDefaults.execution, {
    ...evidence, loopBound: Boolean(loopDefinition), hasDeclaredAnalysis: Boolean(loopDefinition?.analysis),
    hasManualParameters: Boolean(loopDefinition?.parameterBinding && loopDefinition.parameterPurpose !== "experiment"),
  });
  return renderControlStudioHtml({
    cspSource: webview.cspSource, nonce, chartScript, workflowScript,
    themeStyles: webview.asWebviewUri(vscode.Uri.file(path.join(context.extensionPath, "src", "webview", "theme.css"))).toString(),
    plant, sourceLabel, runtime, controllerInstance, project, session, revision,
    loop, loopRevision, processUnit, outputUnit, experimentDefaults, experimentOptions,
    pidDesign, tuneDescription, tuneDisabledReason, tuneStateText, automaticTune,
    stageAvailability, loopDefinition, handoff, guidedWorkflow: connectedWorkflow,
  });
}

export function webviewNonce(): string {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let value = "";
  for (let index = 0; index < 24; index += 1) value += alphabet[Math.floor(Math.random() * alphabet.length)];
  return value;
}
