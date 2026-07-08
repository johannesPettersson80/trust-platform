import * as vscode from "vscode";

export interface WorkspaceProjectState {
  readonly kind: "none" | "nonTrust" | "trust" | "malformed";
  readonly folder?: vscode.WorkspaceFolder;
  readonly issue?: string;
}

// "Project open" = a workspace folder is open AND it has a readable truST project manifest. Keep
// "no folder", "non-truST folder", and "malformed manifest" distinct so UI surfaces can explain
// exactly what happened instead of showing Run/Start for a project that cannot load.
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
  try {
    const bytes = await vscode.workspace.fs.readFile(manifest);
    const text = Buffer.from(bytes).toString("utf8");
    const manifestIssue = manifestReadabilityIssue(text);
    if (manifestIssue) {
      return { kind: "malformed", folder, issue: manifestIssue };
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { kind: "malformed", folder, issue: message };
  }
  return { kind: "trust", folder };
}

export async function workspaceHasReadableTrustProject(): Promise<boolean> {
  return (await getWorkspaceProjectState()).kind === "trust";
}

export function manifestReadabilityIssue(text: string): string | undefined {
  for (const [index, raw] of text.split(/\r?\n/).entries()) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) {
      continue;
    }
    if (
      line.startsWith("[") &&
      !/^\[\[?[A-Za-z0-9_.-]+(?:\.[A-Za-z0-9_.-]+)*\]?\]$/.test(line)
    ) {
      return `Project settings header on line ${index + 1} is incomplete.`;
    }
  }
  return undefined;
}
