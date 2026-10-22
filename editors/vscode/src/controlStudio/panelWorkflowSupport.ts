import * as vscode from "vscode";
import { randomUUID } from "crypto";
import type { LoopDefinition } from "./generated/ProjectLoopsResult";

/** Posts an engineering answer of a stage to the page, in the envelope the page checks against the loop's scope (B1.2). */
export async function postEngineeringResult(panel: vscode.WebviewPanel | undefined,
  loop: Pick<LoopDefinition, "projectId" | "id" | "revision"> | undefined, editorSessionId: string, stage: string, payload: unknown): Promise<void> {
  if (!panel || !loop) return;
  await panel.webview.postMessage({ schemaVersion: 1, kind: "control-studio.engineering.result", messageId: randomUUID(),
    projectId: loop.projectId, editorSessionId, loopId: loop.id, expectedLoopRevision: loop.revision, stage, payload });
}
