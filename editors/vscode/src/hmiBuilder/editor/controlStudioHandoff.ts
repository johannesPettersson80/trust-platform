import * as vscode from "vscode";
import { getSelectedRuntimeId } from "../../selectedRuntime";
import { controlStudioRequestsForFolder } from "../../controlStudio/projectLoop";
import type { HmiObjectModel } from "../drawing/model";

export function controllerBelongsToEquipment(controller: string, equipment: string): boolean {
  const actual = controller.toLowerCase(),
    selected = equipment.toLowerCase();
  return actual === selected || actual.startsWith(selected + ".");
}
/** Open only an existing validated configuration for this equipment and the selected target. */
export async function openEquipmentControlStudio(project: vscode.Uri, object: HmiObjectModel): Promise<void> {
  if (object.type !== "faceplate" || !object.instance) return;
  const folder = vscode.workspace.workspaceFolders?.find((folder) => folder.uri.toString() === project.toString());
  if (!folder) {
    await vscode.window.showWarningMessage("Open this HMI's project before opening Control Studio.");
    return;
  }
  const resolutions = await controlStudioRequestsForFolder(folder, getSelectedRuntimeId());
  const matches = resolutions.flatMap((result) =>
    result.kind === "accepted" &&
    controllerBelongsToEquipment(result.request.loopDefinition.controller.instance, object.instance!)
      ? [result.request]
      : [],
  );
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
  const issue = resolutions.find((result) => result.kind === "invalid");
  await vscode.window.showWarningMessage(
    issue?.kind === "invalid"
      ? issue.reason
      : `No configured Control Studio loop matches ${object.instance} on the selected runtime. Configure its actual controller signals first.`,
  );
}
