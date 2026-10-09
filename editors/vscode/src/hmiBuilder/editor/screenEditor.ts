// Screen settings use the native screen parser/writer and ordinary unsaved editor edits.
import * as vscode from "vscode";
import { listPages } from "./pages";
import { languageServerTexts } from "./textFiles";
import { sameModel, smallestEdit } from "./pageDocument";
import { ADD_USER_COMMAND, projectUsers, REMOVE_USER_COMMAND } from "./hmiUsers";

export function isScreenDefinition(uri: vscode.Uri): boolean {
  return /\/hmi\/screen\.toml$/.test(uri.path);
}

export function resolveScreenEditor(document: vscode.TextDocument, panel: vscode.WebviewPanel): void {
  const project = document.uri.with({ path: document.uri.path.replace(/\/hmi\/screen\.toml$/, "") });
  const runtimeToml = vscode.Uri.joinPath(project, "runtime.toml").toString();
  let open = true;
  let loaded: Record<string, unknown> | undefined;
  let applying = false;
  const post = (message: unknown): void => {
    if (open) void panel.webview.postMessage(message);
  };
  const failed = (error: unknown): void =>
    post({ type: "screenError", message: error instanceof Error ? error.message : String(error) });
  const refresh = async (): Promise<void> => {
    const version = document.version;
    try {
      const result = await vscode.commands.executeCommand<{
        ok: boolean;
        model?: Record<string, unknown>;
        errors?: Array<{ message: string }>;
      }>("trust-lsp.hmiFileParse", { kind: "screen", text: document.getText(), file: "hmi/screen.toml" });
      if (!open || version !== document.version) return;
      loaded = result?.ok ? result.model : undefined;
      if (loaded) {
        const [pages, texts, users] = await Promise.all([
          listPages(project),
          languageServerTexts(project),
          projectUsers(project),
        ]);
        if (open && version === document.version)
          post({ type: "screenDocument", model: loaded, pages, shipped: texts?.shipped ?? [], users });
      } else failed(result?.errors?.[0]?.message ?? "Screen settings cannot be read.");
    } catch (error) {
      failed(error);
    }
  };
  const apply = async (patch: unknown): Promise<void> => {
    if (!loaded || applying || !patch || typeof patch !== "object" || Array.isArray(patch)) return;
    const changes = patch as Record<string, unknown>;
    // Keep unrelated settings and project colours; reject arbitrary model replacement.
    if (
      Object.keys(changes).some(
        (key) => !["size", "theme", "languages", "colors", "navigation", "start_page", "top_bar"].includes(key),
      )
    )
      return;
    const model = { ...loaded, ...changes };
    if (model.start_page === "") delete model.start_page;
    if (sameModel(model, loaded)) return;
    const version = document.version;
    applying = true;
    try {
      const answer = await vscode.commands.executeCommand<{
        ok: boolean;
        text?: string;
        errors?: Array<{ message: string }>;
      }>("trust-lsp.hmiFileFormat", { kind: "screen", model });
      if (!open || document.version !== version) {
        failed("Screen settings changed while editing; your work was preserved.");
        return;
      }
      if (!answer?.ok || answer.text === undefined) {
        failed(answer?.errors?.[0]?.message ?? "Invalid screen settings.");
        return;
      }
      const change = smallestEdit(document.getText(), answer.text);
      if (!change) return;
      const pos = (line: number): vscode.Position =>
        line < document.lineCount ? new vscode.Position(line, 0) : document.lineAt(document.lineCount - 1).range.end;
      const edit = new vscode.WorkspaceEdit();
      edit.replace(document.uri, new vscode.Range(pos(change.startLine), pos(change.endLine)), change.text);
      if (!(await vscode.workspace.applyEdit(edit))) failed("VS Code could not apply screen settings.");
      await refresh();
    } catch (error) {
      failed(error);
    } finally {
      applying = false;
    }
  };
  const subscriptions = [
    panel.webview.onDidReceiveMessage((message) => {
      if (message?.type === "ready") void refresh();
      if (message?.type === "editScreen") void apply(message.changes);
      if (message?.type === "openAsText")
        void vscode.commands.executeCommand("vscode.openWith", document.uri, "default");
      // The runtime display's users live in runtime.toml (§12.8); a change to it refreshes them.
      if (message?.type === "addUser")
        void vscode.commands.executeCommand(ADD_USER_COMMAND, { project: project.toString() });
      if (message?.type === "removeUser" && typeof message.name === "string")
        void vscode.window
          .showWarningMessage(`Remove ${message.name}? They can no longer log in to the runtime display.`, "Remove")
          .then((answer) =>
            answer === "Remove"
              ? vscode.commands.executeCommand(REMOVE_USER_COMMAND, { project: project.toString(), name: message.name })
              : undefined,
          );
    }),
    vscode.workspace.onDidChangeTextDocument((event) => {
      const changed = event.document.uri.toString();
      if ((changed === document.uri.toString() || changed === runtimeToml) && event.contentChanges.length)
        void refresh();
    }),
  ];
  panel.onDidDispose(() => {
    open = false;
    subscriptions.forEach((subscription) => subscription.dispose());
  });
}
