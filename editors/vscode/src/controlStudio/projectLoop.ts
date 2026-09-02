import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { decodeControlLoopDefinition, type ControlLoopDefinition } from "./loopDefinition";

export type ProjectControlLoopResolution =
  | { kind: "accepted"; request: ProjectControlLoopOpenRequest }
  | { kind: "missing"; reason: string }
  | { kind: "invalid"; reason: string };

export interface ProjectControlLoopOpenRequest {
  schemaVersion: 1;
  source: "ConfiguredLoop";
  scope: { projectId: string; runtimeId: string };
  loopDefinition: ControlLoopDefinition;
}

/** Reads the project's explicit loop contract; signal names are never guessed from ST. */
export async function controlStudioRequestForWorkspace(
  selectedRuntimeId: string
): Promise<ProjectControlLoopResolution> {
  const folder = vscode.workspace.workspaceFolders?.[0];
  if (!folder) {
    return { kind: "missing", reason: "Open a truST project before opening Control Studio." };
  }
  const filePath = path.join(folder.uri.fsPath, "control-loop.json");
  let value: unknown;
  try {
    value = JSON.parse(await fs.promises.readFile(filePath, "utf8"));
  } catch (error) {
    if (isMissingFile(error)) {
      return {
        kind: "missing",
        reason: "This project has no control-loop.json, so Control Studio cannot bind SP, PV, and MV safely.",
      };
    }
    return { kind: "invalid", reason: "control-loop.json is not valid JSON." };
  }
  const decoded = decodeControlLoopDefinition(value);
  if (decoded.kind !== "accepted") {
    return { kind: "invalid", reason: "control-loop.json does not contain a complete Control Studio loop definition." };
  }
  if (decoded.loop.runtimeId !== selectedRuntimeId) {
    return {
      kind: "invalid",
      reason: `The configured loop uses ${decoded.loop.runtimeId}, but ${selectedRuntimeId} is the selected target.`,
    };
  }
  return {
    kind: "accepted",
    request: {
      schemaVersion: 1,
      source: "ConfiguredLoop",
      scope: { projectId: decoded.loop.projectId, runtimeId: decoded.loop.runtimeId },
      loopDefinition: decoded.loop,
    },
  };
}

function isMissingFile(error: unknown): boolean {
  return error instanceof Error && "code" in error && error.code === "ENOENT";
}
