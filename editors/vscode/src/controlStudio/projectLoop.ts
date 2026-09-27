import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { decodeControlLoopDefinition, validateControlLoopDefinition, type ControlLoopDefinition } from "./loopDefinition";

export type ProjectControlLoopResolution =
  | { kind: "accepted"; request: ProjectControlLoopOpenRequest }
  | { kind: "cancelled" }
  | { kind: "missing"; reason: string }
  | { kind: "invalid"; reason: string; configurationPath: string };

export interface ProjectControlLoopOpenRequest {
  schemaVersion: 1;
  source: "ConfiguredLoop";
  scope: { projectId: string; runtimeId: string };
  loopDefinition: ControlLoopDefinition;
}

/** Reads the project's explicit loop contract; signal names are never guessed from ST. */
export async function controlStudioRequestForWorkspace(
  selectedRuntimeId: string,
  folders: readonly vscode.WorkspaceFolder[] = vscode.workspace.workspaceFolders ?? []
): Promise<ProjectControlLoopResolution> {
  if (folders.length === 0) {
    return { kind: "missing", reason: "Open a truST project before opening Control Studio." };
  }
  const projects = (await Promise.all(folders.map(async (folder) =>
    (await controlStudioRequestsForFolder(folder, selectedRuntimeId)).map((resolution) => ({ folder, resolution }))
  ))).flat();
  const configured = projects.filter(({ resolution }) => resolution.kind !== "missing");
  if (configured.length === 0) return projects[0].resolution;
  if (configured.length === 1) return configured[0].resolution;
  const selected = await vscode.window.showQuickPick(configured.map(({ folder, resolution }) => ({
    label: resolution.kind === "accepted" ? resolution.request.loopDefinition.name : "Invalid loop configuration",
    description: folder.name,
    detail: resolution.kind === "invalid" ? resolution.reason : folder.uri.fsPath,
    resolution,
  })), { title: "Open Control Studio", placeHolder: "Choose a project loop" });
  return selected?.resolution ?? { kind: "cancelled" };
}

async function controlStudioRequestsForFolder(
  folder: vscode.WorkspaceFolder,
  selectedRuntimeId: string
): Promise<ProjectControlLoopResolution[]> {
  let paths: string[];
  try {
    paths = await controlLoopConfigurationPaths(folder.uri.fsPath);
  } catch {
    return [{ kind: "invalid", reason: "The control-loops directory could not be read.",
      configurationPath: path.join(folder.uri.fsPath, "control-loops") }];
  }
  const resolutions = await Promise.all(paths.map((file) => controlStudioRequestForFile(file, selectedRuntimeId)));
  const ids = new Set<string>();
  for (let index = 0; index < resolutions.length; index += 1) {
    const resolution = resolutions[index];
    if (resolution.kind !== "accepted") continue;
    const id = resolution.request.loopDefinition.id;
    if (ids.has(id)) {
      return [{ kind: "invalid", reason: `Duplicate control loop ID ${id} in this project. Give each loop a unique ID.`, configurationPath: paths[index] }];
    }
    ids.add(id);
  }
  const configured = resolutions.filter((resolution) => resolution.kind !== "missing");
  return configured.length ? configured : [resolutions[0]];
}

async function controlStudioRequestForFile(
  filePath: string,
  selectedRuntimeId: string
): Promise<ProjectControlLoopResolution> {
  let value: unknown;
  try {
    value = JSON.parse(await fs.promises.readFile(filePath, "utf8"));
  } catch (error) {
    if (isMissingFile(error)) {
      return {
        kind: "missing",
        reason: "This project has no loop definition in control-loop.json or control-loops/. Configure a loop to bind its signals.",
      };
    }
    return { kind: "invalid", reason: `${path.basename(filePath)} could not be read as valid JSON.`, configurationPath: filePath };
  }
  const decoded = decodeControlLoopDefinition(value);
  if (decoded.kind !== "accepted") {
    return { kind: "invalid", reason: `${path.basename(filePath)} does not contain a complete Control Studio loop definition.`, configurationPath: filePath };
  }
  if (decoded.loop.runtimeId !== selectedRuntimeId) {
    return {
      kind: "invalid",
      configurationPath: filePath,
      reason: `The configured loop uses ${decoded.loop.runtimeId}, but ${selectedRuntimeId} is the selected target.`,
    };
  }
  const validated = validateControlLoopDefinition(decoded.loop, {
    projectId: decoded.loop.projectId, runtimeId: selectedRuntimeId,
  });
  if (validated.kind !== "accepted") {
    const fields = "signals" in validated ? validated.signals.join(" and ")
      : "signal" in validated ? String(validated.signal) : "loop configuration";
    const detail = validated.reason === "incompatible_units"
      ? `${fields} must use matching engineering units`
      : `${fields}: ${validated.reason.replace(/_/g, " ")}`;
    return { kind: "invalid", reason: `${path.basename(filePath)}: ${detail}.`, configurationPath: filePath };
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


export async function controlLoopConfigurationPaths(projectRoot: string): Promise<string[]> {
  const rootFile = path.join(projectRoot, "control-loop.json");
  const directory = path.join(projectRoot, "control-loops");
  try {
    const additional = (await fs.promises.readdir(directory, { withFileTypes: true }))
      .filter(entry => entry.isFile() && entry.name.endsWith(".json"))
      .map(entry => path.join(directory, entry.name)).sort();
    return [rootFile, ...additional];
  } catch (error) {
    if (isMissingFile(error)) return [rootFile];
    throw error;
  }
}

export async function assertControlLoopIdAvailable(projectRoot: string, id: string): Promise<void> {
  for (const file of await controlLoopConfigurationPaths(projectRoot)) {
    let content: string;
    try { content = await fs.promises.readFile(file, "utf8"); }
    catch (error) { if (isMissingFile(error)) continue; throw error; }
    let value: unknown;
    try { value = JSON.parse(content); }
    catch { throw new Error(`Repair ${path.basename(file)} before adding a control loop: its JSON is invalid.`); }
    if (value && typeof value === "object" && (value as { id?: unknown }).id === id) {
      throw new Error(`Duplicate control loop ID ${id} in ${path.basename(file)}. Choose a different loop name.`);
    }
  }
}
