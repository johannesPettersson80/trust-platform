import * as vscode from "vscode";

const STRUCTURED_TEXT_FILE = /\.(?:st|pou)$/i;

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function isFileNotFound(error: unknown): boolean {
  return error instanceof vscode.FileSystemError && error.code === "FileNotFound";
}

async function pathExists(uri: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(uri);
    return true;
  } catch (error) {
    if (isFileNotFound(error)) {
      return false;
    }
    throw error;
  }
}

async function structuredTextFilesUnder(uri: vscode.Uri): Promise<vscode.Uri[]> {
  if (!(await pathExists(uri))) {
    return [];
  }
  const files: vscode.Uri[] = [];
  for (const [name, type] of await vscode.workspace.fs.readDirectory(uri)) {
    const child = vscode.Uri.joinPath(uri, name);
    if ((type & vscode.FileType.Directory) !== 0) {
      files.push(...(await structuredTextFilesUnder(child)));
    } else if (STRUCTURED_TEXT_FILE.test(name)) {
      files.push(child);
    }
  }
  return files;
}

export async function closeAllEditorsAndWait(
  sourceUris: readonly vscode.Uri[] = []
): Promise<void> {
  const documents = new Map(
    vscode.workspace.textDocuments
      .filter(
        (document) =>
          document.languageId === "structured-text" &&
          document.uri.scheme === "file"
      )
      .map((document) => [document.uri.toString(), document] as const)
  );

  // VS Code may retain a TextDocument after its final editor tab closes. In
  // that case the language client sends no didClose, and deleting the backing
  // tree leaves the server's open-document overlay (and its declarations)
  // alive for later suites. Neutralize only sources owned by this cleanup
  // before deleting them. The files are test fixtures that are removed below;
  // saving the empty text guarantees that a retained overlay contains no
  // symbols even when VS Code chooses not to dispose its TextDocument.
  const cleanupSources = new Set(sourceUris.map((uri) => uri.toString()));
  const openCleanupDocuments = [...documents.values()].filter((document) =>
    cleanupSources.has(document.uri.toString())
  );
  if (openCleanupDocuments.length > 0) {
    const edit = new vscode.WorkspaceEdit();
    for (const document of openCleanupDocuments) {
      const end = document.lineAt(document.lineCount - 1).rangeIncludingLineBreak
        .end;
      edit.replace(
        document.uri,
        new vscode.Range(new vscode.Position(0, 0), end),
        ""
      );
    }
    if (!(await vscode.workspace.applyEdit(edit))) {
      throw new Error("Failed to neutralize Extension Host test sources.");
    }
    for (const document of openCleanupDocuments) {
      if (!(await document.save())) {
        throw new Error(
          `Failed to save neutralized Extension Host test source ${document.uri.fsPath}.`
        );
      }
    }
    await delay(250);
  }

  // Never open a hidden source merely to delete it. Opening a workspace file
  // sends textDocument/didOpen to the real language server; VS Code may retain
  // that model after its tab closes, so cleanup itself can leak declarations
  // into later suites. Only close documents that a test actually opened.

  const textTabUris = new Set(
    vscode.window.tabGroups.all
      .flatMap((group) => [...group.tabs])
      .flatMap((tab) =>
        tab.input instanceof vscode.TabInputText
          ? [tab.input.uri.toString()]
          : []
      )
  );
  for (const document of documents.values()) {
    if (!textTabUris.has(document.uri.toString())) {
      await vscode.window.showTextDocument(document, {
        preview: false,
        preserveFocus: true,
      });
    }
  }
  const tabs = vscode.window.tabGroups.all.flatMap((group) => [...group.tabs]);
  if (tabs.length > 0) {
    const closed = await vscode.window.tabGroups.close(tabs, true);
    if (!closed) {
      throw new Error("Failed to close all Extension Host test editors.");
    }
  }

  const deadline = Date.now() + 5_000;
  while (
    (vscode.window.tabGroups.all.some((group) => group.tabs.length > 0) ||
      vscode.window.visibleTextEditors.length > 0) &&
    Date.now() < deadline
  ) {
    await delay(50);
  }
  if (
    vscode.window.tabGroups.all.some((group) => group.tabs.length > 0) ||
    vscode.window.visibleTextEditors.length > 0
  ) {
    throw new Error("Timed out waiting for Extension Host test editors to close.");
  }

  // Give the language client time to forward textDocument/didClose before
  // watched source files are removed. External /tmp documents are included:
  // project-scoped test cleanup must not leave them in the LSP either.
  await delay(250);
}

export async function deleteFileIfExistsStrict(
  uri: vscode.Uri
): Promise<boolean> {
  if (!(await pathExists(uri))) {
    return false;
  }
  await vscode.workspace.fs.delete(uri, { useTrash: false });
  if (await pathExists(uri)) {
    throw new Error(`Extension Host test cleanup did not delete ${uri.fsPath}.`);
  }
  return true;
}

export async function waitForStructuredTextEviction(
  uris: readonly vscode.Uri[],
  timeoutMs = 5_000
): Promise<void> {
  const watched = uris.filter((uri) => vscode.workspace.getWorkspaceFolder(uri));
  if (watched.length === 0) {
    return;
  }
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const filesRemain = (
      await Promise.all(watched.map((uri) => pathExists(uri)))
    ).some(Boolean);
    if (!filesRemain) {
      // VS Code may retain deleted TextDocument/diagnostic objects after the
      // language server has evicted their indexed sources. Those caches are
      // not an eviction signal. Wait through the file-watcher debounce, then
      // observe diagnostics once so queued refresh notifications are drained
      // before the next suite creates declarations with the same names.
      await delay(2_000);
      for (const uri of watched) {
        vscode.languages.getDiagnostics(uri);
      }
      return;
    }
    await delay(50);
  }
  throw new Error(
    `Timed out waiting for Structured Text cleanup: ${watched
      .map((uri) => uri.fsPath)
      .join(", ")}`
  );
}

export async function deleteWorkspaceTreeStrict(
  root: vscode.Uri
): Promise<void> {
  if (!(await pathExists(root))) {
    return;
  }
  const sources = (await structuredTextFilesUnder(root)).sort((left, right) =>
    left.fsPath.localeCompare(right.fsPath)
  );
  await closeAllEditorsAndWait(sources);
  for (const source of sources) {
    await deleteFileIfExistsStrict(source);
  }
  await waitForStructuredTextEviction(sources);
  await vscode.workspace.fs.delete(root, {
    recursive: true,
    useTrash: false,
  });
  if (await pathExists(root)) {
    throw new Error(`Extension Host test cleanup did not delete ${root.fsPath}.`);
  }
}
