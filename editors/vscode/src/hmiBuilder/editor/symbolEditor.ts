// The symbol editor changes its normal TOML text document; the runtime owns the format (§14.4.7).
import * as path from "path";
import * as vscode from "vscode";
import type { CustomSymbolModel } from "../drawing/symbols/custom";
import { languageServerScreen } from "./languageServerPageFiles";
import { sameModel, smallestEdit } from "./pageDocument";

export function isSymbolDefinition(uri: vscode.Uri): boolean {
  return /[\\/]hmi[\\/]symbols[\\/][A-Za-z0-9_-]+\.toml$/.test(uri.fsPath);
}

export function resolveSymbolEditor(document: vscode.TextDocument, panel: vscode.WebviewPanel): void {
  const drawingUri = vscode.Uri.file(document.uri.fsPath.replace(/\.toml$/, ".svg"));
  const project = vscode.Uri.file(path.dirname(path.dirname(path.dirname(document.uri.fsPath))));
  let open = true;
  let loaded: CustomSymbolModel | undefined;
  let applying = false;
  const post = (message: unknown) => {
    if (open) void panel.webview.postMessage(message);
  };
  const drawing = async () => (await vscode.workspace.openTextDocument(drawingUri)).getText();
  const failed = (error: unknown) =>
    post({ type: "invalid", errors: [{ message: error instanceof Error ? error.message : String(error) }] });
  const refresh = async () => {
    const version = document.version;
    loaded = undefined;
    try {
      const answer = await vscode.commands.executeCommand<{
        ok: boolean;
        model?: CustomSymbolModel;
        errors?: unknown[];
      }>("trust-lsp.hmiFileParse", {
        kind: "symbol",
        text: document.getText(),
        file: document.uri.fsPath,
        drawing: await drawing(),
      });
      if (!open || version !== document.version) return;
      if (answer?.ok && answer.model) {
        loaded = answer.model;
        const screen = await languageServerScreen(vscode.Uri.joinPath(project, "hmi", "screen.toml"));
        if (version === document.version) post({ type: "symbol", model: loaded, theme: screen.theme });
      } else post({ type: "invalid", errors: answer?.errors ?? [{ message: "The symbol could not be loaded." }] });
    } catch (error) {
      failed(error);
    }
  };
  const apply = async (model: CustomSymbolModel) => {
    if (!loaded || applying || sameModel(model, loaded)) return;
    applying = true;
    const version = document.version;
    try {
      const answer = await vscode.commands.executeCommand<{ ok: boolean; text?: string; errors?: unknown[] }>(
        "trust-lsp.hmiFileFormat",
        {
          kind: "symbol",
          model: { ...model, name: loaded.name },
          drawing: await drawing(),
        },
      );
      if (!open || version !== document.version) return;
      if (!answer?.ok || answer.text === undefined) {
        post({ type: "editRefused", errors: answer?.errors ?? [{ message: "The symbol could not be written." }] });
        return;
      }
      const replacement = smallestEdit(document.getText(), answer.text);
      if (!replacement) return;
      const position = (line: number) =>
        line < document.lineCount ? new vscode.Position(line, 0) : document.lineAt(document.lineCount - 1).range.end;
      const edit = new vscode.WorkspaceEdit();
      edit.replace(
        document.uri,
        new vscode.Range(position(replacement.startLine), position(replacement.endLine)),
        replacement.text,
      );
      await vscode.workspace.applyEdit(edit);
      await refresh();
    } catch (error) {
      failed(error);
    } finally {
      applying = false;
    }
  };
  const subscriptions = [
    panel.webview.onDidReceiveMessage((message) => {
      if (message.type === "ready") void refresh();
      if (message.type === "editSymbol") void apply(message.model);
      if (message.type === "openAsText")
        void vscode.commands.executeCommand("vscode.openWith", document.uri, "default");
    }),
    vscode.workspace.onDidChangeTextDocument((event) => {
      if (
        event.document.uri.toString() === document.uri.toString() ||
        event.document.uri.toString() === drawingUri.toString()
      )
        void refresh();
    }),
  ];
  panel.onDidDispose(() => {
    open = false;
    subscriptions.forEach((subscription) => subscription.dispose());
  });
}
