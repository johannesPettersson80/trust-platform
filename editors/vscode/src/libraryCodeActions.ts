import * as vscode from "vscode";

import { ADD_LIBRARY_COMMAND } from "./libraries";
import { parseDependencyEntries } from "./librariesModel";

type CuratedLibraryId = "oscat" | "plcopen_motion";

export interface LibraryCodeActionCandidate {
  readonly id: CuratedLibraryId;
  readonly label: string;
  readonly dependencyName: string;
}

const CURATED_SYMBOLS: readonly (LibraryCodeActionCandidate & {
  readonly matches: readonly RegExp[];
})[] = [
  {
    id: "oscat",
    label: "OSCAT",
    dependencyName: "OSCAT",
    matches: [/^INC$/i, /^SCALE$/i, /^BIT_COUNT$/i],
  },
  {
    id: "plcopen_motion",
    label: "PLCopen Motion",
    dependencyName: "PLCopenMotionSingleAxis",
    matches: [/^MC_[A-Za-z0-9_]+$/],
  },
];

export function curatedLibraryActionForSymbol(
  symbol: string,
  existingDependencyNames: readonly string[]
): LibraryCodeActionCandidate | undefined {
  const candidate = CURATED_SYMBOLS.find((library) =>
    library.matches.some((pattern) => pattern.test(symbol))
  );
  if (!candidate || existingDependencyNames.includes(candidate.dependencyName)) {
    return undefined;
  }
  return {
    id: candidate.id,
    label: candidate.label,
    dependencyName: candidate.dependencyName,
  };
}

export function registerLibraryCodeActions(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.languages.registerCodeActionsProvider(
      [{ scheme: "file", language: "structured-text" }],
      {
        async provideCodeActions(document, range) {
          const symbol = symbolAtRange(document, range);
          if (!symbol) {
            return [];
          }
          const root = vscode.workspace.getWorkspaceFolder(document.uri);
          if (!root) {
            return [];
          }
          const dependencies = await dependencyNames(root.uri);
          const candidate = curatedLibraryActionForSymbol(symbol, dependencies);
          if (!candidate) {
            return [];
          }
          const action = new vscode.CodeAction(
            `Add ${candidate.label} to this project`,
            vscode.CodeActionKind.QuickFix
          );
          action.command = {
            command: ADD_LIBRARY_COMMAND,
            title: action.title,
            arguments: [{ source: { kind: "curated", id: candidate.id } }],
          };
          return [action];
        },
      },
      { providedCodeActionKinds: [vscode.CodeActionKind.QuickFix] }
    )
  );
}

function symbolAtRange(document: vscode.TextDocument, range: vscode.Range): string | undefined {
  const wordRange = document.getWordRangeAtPosition(
    range.start,
    /[A-Za-z_][A-Za-z0-9_]*/
  );
  const symbol = wordRange ? document.getText(wordRange).trim() : "";
  return symbol.length > 0 ? symbol : undefined;
}

async function dependencyNames(root: vscode.Uri): Promise<string[]> {
  try {
    const bytes = await vscode.workspace.fs.readFile(vscode.Uri.joinPath(root, "trust-lsp.toml"));
    return parseDependencyEntries(Buffer.from(bytes).toString("utf8")).map((dep) => dep.name);
  } catch {
    return [];
  }
}
