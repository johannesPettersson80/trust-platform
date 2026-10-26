import * as path from "path";
import * as vscode from "vscode";
import { getBinaryPath } from "../../binary";
import { escapeHtml } from "../panelTemplate";
import { webviewNonce } from "../panelPresentation";
import { THREE_PAGE_STYLES } from "../threePageStyles";
import { createEngineClient } from "../engine/engineClient";
import { controlStudioEngine } from "../engine/enginePanelRoute";
import type { SetupDiscoverResult } from "../generated/SetupDiscoverResult";
import { renderLoopSetupBody, type SetupPageModel } from "./loopSetupTemplate";
import { engineAnswer, refusalText, saveSetupDraft } from "./setupEngine";

export const LOOP_SETUP_VIEW_TYPE = "trust-loop-setup";

/**
 * The one-page setup form (B1.3). The engine answers the form's model from the compiled program and saves the confirmed draft.
 * Saving writes the loops, closes the form and opens Control Studio on the first loop. A refusal shows the engine's whole text under
 * the field it names, or in the message area when it names none.
 */
export async function openLoopSetupPanel(context: vscode.ExtensionContext, folder: vscode.Uri, runtimeId: string,
  open: (request: unknown) => void | Promise<void>, stepByStep?: () => void | Promise<void>): Promise<vscode.WebviewPanel | undefined> {
  const engine = createEngineClient(controlStudioEngine(context));
  // The compiled project is read by the language tool the extension runs, so the engine is given that tool (§0.1a).
  const languageTool = getBinaryPath(context, "trust-lsp", "server.path");
  const discovered = await engineAnswer(() => engine.setupDiscover({ projectRoot: folder.fsPath, runtimeId, languageTool }));
  if ("failure" in discovered) {
    void vscode.window.showErrorMessage(refusalText(discovered.failure));
    return undefined;
  }
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
${renderLoopSetupBody(formModelOf(discovered.result), escapeHtml)}</main>
<script nonce="${nonce}" src="${script}"></script></body></html>`;
  webview.onDidReceiveMessage(async (message: { kind?: string; draft?: Record<string, unknown> }) => {
    if (message?.kind === "control-studio.setup.step-by-step" && stepByStep) { panel.dispose(); await stepByStep(); return; }
    const draft = message?.draft;
    if (message?.kind !== "control-studio.setup.save" || !draft) return;
    const outcome = await saveSetupDraft(engine, { projectRoot: folder.fsPath, runtimeId, languageTool, draft });
    if (outcome.kind === "refused") {
      const field = outcome.failure.field;
      void webview.postMessage({ kind: "control-studio.setup.error", reason: refusalText(outcome.failure), field: typeof field === "string" ? field : undefined });
      return;
    }
    if (!outcome.openRequest) {
      void webview.postMessage({ kind: "control-studio.setup.error", reason: "The engine named no loop to open." });
      return;
    }
    try {
      panel.dispose();
      await open(outcome.openRequest);
    } catch (error) {
      void webview.postMessage({ kind: "control-studio.setup.error", reason: error instanceof Error ? error.message : String(error) });
    }
  });
  return panel;
}

/** The setup form's model as the engine answers it, with the model files an LQG estimator can implement (B1.3, C4). */
export function formModelOf(answer: SetupDiscoverResult): SetupPageModel {
  const estimatorModels = answer.estimatorModels ?? [];
  return { ...answer.formModel, ...(estimatorModels.length ? { estimatorModels } : {}) };
}
