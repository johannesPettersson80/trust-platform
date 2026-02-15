import * as vscode from "vscode";

export function asUri(value?: vscode.Uri | string): vscode.Uri | undefined {
  if (!value) {
    return undefined;
  }

  if (value instanceof vscode.Uri) {
    return value;
  }

  try {
    if (value.includes("://")) {
      return vscode.Uri.parse(value);
    }
    return vscode.Uri.file(value);
  } catch {
    return undefined;
  }
}

export async function pathExists(uri: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(uri);
    return true;
  } catch {
    return false;
  }
}
