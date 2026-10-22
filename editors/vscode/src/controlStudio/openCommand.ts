import type { SetupPromptPresenter } from "./setupPromptTypes";
import * as vscode from "vscode";
import { configureControlLoop } from "./configureLoop";
import { openLoopSetupPanel } from "./loopSetup/loopSetupPanel";
import { getSelectedRuntimeId } from "../selectedRuntime";
import { createEngineClient } from "./engine/engineClient";
import { controlStudioEngine } from "./engine/enginePanelRoute";
import { projectLoops } from "./engine/engineProjectLoops";
import { refusalText } from "./loopSetup/setupEngine";
import type { ConfiguredLoopRequest, LoopResolution } from "./generated/ProjectLoopsResult";

/** The workspace's loop to open, as the engine decides it: the one to open, a file to repair, a missing file, or a refusal. */
type WorkspaceLoop =
  | { kind: "cancelled" }
  | { kind: "accepted"; request: ConfiguredLoopRequest }
  | { kind: "invalid"; reason: string; configurationPath: string }
  | { kind: "missing"; reason: string };

/** Command discovery is separate from the bound editor's lifecycle and state. */
export async function openControlStudioCommand(
  context: vscode.ExtensionContext,
  request: unknown,
  show: (context: vscode.ExtensionContext, request: unknown) => void | Promise<void>,
  presenter?: SetupPromptPresenter
): Promise<void> {
  if (request !== undefined) {
    await show(context, request);
    return;
  }
  const resolution = await workspaceLoop(context, getSelectedRuntimeId());
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
    await configureControlStudioCommand(context, show, presenter);
    return;
  }
  if (action === "Choose an example") {
    await vscode.commands.executeCommand("trust.examples.start");
  }
}

/**
 * The loop the workspace opens. The engine reads every folder's loop files and decides which one opens, or which of several the engineer
 * chooses among (`controlStudio/projectLoops`, `open` and `choices`); the extension shows that decision and nothing else.
 */
async function workspaceLoop(context: vscode.ExtensionContext, runtimeId: string): Promise<WorkspaceLoop> {
  const folders = vscode.workspace.workspaceFolders ?? [];
  const answer = await projectLoops(createEngineClient(controlStudioEngine(context)), {
    projectRoots: folders.map(folder => folder.uri.fsPath), runtimeId,
  });
  if ("failure" in answer) return { kind: "missing", reason: refusalText(answer.failure) };
  const { open, choices = [] } = answer.result;
  if (open) return workspaceOf(open.resolution);
  const selected = await vscode.window.showQuickPick(choices.map(choice => ({
    label: choice.resolution.request ? choice.resolution.request.loopDefinition.name : "Invalid loop configuration",
    description: choice.projectRoot,
    detail: choice.resolution.state === "invalid" ? choice.resolution.reason ?? undefined : choice.projectRoot,
    workspace: workspaceOf(choice.resolution),
  })), { title: "Open Control Studio", placeHolder: "Choose a project loop" });
  return selected?.workspace ?? { kind: "cancelled" };
}

function workspaceOf(resolution: LoopResolution): WorkspaceLoop {
  if (resolution.request) return { kind: "accepted", request: resolution.request };
  if (resolution.state === "invalid") return { kind: "invalid", reason: resolution.reason ?? "", configurationPath: resolution.configurationPath };
  return { kind: "missing", reason: resolution.reason ?? "" };
}


export async function configureControlStudioCommand(
  context: vscode.ExtensionContext,
  show: (context: vscode.ExtensionContext, request: unknown) => void | Promise<void>,
  presenter?: SetupPromptPresenter
): Promise<{ opened: boolean; reason?: string }> {
  const folders = vscode.workspace.workspaceFolders ?? [];
  const folder = folders.length === 1 ? folders[0] : (await vscode.window.showQuickPick(
    folders.map(item => ({ label: item.name, description: item.uri.fsPath, folder: item })),
    { title: "Configure control loop", placeHolder: "Choose the project containing your controller" }))?.folder;
  if (!folder) return { opened: false, reason: "cancelled" };
  // The engine reads the compiled project and refuses one that does not compile (setupDiscover), so the form needs no catalog here.
  const panel = await openLoopSetupPanel(context, folder.uri, getSelectedRuntimeId(), request => show(context, request), async () => {
    // Controllers the form does not set up yet keep the step-by-step setup, whose questions the engine names.
    const configured = await configureControlLoop(context, getSelectedRuntimeId(), presenter);
    if (configured.kind === "accepted") await show(context, configured.request);
    else if (configured.kind !== "cancelled") void vscode.window.showErrorMessage(configured.reason);
  });
  return { opened: panel !== undefined };
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
