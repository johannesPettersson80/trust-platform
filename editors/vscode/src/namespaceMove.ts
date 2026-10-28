import * as path from "path";
import * as vscode from "vscode";
import {
  ExecuteCommandRequest,
  LanguageClient,
} from "vscode-languageclient/node";

import { projectRequest } from "./projectRequest";

// Move Structured Text Namespace (specification 25 §1.1). The language server decides whether a line is a namespace
// line, which name the move starts from, whether a new namespace path can be taken and which file it moves to by
// default (`trust-lsp.namespaceTarget`), offers the quick fix, and computes the move (`trust-lsp.moveNamespace`). This
// module prompts, creates and cleans up the target file, applies the server's edit and keeps the menu's context key.

type PositionLike = vscode.Position | { line: number; character: number };

type MoveNamespaceArgs = {
  uri?: vscode.Uri | string;
  textDocument?: { uri: string };
  position?: PositionLike;
  newPath?: string;
  new_path?: string;
  targetUri?: vscode.Uri | string;
  target_uri?: string;
};

type LspPosition = { line: number; character: number };
type LspRange = { start: LspPosition; end: LspPosition };
type LspTextEdit = { range: LspRange; newText?: string; new_text?: string };
type LspTextDocumentEdit = {
  textDocument?: { uri?: string };
  text_document?: { uri?: string };
  edits?: LspTextEdit[];
};
type LspResourceOperation = {
  kind?: "create" | "delete" | "rename";
  uri?: string;
  oldUri?: string;
  newUri?: string;
  old_uri?: string;
  new_uri?: string;
  options?: {
    overwrite?: boolean;
    ignoreIfExists?: boolean;
    ignore_if_exists?: boolean;
    recursive?: boolean;
    ignoreIfNotExists?: boolean;
    ignore_if_not_exists?: boolean;
  };
};
type LspWorkspaceEdit = {
  changes?: Record<string, LspTextEdit[]>;
  documentChanges?: Array<LspTextDocumentEdit | LspResourceOperation>;
  document_changes?: Array<LspTextDocumentEdit | LspResourceOperation>;
};

/** What `trust-lsp.namespaceTarget` answers for the cursor's line. */
type NamespaceContext = { onNamespaceLine: boolean; namePosition?: LspPosition | null };
/** What `trust-lsp.namespaceTarget` answers for a new namespace path. */
type NamespacePath = { problem?: string | null; path?: string | null; target?: string | null };

const NAMESPACE_CONTEXT_KEY = "trust-lsp.namespaceContext";
const MOVE_NAMESPACE_UI_COMMAND = "trust-lsp.moveNamespace.ui";
const NAMESPACE_TARGET_COMMAND = "trust-lsp.namespaceTarget";
const LAST_TARGET_KEY = "trust-lsp.lastNamespaceMoveTarget";
/** The context key follows the cursor after it rests this long, so a fast selection asks the server once. */
const CONTEXT_DELAY_MS = 75;

function asUri(value?: vscode.Uri | string): vscode.Uri | undefined {
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

function asPosition(value?: PositionLike): vscode.Position | undefined {
  if (!value) {
    return undefined;
  }
  if (value instanceof vscode.Position) {
    return value;
  }
  if (
    typeof value.line === "number" &&
    typeof value.character === "number" &&
    Number.isFinite(value.line) &&
    Number.isFinite(value.character)
  ) {
    return new vscode.Position(value.line, value.character);
  }
  return undefined;
}

function isStructuredTextEditor(editor?: vscode.TextEditor): editor is vscode.TextEditor {
  return !!editor && editor.document.languageId === "structured-text";
}

/** The server's answer for the line at `position` of `uri`; a server that is not running says it is no namespace line. */
async function namespaceContext(
  uri: vscode.Uri,
  position: vscode.Position,
  wait: boolean
): Promise<NamespaceContext> {
  const answer = await projectRequest<NamespaceContext>(
    NAMESPACE_TARGET_COMMAND,
    {
      action: "context",
      textDocument: { uri: uri.toString() },
      position: { line: position.line, character: position.character },
    },
    { wait }
  );
  return answer.ok ? answer : { onNamespaceLine: false };
}

let contextTimer: ReturnType<typeof setTimeout> | undefined;
let contextGeneration = 0;

async function updateNamespaceContext(editor: vscode.TextEditor | undefined): Promise<void> {
  const generation = ++contextGeneration;
  const onNamespaceLine = isStructuredTextEditor(editor)
    ? (await namespaceContext(editor.document.uri, editor.selection.active, false)).onNamespaceLine
    : false;
  if (generation === contextGeneration) {
    await vscode.commands.executeCommand("setContext", NAMESPACE_CONTEXT_KEY, onNamespaceLine);
  }
}

function scheduleNamespaceContext(editor: vscode.TextEditor | undefined): void {
  if (contextTimer) {
    clearTimeout(contextTimer);
  }
  contextTimer = setTimeout(() => {
    contextTimer = undefined;
    void updateNamespaceContext(editor);
  }, CONTEXT_DELAY_MS);
}

/** The server's check of a new namespace path, and the file it moves to by default. */
async function checkNamespacePath(
  newPath: string
): Promise<{ ok: true; answer: NamespacePath } | { ok: false; error: string }> {
  const answer = await projectRequest<NamespacePath>(NAMESPACE_TARGET_COMMAND, {
    action: "path",
    newPath,
  });
  return answer.ok ? { ok: true, answer } : { ok: false, error: answer.error };
}

async function promptForNewPath(): Promise<string | undefined> {
  return vscode.window.showInputBox({
    prompt: "Enter the new namespace path",
    placeHolder: "Company.Library.Namespace",
    async validateInput(value) {
      const checked = await checkNamespacePath(value);
      return checked.ok ? checked.answer.problem ?? undefined : checked.error;
    },
  });
}

type TargetChoice = vscode.QuickPickItem & {
  target?: vscode.Uri;
  choiceKind: "default" | "last" | "pick";
};

async function promptForTarget(
  suggested: vscode.Uri | undefined,
  lastTarget?: vscode.Uri
): Promise<vscode.Uri | undefined> {
  const choices: TargetChoice[] = [
    {
      label: "Use default mapping",
      description: suggested?.fsPath ?? "Workspace-based mapping",
      target: suggested,
      choiceKind: "default",
    },
  ];
  if (lastTarget) {
    choices.push({
      label: "Use last target",
      description: lastTarget.fsPath,
      target: lastTarget,
      choiceKind: "last",
    });
  }
  choices.push({
    label: "Choose target file...",
    description: "Pick or create a .st file for the moved namespace",
    choiceKind: "pick",
  });

  const choice = await vscode.window.showQuickPick<TargetChoice>(choices, {
    placeHolder: "Select a target file for the namespace relocation",
  });
  if (!choice) {
    return undefined;
  }
  if (choice.choiceKind === "default" || choice.choiceKind === "last") {
    return choice.target;
  }
  return vscode.window.showSaveDialog({
    defaultUri: suggested,
    filters: { "Structured Text": ["st", "ST"] },
    saveLabel: "Move namespace here",
  });
}

async function ensureTargetFile(uri: vscode.Uri): Promise<boolean> {
  await vscode.workspace.fs.createDirectory(
    vscode.Uri.file(path.dirname(uri.fsPath))
  );
  try {
    await vscode.workspace.fs.stat(uri);
    return false;
  } catch {
    await vscode.workspace.fs.writeFile(uri, Buffer.from(""));
    return true;
  }
}

async function removeCreatedTargetIfEmpty(
  uri: vscode.Uri,
  created: boolean
): Promise<void> {
  if (!created) {
    return;
  }
  try {
    const data = await vscode.workspace.fs.readFile(uri);
    if (data.length === 0) {
      await vscode.workspace.fs.delete(uri);
    }
  } catch {
    // Best-effort cleanup only; the user-facing failure message is shown below.
  }
}

function toVsCodeRange(range: LspRange): vscode.Range {
  return new vscode.Range(
    new vscode.Position(range.start.line, range.start.character),
    new vscode.Position(range.end.line, range.end.character)
  );
}

function addTextEdits(
  edit: vscode.WorkspaceEdit,
  uriValue: string | undefined,
  edits: LspTextEdit[] | undefined
): void {
  if (!uriValue || !Array.isArray(edits)) {
    return;
  }
  const uri = vscode.Uri.parse(uriValue);
  for (const textEdit of edits) {
    if (!textEdit.range) {
      continue;
    }
    edit.replace(
      uri,
      toVsCodeRange(textEdit.range),
      textEdit.newText ?? textEdit.new_text ?? ""
    );
  }
}

async function applyLspWorkspaceEdit(value: unknown): Promise<boolean> {
  const lspEdit = value as LspWorkspaceEdit | undefined;
  if (!lspEdit || typeof lspEdit !== "object") {
    return false;
  }

  const applyEdit = new vscode.WorkspaceEdit();
  const cleanupEdit = new vscode.WorkspaceEdit();
  let hasApplyOps = false;
  let hasCleanupOps = false;

  for (const [uri, edits] of Object.entries(lspEdit.changes ?? {})) {
    addTextEdits(applyEdit, uri, edits);
    hasApplyOps = hasApplyOps || edits.length > 0;
  }

  for (const operation of lspEdit.documentChanges ?? lspEdit.document_changes ?? []) {
    const textDocument = (operation as LspTextDocumentEdit).textDocument ??
      (operation as LspTextDocumentEdit).text_document;
    if (textDocument) {
      const edits = (operation as LspTextDocumentEdit).edits ?? [];
      addTextEdits(applyEdit, textDocument.uri, edits);
      hasApplyOps = hasApplyOps || edits.length > 0;
      continue;
    }

    const resource = operation as LspResourceOperation;
    if (resource.kind === "create" && resource.uri) {
      applyEdit.createFile(vscode.Uri.parse(resource.uri), {
        overwrite: resource.options?.overwrite,
        ignoreIfExists:
          resource.options?.ignoreIfExists ?? resource.options?.ignore_if_exists,
      });
      hasApplyOps = true;
    } else if (resource.kind === "rename") {
      const oldUri = resource.oldUri ?? resource.old_uri;
      const newUri = resource.newUri ?? resource.new_uri;
      if (oldUri && newUri) {
        applyEdit.renameFile(vscode.Uri.parse(oldUri), vscode.Uri.parse(newUri), {
          overwrite: resource.options?.overwrite,
          ignoreIfExists:
            resource.options?.ignoreIfExists ?? resource.options?.ignore_if_exists,
        });
        hasApplyOps = true;
      }
    } else if (resource.kind === "delete" && resource.uri) {
      cleanupEdit.deleteFile(vscode.Uri.parse(resource.uri), {
        recursive: resource.options?.recursive,
        ignoreIfNotExists:
          resource.options?.ignoreIfNotExists ??
          resource.options?.ignore_if_not_exists,
      });
      hasCleanupOps = true;
    }
  }

  if (hasApplyOps && !(await vscode.workspace.applyEdit(applyEdit))) {
    return false;
  }
  return hasCleanupOps ? vscode.workspace.applyEdit(cleanupEdit) : true;
}

export function registerNamespaceMoveCommand(
  context: vscode.ExtensionContext,
  client: LanguageClient
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(
      MOVE_NAMESPACE_UI_COMMAND,
      async (args?: MoveNamespaceArgs) => {
        const editor = vscode.window.activeTextEditor;
        const uri =
          asUri(args?.uri) ??
          asUri(args?.textDocument?.uri) ??
          editor?.document.uri;
        if (!uri) {
          vscode.window.showErrorMessage(
            "Move namespace requires an active Structured Text file."
          );
          return false;
        }

        // A position the caller names is taken as it is; the cursor starts the move from the name the server finds
        // on its namespace line, else from where it stands.
        let position = asPosition(args?.position);
        if (!position) {
          const cursor = editor?.selection.active;
          if (!cursor) {
            vscode.window.showErrorMessage(
              "Move namespace requires a cursor position in a namespace."
            );
            return false;
          }
          const found = (await namespaceContext(uri, cursor, true)).namePosition;
          position = found ? new vscode.Position(found.line, found.character) : cursor;
        }

        const requestedPath = args?.newPath ?? args?.new_path ?? (await promptForNewPath());
        if (!requestedPath) {
          return false;
        }
        const checked = await checkNamespacePath(requestedPath);
        if (!checked.ok) {
          vscode.window.showErrorMessage(checked.error);
          return false;
        }
        if (checked.answer.problem || !checked.answer.path) {
          vscode.window.showErrorMessage(
            checked.answer.problem ?? "Namespace path must use dot-separated identifiers."
          );
          return false;
        }
        const newPath = checked.answer.path;

        let targetUri =
          asUri(args?.targetUri) ?? asUri(args?.target_uri ?? undefined);
        if (!args?.targetUri && !args?.target_uri) {
          const lastTarget = context.globalState.get<string>(LAST_TARGET_KEY);
          const lastTargetUri = lastTarget ? asUri(lastTarget) : undefined;
          const suggested = checked.answer.target ? asUri(checked.answer.target) : undefined;
          const target = await promptForTarget(suggested, lastTargetUri);
          if (!target) {
            return false;
          }
          targetUri = target;
        }
        let createdTarget = false;
        if (targetUri) {
          createdTarget = await ensureTargetFile(targetUri);
        }

        await client.start();
        let result: unknown;
        try {
          result = await client.sendRequest(ExecuteCommandRequest.type, {
            command: "trust-lsp.moveNamespace",
            arguments: [
              {
                text_document: { uri: uri.toString() },
                position: { line: position.line, character: position.character },
                new_path: newPath,
                return_edit: true,
                ...(targetUri ? { target_uri: targetUri.toString() } : {}),
              },
            ],
          });
        } catch (error) {
          if (targetUri) {
            await removeCreatedTargetIfEmpty(targetUri, createdTarget);
          }
          throw error;
        }

        result = await applyLspWorkspaceEdit(result);

        if (result === true) {
          if (targetUri) {
            await context.globalState.update(
              LAST_TARGET_KEY,
              targetUri.toString()
            );
          }
          vscode.window.showInformationMessage("Namespace move applied.");
        } else {
          if (targetUri) {
            await removeCreatedTargetIfEmpty(targetUri, createdTarget);
          }
          vscode.window.showWarningMessage(
            "Namespace move did not apply. Ensure the cursor is inside a namespace declaration."
          );
        }
        return result === true;
      }
    )
  );
}

/** Keeps the editor menu's `trust-lsp.namespaceContext` key on the server's answer for the cursor's line. */
export function registerNamespaceMoveContext(
  context: vscode.ExtensionContext
): void {
  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor(scheduleNamespaceContext),
    vscode.window.onDidChangeTextEditorSelection((event) =>
      scheduleNamespaceContext(event.textEditor)
    ),
    {
      dispose: () => {
        if (contextTimer) {
          clearTimeout(contextTimer);
          contextTimer = undefined;
        }
      },
    }
  );
  scheduleNamespaceContext(vscode.window.activeTextEditor);
}
