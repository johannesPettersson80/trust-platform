// Bundled packages use the ordinary library manifests. Source and visuals travel together.
import * as vscode from "vscode";
import * as path from "path";
import { parseDependencyEntries, parsePackageVersion, upsertDependency } from "../../librariesModel";

interface FileState {
  uri: vscode.Uri;
  text?: string;
  document?: vscode.TextDocument;
  version?: number;
}
/** A file as an open document has it, else as the disk has it; none when it does not exist. */
async function read(uri: vscode.Uri): Promise<FileState> {
  const open = vscode.workspace.textDocuments.find((document) => document.uri.toString() === uri.toString());
  if (open) return { uri, text: open.getText(), document: open, version: open.version };
  try {
    const text = new TextDecoder("utf-8", { fatal: true }).decode(await vscode.workspace.fs.readFile(uri));
    return { uri, text };
  } catch (error) {
    if (error instanceof vscode.FileSystemError && error.code === "FileNotFound") return { uri };
    throw error;
  }
}
export async function prepareBundledLibraries(
  project: vscode.Uri,
  extensionPath: string,
  requested: Array<{ name: string; folder: string }>,
  edit: vscode.WorkspaceEdit,
): Promise<() => Promise<void>> {
  const base = vscode.Uri.file(path.join(extensionPath, "media", "libraries"));
  const before: FileState[] = [];
  const files = new Map<string, { uri: vscode.Uri; text: string }>();
  const visited = new Set<string>(),
    visiting = new Set<string>();
  const readUtf8 = async (uri: vscode.Uri): Promise<string> =>
    new TextDecoder("utf-8", { fatal: true }).decode(await vscode.workspace.fs.readFile(uri));
  const collect = async (source: vscode.Uri, target: vscode.Uri, top = false): Promise<void> => {
    for (const [name, kind] of await vscode.workspace.fs.readDirectory(source)) {
      const from = vscode.Uri.joinPath(source, name),
        to = vscode.Uri.joinPath(target, name);
      // A library's example projects document it; they are not part of a project using it.
      if (kind === vscode.FileType.Directory) {
        if (!(top && name === "examples")) await collect(from, to);
      } else if (kind === vscode.FileType.File) files.set(to.toString(), { uri: to, text: await readUtf8(from) });
    }
  };
  const packageFor = async (folder: string): Promise<string> => {
    if (!folder.split("/").every((part) => /^[a-z0-9_-]+$/.test(part)))
      throw new Error("Bundled library dependencies must stay inside the package collection.");
    const source = vscode.Uri.joinPath(base, folder);
    const manifest = await readUtf8(vscode.Uri.joinPath(source, "trust-lsp.toml"));
    if (visited.has(folder)) return manifest;
    if (visiting.has(folder)) throw new Error("The bundled library contains a dependency cycle.");
    visiting.add(folder);
    for (const dependency of parseDependencyEntries(manifest)) {
      if (!dependency.path || dependency.git) throw new Error("A bundled dependency must name a local package.");
      const resolved = path.resolve(source.fsPath, dependency.path);
      const relative = path.relative(base.fsPath, resolved);
      if (!relative || path.isAbsolute(relative) || relative.split(path.sep).some((part) => part === ".."))
        throw new Error("A bundled dependency leaves the package collection.");
      const nested = await packageFor(relative.split(path.sep).join("/"));
      if (dependency.version && parsePackageVersion(nested) !== dependency.version)
        throw new Error(`The bundled ${dependency.name} version does not match its contract.`);
    }
    await collect(source, vscode.Uri.joinPath(project, "libraries", folder), true);
    visiting.delete(folder);
    visited.add(folder);
    return manifest;
  };
  const manifest = await read(vscode.Uri.joinPath(project, "trust-lsp.toml"));
  before.push(manifest);
  let text = manifest.text ?? '[project]\ninclude_paths = ["src"]\n';
  for (const library of requested) {
    if (parseDependencyEntries(text).some((dependency) => dependency.name.toLowerCase() === library.name.toLowerCase()))
      continue;
    const packageText = await packageFor(library.folder);
    text = upsertDependency(text, library.name, {
      path: `libraries/${library.folder}`,
      version: parsePackageVersion(packageText),
    });
  }
  for (const file of files.values()) {
    const current = await read(file.uri);
    before.push(current);
    if (current.text !== undefined && current.text !== file.text)
      throw new Error(
        `${file.uri.path} differs from the bundled library. Review that library update first; your version was preserved.`,
      );
    // Install immutable package contents as resource edits. Text inserts would open every
    // bundled ST file as a dirty editor and trigger hundreds of diagnostic refreshes.
    if (current.text === undefined)
      edit.createFile(file.uri, { overwrite: false, ignoreIfExists: false, contents: Buffer.from(file.text, "utf8") });
  }
  if (text !== manifest.text) {
    // An open trust-lsp.toml is edited in its editor; otherwise the file is written as a whole,
    // existing or not, so the dependencies reach the disk the language server reads.
    if (manifest.document) edit.replace(manifest.uri, new vscode.Range(0, 0, manifest.document.lineCount, 0), text);
    else edit.createFile(manifest.uri, { overwrite: true, contents: Buffer.from(text, "utf8") });
  }
  return async () => {
    for (const file of before) {
      const current = await read(file.uri);
      if (current.text !== file.text || (file.document && file.document.version !== file.version))
        throw new Error("Project files changed while preparing the library. Your edits were preserved.");
    }
  };
}
