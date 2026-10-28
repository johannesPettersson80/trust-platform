import { prepareBundledLibraries } from "./libraryEdit";
// Explorer owns page/panel organisation; native formats own their text. The language server
// (`trust-lsp.hmiFiles`, specification 39 §3.3 item 8) decides a new file's name, text and the
// screen layout a project without one gets, and the links a renamed file keeps; this module asks,
// creates the files it is given and edits the files whose links change.
import * as vscode from "vscode";

const VIEW = "trust-lsp.hmiBuilder";
function projectOf(uri?: vscode.Uri): vscode.Uri | undefined {
  if (uri) {
    const index = uri.path.indexOf("/hmi");
    if (index >= 0 && (uri.path.length === index + 4 || uri.path[index + 4] === "/"))
      return uri.with({ path: uri.path.slice(0, index) });
    return vscode.workspace.getWorkspaceFolder(uri)?.uri;
  }
  return vscode.workspace.workspaceFolders?.length === 1 ? vscode.workspace.workspaceFolders[0].uri : undefined;
}

/** An answer of the language server: `ok`, or why not. */
type Answer<T> = T & { ok?: boolean; error?: string };

/** The language server's answer to a request about HMI files, or `undefined` while it is not running. */
async function files<T>(request: Record<string, unknown>): Promise<Answer<T> | undefined> {
  try {
    return await vscode.commands.executeCommand<Answer<T>>("trust-lsp.hmiFiles", request);
  } catch {
    return undefined;
  }
}

/** Why `name` cannot name an HMI file, for the name box while it is typed; `undefined` when it can. */
async function nameProblem(name: string): Promise<string | undefined> {
  const answer = await files<{ problem?: string }>({ action: "name", name });
  if (answer?.ok !== true) return answer?.error ?? "The truST language server is not running yet.";
  return answer.problem;
}

/** Whether the project has its screen layout; another failure than a missing file is thrown. */
async function hasScreen(screen: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(screen);
    return true;
  } catch (error) {
    if (!(error instanceof vscode.FileSystemError) || error.code !== "FileNotFound") throw error;
    return false;
  }
}

export async function createHmiFile(
  kind: "page" | "panel",
  uri?: vscode.Uri,
  suppliedName?: string,
  extensionPath?: string,
): Promise<vscode.Uri | undefined> {
  const project = projectOf(uri ?? vscode.window.activeTextEditor?.document.uri);
  if (!project) {
    void vscode.window.showInformationMessage("Choose the project folder in Explorer first.");
    return;
  }
  const name =
    suppliedName ??
    (await vscode.window.showInputBox({
      title: kind === "page" ? "New HMI page" : "New HMI panel",
      prompt: "Name",
      validateInput: nameProblem,
    }));
  if (!name) return;
  const screen = vscode.Uri.joinPath(project, "hmi", "screen.toml");
  const created = await files<{ file?: string; text?: string; screen?: string }>({
    action: "create",
    kind,
    name,
    hasScreen: await hasScreen(screen),
  });
  if (created?.ok !== true || created.file === undefined || created.text === undefined)
    throw new Error(created?.error ?? "The HMI definition could not be created.");
  const file = vscode.Uri.joinPath(project, created.file);
  const edit = new vscode.WorkspaceEdit();
  edit.createFile(file, { overwrite: false, ignoreIfExists: false });
  edit.insert(file, new vscode.Position(0, 0), created.text);
  if (created.screen !== undefined) {
    edit.createFile(screen, { overwrite: false, ignoreIfExists: false });
    edit.insert(screen, new vscode.Position(0, 0), created.screen);
  }
  const installed = extensionPath ?? vscode.extensions.getExtension("trust-platform.trust-lsp")?.extensionPath;
  if (!installed) throw new Error("The truST extension library location is unavailable.");
  const verifyLibraries = await prepareBundledLibraries(
    project,
    installed,
    [
      { name: "Hmi", folder: "hmi" },
      { name: "Plant", folder: "plant" },
    ],
    edit,
  );
  await verifyLibraries();
  if (!(await vscode.workspace.applyEdit(edit)))
    throw new Error("VS Code could not create the HMI file. An existing file was left intact.");
  await vscode.commands.executeCommand("vscode.openWith", file, VIEW);
  return file;
}

/** The renamed pages, panels and popups of one project, as the language server groups them. */
interface RenamedProject {
  path: string;
  index: number;
  renames: Array<{ folder: string; before: string; after: string }>;
}

/** Preserve references, including sidebar assignments, through ordinary Explorer rename: the
 * language server says which renames are of HMI files and gives the new text of each file whose
 * links change. */
async function referenceEdits(
  renames: readonly { oldUri: vscode.Uri; newUri: vscode.Uri }[],
): Promise<vscode.WorkspaceEdit> {
  const edit = new vscode.WorkspaceEdit();
  const scope = await files<{ projects?: RenamedProject[]; pattern?: string }>({
    action: "renames",
    renames: renames.map(({ oldUri, newUri }) => ({ from: oldUri.path, to: newUri.path })),
  });
  // Without the language server no link can be read, so the rename goes ahead without them.
  if (scope === undefined) return edit;
  if (scope.ok !== true) throw new Error(scope.error ?? "HMI references could not be updated.");
  for (const renamed of scope.projects ?? []) {
    const project = renames[renamed.index].oldUri.with({ path: renamed.path });
    const opened = new Map<string, { document: vscode.TextDocument; version: number }>();
    const texts: Record<string, string> = {};
    for (const uri of await vscode.workspace.findFiles(new vscode.RelativePattern(project, scope.pattern ?? ""))) {
      const document = await vscode.workspace.openTextDocument(uri);
      opened.set(uri.path, { document, version: document.version });
      texts[uri.path] = document.getText();
    }
    const answer = await files<{ write?: Array<{ file: string; text: string }> }>({
      action: "references",
      renames: renamed.renames,
      files: texts,
    });
    if (answer?.ok !== true) throw new Error(answer?.error ?? "HMI references could not be updated.");
    for (const { file, text } of answer.write ?? []) {
      const target = opened.get(file);
      if (!target) continue;
      if (target.document.version !== target.version)
        throw new Error("An HMI file changed during rename. Your edits were preserved; review its page links.");
      edit.replace(target.document.uri, new vscode.Range(0, 0, target.document.lineCount, 0), text);
    }
  }
  return edit;
}

export function registerHmiExplorer(extensionPath: string): vscode.Disposable {
  const create = (kind: "page" | "panel", uri?: vscode.Uri): Promise<unknown> =>
    createHmiFile(kind, uri, undefined, extensionPath).catch((error) => vscode.window.showErrorMessage(String(error)));
  return vscode.Disposable.from(
    vscode.commands.registerCommand("trust-lsp.hmiBuilder.newPage", (uri?: vscode.Uri) => create("page", uri)),
    vscode.commands.registerCommand("trust-lsp.hmiBuilder.newPanel", (uri?: vscode.Uri) => create("panel", uri)),
    vscode.workspace.onWillRenameFiles((event) => event.waitUntil(referenceEdits(event.files))),
  );
}
