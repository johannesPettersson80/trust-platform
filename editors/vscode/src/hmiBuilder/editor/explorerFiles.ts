import { prepareBundledLibraries } from "./libraryEdit";
// Explorer owns page/panel organisation; native formats own their text.
import * as vscode from "vscode";
import type { HmiPageModel } from "../drawing/model";
import { sameModel } from "./pageDocument";

const VIEW = "trust-lsp.hmiBuilder";
const NAME = /^[A-Za-z0-9_-]{1,64}$/;
interface HmiAddress {
  project: vscode.Uri;
  folder: string;
  name: string;
}
function address(uri: vscode.Uri): HmiAddress | undefined {
  const match = /^(.*)\/hmi\/(pages|panels|popups)\/([^/]+)\.toml$/.exec(uri.path);
  return match ? { project: uri.with({ path: match[1] }), folder: match[2], name: match[3] } : undefined;
}
function projectOf(uri?: vscode.Uri): vscode.Uri | undefined {
  if (uri) {
    const index = uri.path.indexOf("/hmi");
    if (index >= 0 && (uri.path.length === index + 4 || uri.path[index + 4] === "/"))
      return uri.with({ path: uri.path.slice(0, index) });
    return vscode.workspace.getWorkspaceFolder(uri)?.uri;
  }
  return vscode.workspace.workspaceFolders?.length === 1 ? vscode.workspace.workspaceFolders[0].uri : undefined;
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
      validateInput: (value) => (NAME.test(value) ? undefined : "Use 1–64 letters, digits, - or _."),
    }));
  if (!name) return;
  if (!NAME.test(name)) throw new Error("Invalid HMI file name.");
  const file = vscode.Uri.joinPath(project, "hmi", kind === "page" ? "pages" : "panels", `${name}.toml`);
  const title = name.replace(/[-_]/g, " ").replace(/^./, (letter) => letter.toUpperCase());
  const model: HmiPageModel = {
    title,
    object: [],
    pipe: [],
    ...(kind === "panel"
      ? {
          size: [240, 720] as [number, number],
          fields: {},
          panel: { scope: "manual" as const, pages: [], dock: "left" as const, order: 0 },
        }
      : {}),
  };
  const formatted = await vscode.commands.executeCommand<{ ok: boolean; text?: string }>("trust-lsp.hmiFileFormat", {
    kind,
    model,
  });
  if (!formatted?.ok || formatted.text === undefined) throw new Error("The HMI definition could not be created.");
  const edit = new vscode.WorkspaceEdit();
  edit.createFile(file, { overwrite: false, ignoreIfExists: false });
  edit.insert(file, new vscode.Position(0, 0), formatted.text);
  const screen = vscode.Uri.joinPath(project, "hmi", "screen.toml");
  try {
    await vscode.workspace.fs.stat(screen);
  } catch (error) {
    if (!(error instanceof vscode.FileSystemError) || error.code !== "FileNotFound") throw error;
    edit.createFile(screen, { overwrite: false, ignoreIfExists: false });
    edit.insert(
      screen,
      new vscode.Position(0, 0),
      'format = 1\nsize = [1280, 720]\ntheme = "dark"\nsymbols = "flat"\n\n[navigation]\nstyle = "none"\n',
    );
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

/** Preserve references, including sidebar assignments, through ordinary Explorer rename. */
export function renamedReferences(
  model: Record<string, unknown>,
  folder: string,
  before: string,
  after: string,
): Record<string, unknown> {
  const next = structuredClone(model);
  if (folder === "pages") {
    if (next.start_page === before) next.start_page = after;
    const panel = next.panel as { pages?: string[] } | undefined;
    if (panel?.pages) panel.pages = panel.pages.map((name) => (name === before ? after : name));
  }
  for (const object of (next.object ?? []) as Array<{ faceplate?: string; click?: Record<string, string> }>) {
    if (folder !== "pages" && object.faceplate === before) object.faceplate = after;
    const click = object.click;
    if (!click) continue;
    if (folder === "pages" && click.page === before) click.page = after;
    if (folder !== "pages") for (const key of ["popup", "faceplate"]) if (click[key] === before) click[key] = after;
  }
  if (folder !== "pages" && next.faceplate === before) next.faceplate = after;
  return next;
}

async function referenceEdits(
  renames: readonly { oldUri: vscode.Uri; newUri: vscode.Uri }[],
): Promise<vscode.WorkspaceEdit> {
  const edit = new vscode.WorkspaceEdit();
  const changes = new Map<
    string,
    { document: vscode.TextDocument; version: number; kind: string; model: Record<string, unknown> }
  >();
  for (const rename of renames) {
    const old = address(rename.oldUri),
      next = address(rename.newUri);
    if (!old || !next || old.folder !== next.folder || old.project.toString() !== next.project.toString()) continue;
    const files = await vscode.workspace.findFiles(
      new vscode.RelativePattern(old.project, "hmi/{screen.toml,{pages,panels,popups,pairs}/*.toml}"),
    );
    for (const uri of files) {
      const key = uri.toString();
      let entry = changes.get(key);
      if (!entry) {
        const document = await vscode.workspace.openTextDocument(uri);
        const version = document.version;
        const kind = uri.path.endsWith("/screen.toml")
          ? "screen"
          : uri.path.includes("/pairs/")
            ? "pair"
            : (address(uri)?.folder.replace(/s$/, "") ?? "page");
        const parsed = await vscode.commands.executeCommand<{ ok: boolean; model?: Record<string, unknown> }>(
          "trust-lsp.hmiFileParse",
          { kind, text: document.getText(), file: uri.path },
        );
        if (!parsed?.ok || !parsed.model) continue;
        entry = { document, version, kind, model: parsed.model };
      }
      const updated = renamedReferences(entry.model, old.folder, old.name, next.name);
      if (!sameModel(updated, entry.model)) changes.set(key, { ...entry, model: updated });
    }
  }
  for (const { document, version, kind, model } of changes.values()) {
    const result = await vscode.commands.executeCommand<{ ok: boolean; text?: string }>("trust-lsp.hmiFileFormat", {
      kind,
      model,
    });
    if (!result?.ok || result.text === undefined) throw new Error("HMI references could not be updated.");
    if (document.version !== version)
      throw new Error("An HMI file changed during rename. Your edits were preserved; review its page links.");
    edit.replace(document.uri, new vscode.Range(0, 0, document.lineCount, 0), result.text);
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
