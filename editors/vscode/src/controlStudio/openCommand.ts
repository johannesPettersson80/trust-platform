import type { SetupPromptPresenter } from "./setupPromptTypes";
import * as vscode from "vscode";
import { configureControlLoop, type DiscoverControlSignals } from "./configureLoop";
import { decodeCompiledCatalog } from "./loopSetup/compiledCatalog";
import { openLoopSetupPanel } from "./loopSetup/loopSetupPanel";
import { getSelectedRuntimeId } from "../selectedRuntime";
import { controlStudioRequestForWorkspace } from "./projectLoop";

/** Command discovery is separate from the bound editor's lifecycle and state. */
export async function openControlStudioCommand(
  context: vscode.ExtensionContext,
  request: unknown,
  show: (context: vscode.ExtensionContext, request: unknown) => void | Promise<void>,
  discover?: DiscoverControlSignals,
  presenter?: SetupPromptPresenter
): Promise<void> {
  if (request !== undefined) {
    await show(context, request);
    return;
  }
  const resolution = await controlStudioRequestForWorkspace(getSelectedRuntimeId());
  if (resolution.kind === "cancelled") return;
  if (resolution.kind === "accepted") {
    await show(context, resolution.request);
    return;
  }
  if (resolution.kind === "invalid") {
    const action = await vscode.window.showErrorMessage(resolution.reason, "Open configuration");
    if (action === "Open configuration") {
      try {
        const document = await vscode.workspace.openTextDocument(vscode.Uri.file(resolution.configurationPath));
        await vscode.window.showTextDocument(document);
      } catch {
        void vscode.window.showErrorMessage("The loop configuration could not be opened. Check that the project file still exists and is readable.");
      }
    }
    return;
  }
  const action = await vscode.window.showErrorMessage(resolution.reason, "Configure control loop", "Choose an example");
  if (action === "Configure control loop") {
    await configureControlStudioCommand(context, show, discover, presenter);
    return;
  }
  if (action === "Choose an example") {
    await vscode.commands.executeCommand("trust.examples.start");
  }
}


export async function configureControlStudioCommand(
  context: vscode.ExtensionContext,
  show: (context: vscode.ExtensionContext, request: unknown) => void | Promise<void>,
  discover?: DiscoverControlSignals,
  presenter?: SetupPromptPresenter
): Promise<{ opened: boolean; reason?: string }> {
  const refuse = (reason: string) => { void vscode.window.showErrorMessage(reason); return { opened: false, reason }; };
  const folders = vscode.workspace.workspaceFolders ?? [];
  const folder = folders.length === 1 ? folders[0] : (await vscode.window.showQuickPick(
    folders.map(item => ({ label: item.name, description: item.uri.fsPath, folder: item })),
    { title: "Configure control loop", placeHolder: "Choose the project containing your controller" }))?.folder;
  if (!folder) return { opened: false, reason: "cancelled" };
  if (!discover) return refuse("Compiled signal discovery is unavailable. Wait for the language server and retry.");
  const payload = await discover(folder.uri.toString()) as { ok?: boolean; error?: string; controlSymbols?: unknown } | undefined;
  const catalog = payload?.ok === true ? decodeCompiledCatalog(payload.controlSymbols) : undefined;
  if (!catalog) return refuse(payload?.error ?? "Compile the project successfully before configuring its loop.");
  await openLoopSetupPanel(context, folder.uri, getSelectedRuntimeId(), catalog, request => show(context, request), async () => {
    // Controllers the form does not set up yet keep the step-by-step setup.
    const configured = await configureControlLoop(getSelectedRuntimeId(), discover, presenter);
    if (configured.kind === "accepted") await show(context, configured.request);
    else if (configured.kind !== "cancelled") void vscode.window.showErrorMessage(configured.reason);
  });
  return { opened: true };
}

/** Native commands reuse the bound panel's existing result/export owner. */
export function registerControlStudioCommands(context: vscode.ExtensionContext, handlers: {
  open: (request?: unknown) => void | Promise<void>;
  configure: () => unknown;
  exportRecording: () => void | Promise<void>;
}): void {
  for (const [name, handler] of Object.entries(handlers)) {
    context.subscriptions.push(vscode.commands.registerCommand(`trust-lsp.controlStudio.${name}`, handler));
  }
}
