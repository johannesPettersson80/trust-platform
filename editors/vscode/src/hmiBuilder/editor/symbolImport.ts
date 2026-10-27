// HMI: Import Symbol (specification 39, §14.4 item 5): an SVG file becomes one of the project's own
// symbols, its drawing copied to `hmi/symbols/<name>.svg` and a definition written beside it that
// names each element of the drawing as a part to drive, which the engineer then completes.

import * as path from "path";
import * as vscode from "vscode";

export const IMPORT_SYMBOL_COMMAND = "trust-lsp.hmiBuilder.importSymbol";

/** What the command may be given instead of asking: the SVG file, the name and the project. */
interface ImportArgs {
  file?: string;
  name?: string;
  project?: string;
}

export function registerSymbolImport(): vscode.Disposable {
  return vscode.commands.registerCommand(IMPORT_SYMBOL_COMMAND, async (args?: ImportArgs) => importSymbol(args ?? {}));
}

/** What `trust-lsp.hmiImportSymbol` answers. */
interface ImportAnswer {
  ok?: boolean;
  error?: string;
  name?: string;
  problem?: string | null;
  text?: string;
}

async function ask(argument: Record<string, unknown>): Promise<ImportAnswer> {
  const answer = await vscode.commands.executeCommand<ImportAnswer | undefined>("trust-lsp.hmiImportSymbol", argument);
  if (answer?.ok !== true) throw new Error(answer?.error ?? "The truST language server is not running.");
  return answer;
}

/** Why `name` cannot name a symbol, or undefined. */
async function nameProblem(name: string): Promise<string | undefined> {
  try {
    return (await ask({ action: "checkName", name })).problem ?? undefined;
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
}

async function importSymbol(args: ImportArgs): Promise<void> {
  const project = args.project ? vscode.Uri.file(args.project) : await projectWithHmi();
  if (!project) {
    void vscode.window.showErrorMessage("Open a project with an HMI (hmi/screen.toml) to import a symbol into.");
    return;
  }
  const source = args.file
    ? vscode.Uri.file(args.file)
    : (
        await vscode.window.showOpenDialog({
          canSelectMany: false,
          filters: { "SVG drawing": ["svg"] },
          title: "Import Symbol",
        })
      )?.[0];
  if (!source) return;
  const title = path.basename(source.fsPath, path.extname(source.fsPath));
  try {
    const suggested = (await ask({ action: "suggest", title })).name ?? "symbol";
    const name =
      args.name ??
      (await vscode.window.showInputBox({
        title: "Import Symbol",
        prompt: "The symbol's name, which pages give its objects as my:<name>",
        value: suggested,
        validateInput: nameProblem,
      }));
    if (!name || (await nameProblem(name)) !== undefined) return;
    const folder = vscode.Uri.joinPath(project, "hmi", "symbols");
    const definition = vscode.Uri.joinPath(folder, `${name}.toml`);
    if (await exists(definition)) {
      void vscode.window.showErrorMessage(`The project has a symbol "${name}" already: ${definition.fsPath}`);
      return;
    }
    const drawing = await vscode.workspace.fs.readFile(source);
    const starter = await ask({ action: "definition", title, svg: Buffer.from(drawing).toString("utf8") });
    await vscode.workspace.fs.createDirectory(folder);
    await vscode.workspace.fs.writeFile(vscode.Uri.joinPath(folder, `${name}.svg`), drawing);
    await vscode.workspace.fs.writeFile(definition, Buffer.from(starter.text ?? "", "utf8"));
    await vscode.window.showTextDocument(definition);
  } catch (error) {
    void vscode.window.showErrorMessage(
      `The symbol was not imported: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
}

/** The open project with an HMI; the user picks one when there are several. */
export async function projectWithHmi(): Promise<vscode.Uri | undefined> {
  const found: vscode.Uri[] = [];
  for (const folder of vscode.workspace.workspaceFolders ?? []) {
    for (const screen of await vscode.workspace.findFiles(
      new vscode.RelativePattern(folder, "**/hmi/screen.toml"),
      "**/node_modules/**",
      20,
    )) {
      found.push(vscode.Uri.file(path.dirname(path.dirname(screen.fsPath))));
    }
  }
  if (found.length <= 1) return found[0];
  const picked = await vscode.window.showQuickPick(
    found.map((uri) => ({ label: path.basename(uri.fsPath), description: uri.fsPath, uri })),
    { title: "Import Symbol", placeHolder: "The project to import the symbol into" },
  );
  return picked?.uri;
}

async function exists(uri: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(uri);
    return true;
  } catch {
    return false;
  }
}
