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

const NAME = /^[A-Za-z0-9_-]+$/;

export function registerSymbolImport(): vscode.Disposable {
  return vscode.commands.registerCommand(IMPORT_SYMBOL_COMMAND, async (args?: ImportArgs) => importSymbol(args ?? {}));
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
  const suggested =
    title
      .toLowerCase()
      .replace(/[^a-z0-9_-]+/g, "-")
      .replace(/^-+|-+$/g, "") || "symbol";
  const name =
    args.name ??
    (await vscode.window.showInputBox({
      title: "Import Symbol",
      prompt: "The symbol's name, which pages give its objects as my:<name>",
      value: suggested,
      validateInput: (text) => (NAME.test(text) ? undefined : "Use letters, digits, - and _"),
    }));
  if (!name || !NAME.test(name)) return;
  const folder = vscode.Uri.joinPath(project, "hmi", "symbols");
  const definition = vscode.Uri.joinPath(folder, `${name}.toml`);
  if (await exists(definition)) {
    void vscode.window.showErrorMessage(`The project has a symbol "${name}" already: ${definition.fsPath}`);
    return;
  }
  const drawing = await vscode.workspace.fs.readFile(source);
  await vscode.workspace.fs.createDirectory(folder);
  await vscode.workspace.fs.writeFile(vscode.Uri.joinPath(folder, `${name}.svg`), drawing);
  await vscode.workspace.fs.writeFile(
    definition,
    Buffer.from(starterDefinition(title, Buffer.from(drawing).toString("utf8")), "utf8"),
  );
  await vscode.window.showTextDocument(definition);
}

/** A definition to complete: the title, a new object's size in the drawing's proportions (its
 * symbol 64 px wide above the 44 px of tag and state word), and each element id of the drawing as
 * a part to name. */
export function starterDefinition(title: string, svg: string): string {
  const [width, height] = proportions(svg);
  const symbolHeight = Math.max(16, Math.round((64 * height) / width));
  const ids = [...new Set([...svg.matchAll(/\sid\s*=\s*["']([^"']+)["']/g)].map((match) => match[1]))];
  const lines = [
    "format = 1",
    `title = ${JSON.stringify(title)}`,
    `size = [128, ${symbolHeight + 44}]`,
    "",
    "# The bindings the symbol takes, each boolean, number or text, for example:",
    "[properties]",
    '# running = "boolean"',
    '# level = "number"',
    "",
    "# Ports where pipes connect, in the drawing's coordinates, for example:",
    "# [[port]]",
    '# name = "inlet"',
    "# at = [0, 32]",
    '# direction = "left"',
    "",
    "# The drawing's elements. Name the ones the bindings drive, each one way:",
    "# spin, angle, level, show, blink, alarm or active.",
  ];
  for (const id of ids) lines.push("", "# [[part]]", `# id = ${JSON.stringify(id)}`, '# spin = "running"');
  return `${lines.join("\n")}\n`;
}

/** The drawing's width and height: its viewBox, else its width and height, else a square. */
function proportions(svg: string): [number, number] {
  const box = /viewBox\s*=\s*["']\s*[-\d.eE]+[\s,]+[-\d.eE]+[\s,]+([\d.eE]+)[\s,]+([\d.eE]+)/.exec(svg);
  const sized = box ?? /<svg[^>]*\swidth\s*=\s*["']([\d.]+)[^"']*["'][^>]*\sheight\s*=\s*["']([\d.]+)/.exec(svg);
  const [width, height] = sized ? [Number(sized[1]), Number(sized[2])] : [64, 64];
  return width > 0 && height > 0 ? [width, height] : [64, 64];
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
