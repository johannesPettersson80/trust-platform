import type { SetupPromptPresenter } from "./setupPromptTypes";
import * as vscode from "vscode";
import { configureControlLoop, type DiscoverControlSignals } from "./configureLoop";
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
): Promise<void> {
  const configured = await configureControlLoop(getSelectedRuntimeId(), discover, presenter);
  if (configured.kind === "accepted") await show(context, configured.request);
  else if (configured.kind !== "cancelled") void vscode.window.showErrorMessage(configured.reason);
}

/** Native commands reuse the bound panel's existing result/export owner. */
export function registerControlStudioCommands(context: vscode.ExtensionContext, handlers: {
  open: (request?: unknown) => void | Promise<void>;
  configure: () => void | Promise<void>;
  exportRecording: () => void | Promise<void>;
}): void {
  for (const [name, handler] of Object.entries(handlers)) {
    context.subscriptions.push(vscode.commands.registerCommand(`trust-lsp.controlStudio.${name}`, handler));
  }
}
