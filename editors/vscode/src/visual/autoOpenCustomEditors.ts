import { affectsTrustConfiguration, getTrustConfiguration } from "../configuration";
import * as vscode from "vscode";

interface VisualEditorBinding {
  suffix: string;
  viewType: string;
}

const VISUAL_EDITOR_BINDINGS: VisualEditorBinding[] = [
  { suffix: ".ladder.json", viewType: "trust-lsp.ladder.editor" },
  { suffix: ".statechart.json", viewType: "trust-lsp.statechartEditor" },
  { suffix: ".blockly.json", viewType: "trust-lsp.blockly.editor" },
];

function viewTypeForUri(uri: vscode.Uri): string | undefined {
  const normalized = uri.path.toLowerCase();
  const binding = VISUAL_EDITOR_BINDINGS.find((entry) =>
    normalized.endsWith(entry.suffix)
  );
  return binding?.viewType;
}

function isVisualAutoOpenEnabled(): boolean {
  return getTrustConfiguration().get<boolean>(
    "visual.autoOpenCustomEditors",
    true
  );
}

function isAlreadyOpenWithViewType(uri: vscode.Uri, viewType: string): boolean {
  return vscode.window.tabGroups.all.some((group) =>
    group.tabs.some((tab) => {
      if (!(tab.input instanceof vscode.TabInputCustom)) {
        return false;
      }
      return (
        tab.input.uri.toString() === uri.toString() &&
        tab.input.viewType === viewType
      );
    })
  );
}

export function registerVisualCustomEditorAutoOpen(): vscode.Disposable {
  const inFlight = new Set<string>();
  const openedThisSession = new Set<string>();

  const maybeOpen = async (uri: vscode.Uri): Promise<void> => {
    if (!isVisualAutoOpenEnabled()) {
      return;
    }
    if (uri.scheme !== "file") {
      return;
    }

    const viewType = viewTypeForUri(uri);
    if (!viewType) {
      return;
    }

    const key = `${uri.toString()}::${viewType}`;
    if (inFlight.has(key) || openedThisSession.has(key)) {
      return;
    }

    if (isAlreadyOpenWithViewType(uri, viewType)) {
      openedThisSession.add(key);
      return;
    }

    inFlight.add(key);
    try {
      await vscode.commands.executeCommand(
        "vscode.openWith",
        uri,
        viewType,
        {
          preview: false,
        }
      );
      openedThisSession.add(key);
    } catch (error) {
      console.warn(
        `[trust-lsp] Failed to auto-open ${uri.toString()} with ${viewType}:`,
        error
      );
    } finally {
      inFlight.delete(key);
    }
  };

  const openVisibleEditors = (): void => {
    for (const editor of vscode.window.visibleTextEditors) {
      void maybeOpen(editor.document.uri);
    }
  };

  openVisibleEditors();

  const subscriptions: vscode.Disposable[] = [
    vscode.workspace.onDidOpenTextDocument((document) => {
      void maybeOpen(document.uri);
    }),
    vscode.window.onDidChangeVisibleTextEditors((editors) => {
      for (const editor of editors) {
        void maybeOpen(editor.document.uri);
      }
    }),
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      if (editor) {
        void maybeOpen(editor.document.uri);
      }
    }),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (
        affectsTrustConfiguration(event, "visual.autoOpenCustomEditors") &&
        isVisualAutoOpenEnabled()
      ) {
        openVisibleEditors();
      }
    }),
  ];

  return vscode.Disposable.from(...subscriptions);
}
