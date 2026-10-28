// New HMI Screen (specification 39, §4.10): a screen layout and a first page from a template.
// The language server gives the templates and the files' canonical texts
// (`trust-lsp.hmiNewScreen`); this module offers the templates, never overwrites an existing
// screen layout, writes the files and opens the page. The program gets what serves the screens
// (§18 Phase 5).

import * as path from "path";
import * as vscode from "vscode";

import { prepareBundledLibraries } from "./libraryEdit";

export const NEW_SCREEN_COMMAND = "trust-lsp.hmiBuilder.newScreen";

/** A template as the language server offers it. */
interface Template {
  id: string;
  label: string;
  detail: string;
}

/** A file the language server gives: its path in the project and its text. */
interface ProjectFile {
  file: string;
  text: string;
}

interface NewScreenArgs {
  /** The template's id; asked for when missing. */
  template?: string;
  /** The project folder; the active file's workspace folder, or the first one, when missing. */
  folder?: string;
}

export function registerHmiTemplates(extensionPath: string): vscode.Disposable {
  return vscode.commands.registerCommand(NEW_SCREEN_COMMAND, (args?: NewScreenArgs) =>
    newScreen(args ?? {}, extensionPath),
  );
}

const NOT_RUNNING = "The truST language server is not running yet, so the screen could not be created.";

async function newScreen(args: NewScreenArgs, extensionPath: string): Promise<void> {
  const folder = args.folder ? vscode.Uri.parse(args.folder) : projectFolder();
  if (!folder) {
    void vscode.window.showErrorMessage("Open a truST project folder first.");
    return;
  }
  const offered = await screens<{ templates?: Template[] }>({ action: "templates" });
  if (offered?.ok !== true) {
    void vscode.window.showErrorMessage(offered?.error ?? NOT_RUNNING);
    return;
  }
  const templates = offered.templates ?? [];
  const template = args.template
    ? templates.find((candidate) => candidate.id === args.template)
    : await pickTemplate(templates);
  if (!template) return;
  // The screen's title is the folder's name in words (§4 item 8): the language server gets the name.
  const created = await screens<{ screen?: ProjectFile; page?: ProjectFile }>({
    action: "create",
    template: template.id,
    folder: path.basename(folder.fsPath),
  });
  if (created?.ok !== true || !created.screen || !created.page) {
    void vscode.window.showErrorMessage(created?.error ?? NOT_RUNNING);
    return;
  }
  const screenFile = vscode.Uri.joinPath(folder, created.screen.file);
  if (await exists(screenFile)) {
    void vscode.window.showInformationMessage(
      "This project already has a screen layout (hmi/screen.toml); it is left as it is.",
    );
    return;
  }
  const pageFile = vscode.Uri.joinPath(folder, created.page.file);
  // Written to disk at once, as the libraries are: a page left in an unsaved buffer showed the
  // empty file's problem until the user saved it.
  const edit = new vscode.WorkspaceEdit();
  edit.createFile(screenFile, {
    overwrite: false,
    ignoreIfExists: false,
    contents: Buffer.from(created.screen.text, "utf8"),
  });
  if (!(await exists(pageFile))) {
    edit.createFile(pageFile, {
      overwrite: false,
      ignoreIfExists: false,
      contents: Buffer.from(created.page.text, "utf8"),
    });
  }
  const verifyLibraries = await prepareBundledLibraries(
    folder,
    extensionPath,
    [
      { name: "Hmi", folder: "hmi" },
      { name: "Plant", folder: "plant" },
    ],
    edit,
  );
  await verifyLibraries();
  if (!(await vscode.workspace.applyEdit(edit)))
    throw new Error("The HMI files could not be created. Existing project files were preserved.");
  await vscode.commands.executeCommand("vscode.openWith", pageFile, "trust-lsp.hmiBuilder");
}

async function pickTemplate(templates: Template[]): Promise<Template | undefined> {
  const picked = await vscode.window.showQuickPick(
    templates.map((template) => ({ label: template.label, detail: template.detail, template })),
    { title: "New HMI Screen", placeHolder: "Choose a screen layout to start from" },
  );
  return picked?.template;
}

/** The project folder: the active file's workspace folder, else the first one. */
export function projectFolder(): vscode.Uri | undefined {
  const active = vscode.window.activeTextEditor?.document.uri;
  const folder = active ? vscode.workspace.getWorkspaceFolder(active) : undefined;
  return (folder ?? vscode.workspace.workspaceFolders?.[0])?.uri;
}

export async function exists(file: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(file);
    return true;
  } catch {
    return false;
  }
}

/** The language server's answer to a New HMI Screen request, or `undefined` while it is not running. */
async function screens<T>(
  request: Record<string, unknown>,
): Promise<(T & { ok?: boolean; error?: string }) | undefined> {
  try {
    return await vscode.commands.executeCommand<T & { ok?: boolean; error?: string }>(
      "trust-lsp.hmiNewScreen",
      request,
    );
  } catch {
    return undefined;
  }
}
