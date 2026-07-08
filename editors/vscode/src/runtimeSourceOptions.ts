import { getTrustConfiguration } from "./configuration";
import * as vscode from "vscode";

const DEFAULT_RUNTIME_INCLUDE_GLOBS = ["**/*.{st,ST,pou,POU}"];

export type RuntimeSourceOptions = {
  runtimeIncludeGlobs?: string[];
  runtimeExcludeGlobs?: string[];
  runtimeIgnorePragmas?: string[];
  runtimeRoot?: string;
};

type RuntimeSourceOptionInputs = {
  includeGlobs: unknown;
  excludeGlobs: unknown;
  ignorePragmas: unknown;
  runtimeRoot?: string;
};

export function normalizeStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .map((item) => (typeof item === "string" ? item.trim() : ""))
    .filter((item) => item.length > 0);
}

export function buildRuntimeSourceOptions(
  inputs: RuntimeSourceOptionInputs
): RuntimeSourceOptions {
  const includeGlobs = normalizeStringArray(inputs.includeGlobs);
  const effectiveIncludeGlobs =
    includeGlobs.length > 0 ? includeGlobs : DEFAULT_RUNTIME_INCLUDE_GLOBS;
  return {
    runtimeIncludeGlobs: effectiveIncludeGlobs,
    runtimeExcludeGlobs: normalizeStringArray(inputs.excludeGlobs),
    runtimeIgnorePragmas: normalizeStringArray(inputs.ignorePragmas),
    runtimeRoot: inputs.runtimeRoot,
  };
}

export function runtimeSourceOptionsForTarget(
  target?: vscode.Uri
): RuntimeSourceOptions {
  const config = getTrustConfiguration();
  const folder = target
    ? vscode.workspace.getWorkspaceFolder(target)
    : vscode.workspace.workspaceFolders?.[0];
  return buildRuntimeSourceOptions({
    includeGlobs: config.get<unknown>("runtime.includeGlobs"),
    excludeGlobs: config.get<unknown>("runtime.excludeGlobs"),
    ignorePragmas: config.get<unknown>("runtime.ignorePragmas"),
    runtimeRoot: folder?.uri.fsPath,
  });
}

