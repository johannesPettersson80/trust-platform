// Responsibility: focused LM tools module with a single concern.
import * as path from "path";
import * as vscode from "vscode";

export function uriFromFilePath(filePath: string): vscode.Uri | undefined {
  const trimmed = filePath.trim();
  if (!trimmed) {
    return undefined;
  }
  if (trimmed.startsWith("file://")) {
    try {
      return vscode.Uri.parse(trimmed);
    } catch {
      return undefined;
    }
  }
  const hasScheme = /^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(trimmed);
  const isWindowsPath = /^[a-zA-Z]:[\\/]/.test(trimmed);
  if (hasScheme && !isWindowsPath) {
    try {
      return vscode.Uri.parse(trimmed);
    } catch {
      return undefined;
    }
  }
  if (!path.isAbsolute(trimmed)) {
    return undefined;
  }
  return vscode.Uri.file(trimmed);
}

export function optionalUriFromFilePath(
  filePath: string | undefined,
): vscode.Uri | undefined {
  if (!filePath) {
    return undefined;
  }
  const trimmed = filePath.trim();
  if (!trimmed) {
    return undefined;
  }
  return uriFromFilePath(trimmed);
}

export function isPathInside(base: string, target: string): boolean {
  const rel = path.relative(base, target);
  return rel === "" || (!rel.startsWith("..") && !path.isAbsolute(rel));
}

export function ensureWorkspaceUri(uri: vscode.Uri): string | undefined {
  const folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length === 0) {
    return "No workspace is open.";
  }
  const normalizedTarget = path.normalize(uri.fsPath);
  for (const folder of folders) {
    const normalizedBase = path.normalize(folder.uri.fsPath);
    if (isPathInside(normalizedBase, normalizedTarget)) {
      return undefined;
    }
  }
  return "filePath must be inside the current workspace.";
}
