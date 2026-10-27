import { getTrustConfiguration } from "./configuration";
import * as vscode from "vscode";

export type RuntimeSourceOptions = {
  runtimeIncludeGlobs?: string[];
  runtimeExcludeGlobs?: string[];
  runtimeIgnorePragmas?: string[];
  runtimeRoot?: string;
};

// The runtime-source settings as the user wrote them. Rust trims each entry, drops blank ones and
// applies the default include glob (trust-runtime `host/runtime_sources.rs`).
export function runtimeSourceOptionsForTarget(
  target?: vscode.Uri
): RuntimeSourceOptions {
  const config = getTrustConfiguration();
  const folder = target
    ? vscode.workspace.getWorkspaceFolder(target)
    : vscode.workspace.workspaceFolders?.[0];
  return {
    runtimeIncludeGlobs: strings(config.get<unknown>("runtime.includeGlobs")),
    runtimeExcludeGlobs: strings(config.get<unknown>("runtime.excludeGlobs")),
    runtimeIgnorePragmas: strings(config.get<unknown>("runtime.ignorePragmas")),
    runtimeRoot: folder?.uri.fsPath,
  };
}

// The string entries of a setting, for the JSON the Rust side reads.
function strings(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}
