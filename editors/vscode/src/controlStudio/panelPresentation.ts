import * as vscode from "vscode";
import * as path from "path";
import type { PlantControlStudioHandoff } from "./handoff";
import type { ControlLoopDefinition } from "./loopDefinition";
import { escapeHtml, renderControlStudioHtml } from "./panelTemplate";

export function controlStudioHtml(
  webview: vscode.Webview,
  context: vscode.ExtensionContext,
  handoff?: PlantControlStudioHandoff,
  loopDefinition?: ControlLoopDefinition,
  sessionId = "",
  targetLabel?: string
): string {
  const project = escapeHtml(loopDefinition?.projectId ?? handoff?.projectId ?? "");
  const session = escapeHtml(sessionId);
  const revision = escapeHtml(handoff?.savedConfigurationRevision ?? "");
  const loop = escapeHtml(loopDefinition?.id ?? handoff?.loopId ?? "");
  const loopRevision = escapeHtml(loopDefinition?.revision ?? "");
  const processUnit = escapeHtml(loopDefinition?.signals.processVariable.unit ?? "");
  const outputUnit = escapeHtml(loopDefinition?.signals.manipulatedVariable.unit ?? "");
  const nonce = webviewNonce();
  const chartScript = webview.asWebviewUri(
    vscode.Uri.file(path.join(context.extensionPath, "media", "controlStudioChart.js"))
  ).toString();
  const workflowScript = webview.asWebviewUri(
    vscode.Uri.file(path.join(context.extensionPath, "media", "controlStudioWorkflow.js"))
  ).toString();
  return renderControlStudioHtml({
    cspSource: webview.cspSource, nonce, chartScript, workflowScript,
    themeStyles: webview.asWebviewUri(vscode.Uri.file(path.join(context.extensionPath, "src", "webview", "theme.css"))).toString(),
    project, session, revision, loop, loopRevision, processUnit, outputUnit, loopDefinition, targetLabel,
  });
}

export function webviewNonce(): string {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let value = "";
  for (let index = 0; index < 24; index += 1) value += alphabet[Math.floor(Math.random() * alphabet.length)];
  return value;
}
