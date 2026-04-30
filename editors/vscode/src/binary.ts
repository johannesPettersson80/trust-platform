import * as path from "path";
import * as fs from "fs";
import * as os from "os";
import * as vscode from "vscode";

/**
 * Resolves the path to a binary, checking in order:
 * 1. User-configured path from settings
 * 2. Bundled binary in extension's bin/ directory
 * 3. Binary name for PATH lookup (development fallback)
 */
export function getBinaryPath(
  context: vscode.ExtensionContext,
  binaryName: string,
  configKey: string
): string {
  const config = vscode.workspace.getConfiguration("trust-lsp");
  const configuredRaw = (config.get<string>(configKey) ?? "").trim();

  // 1. User-configured path takes precedence
  if (configuredRaw) {
    return resolveConfiguredPath(configuredRaw);
  }

  const isDevMode =
    context.extensionMode === vscode.ExtensionMode.Development ||
    context.extensionMode === vscode.ExtensionMode.Test;
  if (isDevMode) {
    const devBinary = getDevelopmentBinaryPath(context, binaryName);
    if (devBinary && fs.existsSync(devBinary)) {
      return devBinary;
    }
    // In extension development prefer PATH over stale bundled binaries.
    return binaryName;
  }

  // 2. Look for bundled binary in extension
  const bundledPath = getBundledBinaryPath(context, binaryName);
  if (bundledPath && fs.existsSync(bundledPath)) {
    return bundledPath;
  }

  // 3. Fall back to PATH (for development)
  return binaryName;
}

/**
 * Returns the path where a bundled binary would be located.
 */
export function getBundledBinaryPath(
  context: vscode.ExtensionContext,
  binaryName: string
): string {
  const suffix = process.platform === "win32" ? ".exe" : "";
  return path.join(context.extensionPath, "bin", `${binaryName}${suffix}`);
}

/**
 * Resolves a user-configured path, expanding ~ and workspace variables.
 */
function resolveConfiguredPath(value: string): string {
  let normalized = value.trim();

  // Strip surrounding quotes
  if (
    (normalized.startsWith("\"") && normalized.endsWith("\"")) ||
    (normalized.startsWith("'") && normalized.endsWith("'"))
  ) {
    normalized = normalized.slice(1, -1);
  }

  // Expand workspace variables
  const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  if (workspaceRoot) {
    normalized = normalized.replace("${workspaceFolder}", workspaceRoot);
    normalized = normalized.replace("${workspaceRoot}", workspaceRoot);
  }

  // Expand home directory
  if (normalized.startsWith("~")) {
    normalized = path.join(os.homedir(), normalized.slice(1));
  }

  return normalized;
}

function getDevelopmentBinaryPath(
  context: vscode.ExtensionContext,
  binaryName: string
): string | undefined {
  const suffix = process.platform === "win32" ? ".exe" : "";
  const repoRoot = path.resolve(context.extensionPath, "..", "..");
  const debugCandidate = path.join(
    repoRoot,
    "target",
    "debug",
    `${binaryName}${suffix}`
  );
  if (fs.existsSync(debugCandidate)) {
    return debugCandidate;
  }
  const releaseCandidate = path.join(
    repoRoot,
    "target",
    "release",
    `${binaryName}${suffix}`
  );
  if (fs.existsSync(releaseCandidate)) {
    return releaseCandidate;
  }
  return undefined;
}
