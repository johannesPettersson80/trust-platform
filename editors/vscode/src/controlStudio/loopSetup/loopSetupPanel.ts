import { createHash } from "crypto";
import * as path from "path";
import * as vscode from "vscode";
import { escapeHtml } from "../panelTemplate";
import { webviewNonce } from "../panelPresentation";
import { THREE_PAGE_STYLES } from "../threePageStyles";
import type { CompiledCatalog } from "./compiledCatalog";
import { saveLoopSetup, type LoopSetupDraft } from "./loopSetupSave";
import { renderLoopSetupBody } from "./loopSetupTemplate";
import { setupFormModel } from "./setupFormModel";
import { estimatorModelFiles } from "./estimatorModelFiles";

export const LOOP_SETUP_VIEW_TYPE = "trust-loop-setup";

/** The project scope Control Studio uses for a workspace folder. */
export function setupProjectId(folder: vscode.Uri): string {
  return `project:${createHash("sha256").update(folder.toString()).digest("hex")}`;
}

/** The one-page setup form. Saving writes the loops, closes the form and opens Control Studio on the first loop. */
export async function openLoopSetupPanel(context: vscode.ExtensionContext, folder: vscode.Uri, runtimeId: string, catalog: CompiledCatalog,
  open: (request: unknown) => void | Promise<void>, stepByStep?: () => void | Promise<void>): Promise<vscode.WebviewPanel> {
  const estimatorModels = await estimatorModelFiles(folder.fsPath);
  const panel = vscode.window.createWebviewPanel(LOOP_SETUP_VIEW_TYPE, "Set up control loop", vscode.ViewColumn.Active, {
    enableScripts: true,
    localResourceRoots: [vscode.Uri.file(path.join(context.extensionPath, "media")), vscode.Uri.file(path.join(context.extensionPath, "src", "webview"))],
  });
  const webview = panel.webview, nonce = webviewNonce();
  const script = webview.asWebviewUri(vscode.Uri.file(path.join(context.extensionPath, "media", "controlStudioSetup.js")));
  const theme = webview.asWebviewUri(vscode.Uri.file(path.join(context.extensionPath, "src", "webview", "theme.css")));
  webview.html = `<!doctype html><html lang="en"><head><meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1.0">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src ${webview.cspSource} 'nonce-${nonce}';">
<link rel="stylesheet" href="${theme}"><style>${THREE_PAGE_STYLES}</style></head><body>
<main class="control-studio"><h1 class="studio-response-title">Set up a control loop</h1>
${renderLoopSetupBody(setupFormModel(catalog, estimatorModels), escapeHtml)}</main>
<script nonce="${nonce}" src="${script}"></script></body></html>`;
  const scope = { projectId: setupProjectId(folder), runtimeId };
  webview.onDidReceiveMessage(async (message: { kind?: string; draft?: LoopSetupDraft }) => {
    if (message?.kind === "control-studio.setup.step-by-step" && stepByStep) { panel.dispose(); await stepByStep(); return; }
    if (message?.kind !== "control-studio.setup.save" || !message.draft) return;
    try {
      const [first] = await saveLoopSetup(folder.fsPath, scope, catalog, message.draft);
      panel.dispose();
      await open(first);
    } catch (error) {
      void webview.postMessage({ kind: "control-studio.setup.error", reason: error instanceof Error ? error.message : String(error) });
    }
  });
  return panel;
}
