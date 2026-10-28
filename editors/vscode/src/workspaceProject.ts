import * as vscode from "vscode";

import { projectRequest } from "./projectRequest";

export interface WorkspaceProjectState {
  readonly kind: "none" | "nonTrust" | "trust" | "malformed";
  readonly folder?: vscode.WorkspaceFolder;
  readonly issue?: string;
}

// "Project open" = a workspace folder is open AND it has a readable truST project manifest. Keep
// "no folder", "non-truST folder", and "malformed manifest" distinct so UI surfaces can explain
// exactly what happened instead of showing Run/Start for a project that cannot load. Whether the
// manifest can be read is the language server's answer (`trust-lsp.manifestCheck`, specification 25
// §6.2); the first check waits a bounded time for the server to start, and a server that never starts
// leaves the manifest counted as readable.
export async function getWorkspaceProjectState(): Promise<WorkspaceProjectState> {
  const folder = vscode.workspace.workspaceFolders?.[0];
  if (!folder) {
    return { kind: "none" };
  }
  const found = await vscode.workspace.findFiles(
    "**/trust-lsp.toml",
    "**/node_modules/**",
    1
  );
  if (found.length === 0) {
    return { kind: "nonTrust", folder };
  }
  const manifest = found[0];
  let text: string;
  try {
    const bytes = await vscode.workspace.fs.readFile(manifest);
    text = Buffer.from(bytes).toString("utf8");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { kind: "malformed", folder, issue: message };
  }
  const answer = await projectRequest<{ issue: string | null }>(
    "trust-lsp.manifestCheck",
    { text }
  );
  if (answer.ok && answer.issue) {
    return { kind: "malformed", folder, issue: answer.issue };
  }
  return { kind: "trust", folder };
}

export async function workspaceHasReadableTrustProject(): Promise<boolean> {
  return (await getWorkspaceProjectState()).kind === "trust";
}
