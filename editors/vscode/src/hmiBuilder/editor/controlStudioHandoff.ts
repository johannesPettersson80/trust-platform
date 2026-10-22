import * as vscode from "vscode";
import { getSelectedRuntimeId } from "../../selectedRuntime";
import { createEngineClient } from "../../controlStudio/engine/engineClient";
import { controlStudioEngine } from "../../controlStudio/engine/enginePanelRoute";
import { projectLoops } from "../../controlStudio/engine/engineProjectLoops";
import { refusalText } from "../../controlStudio/loopSetup/setupEngine";
import type { HmiObjectModel } from "../drawing/model";

/** Open only an existing validated configuration for this equipment and the selected target. The engine decides which loops belong to it. */
export async function openEquipmentControlStudio(
  context: vscode.ExtensionContext,
  project: vscode.Uri,
  object: HmiObjectModel,
): Promise<void> {
  if (object.type !== "faceplate" || !object.instance) return;
  const folder = vscode.workspace.workspaceFolders?.find((folder) => folder.uri.toString() === project.toString());
  if (!folder) {
    await vscode.window.showWarningMessage("Open this HMI's project before opening Control Studio.");
    return;
  }
  const answer = await projectLoops(createEngineClient(controlStudioEngine(context)), {
    projectRoots: [folder.uri.fsPath],
    runtimeId: getSelectedRuntimeId(),
    equipment: object.instance,
  });
  if ("failure" in answer) {
    await vscode.window.showWarningMessage(refusalText(answer.failure));
    return;
  }
  const matches = answer.result.equipment?.requests ?? [];
  const request =
    matches.length === 1
      ? matches[0]
      : matches.length > 1
        ? (
            await vscode.window.showQuickPick(
              matches.map((request) => ({
                label: request.loopDefinition.name,
                description: request.loopDefinition.controller.instance,
                request,
              })),
              { title: "Open equipment in Control Studio" },
            )
          )?.request
        : undefined;
  if (request) {
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", request);
    return;
  }
  if (matches.length) return;
  await vscode.window.showWarningMessage(
    answer.result.equipment?.reason ?? "Control Studio has no loop for this equipment.",
  );
}
