import { openHmiTexts } from "./textFiles";
import { remoteVariableContext } from "./remoteVariables";
// Binding problems of HMI page and popup files in the Problems view (specification 39, §7.6): the
// language server's binding checks run for a file when it opens or changes, and for every file of
// a project when one of its source files is saved. The custom editor shows the same problems on
// the canvas. While the program does not compile, the checks wait, as the variable picker does.
// The quality checks (§14.1 item 4) add warnings: a page's at its objects, the project colours'
// at their lines in `hmi/screen.toml`, and a program without an `Hmi.Display` call at its top.

import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { HmiPageModel } from "../drawing/model";
import type { HmiColorsModel } from "../drawing/tokens";
import type { Catalogue } from "./catalogue";
import { languageTables } from "./languages";
import { languageServerCatalogue, languageServerPageFiles } from "./languageServerPageFiles";
import type { ProjectTexts } from "./messages";
import { colourQualityWarnings, pageQualityWarnings, type QualityRequest, type QualityWarning } from "./quality";
import { languageServerTexts } from "./textFiles";

/** The Problems view's name for these problems. */
export const HMI_PROBLEMS_SOURCE = "truST HMI";
/** How long a changing file waits before it is checked again. */
const CHANGE_DELAY_MS = 400;

interface CheckAnswer {
  ok?: boolean;
  problems?: Array<{ line: number; object: string; property?: string | null; message: string; point?: string | null }>;
  errors?: Array<{ line?: number; message: string }>;
}

/** The folders of the files the binding checks look at: pages, popups, panels (§8.7), pairs
 * (§8.6), the definitions of the project's own symbols (§14.4) and P&IDs (§18 phase 4). */
const CHECKED_FOLDERS = ["pages", "popups", "panels", "pairs", "symbols", "pid"];

/** The project root of a page, popup, panel or pair file (`<root>/hmi/pages/<name>.toml`),
 * none for others. */
export function hmiFileRoot(file: string): string | undefined {
  const normalized = path.normalize(file);
  const folder = path.dirname(normalized);
  const hmi = path.dirname(folder);
  if (
    !normalized.endsWith(".toml") ||
    path.basename(hmi) !== "hmi" ||
    !CHECKED_FOLDERS.includes(path.basename(folder))
  ) {
    return undefined;
  }
  return path.dirname(hmi);
}

/** The project root of a text file (`<root>/hmi/texts/<language>.toml`), none for others. */
export function textsFileRoot(file: string): string | undefined {
  const normalized = path.normalize(file);
  const folder = path.dirname(normalized);
  const hmi = path.dirname(folder);
  return normalized.endsWith(".toml") && path.basename(folder) === "texts" && path.basename(hmi) === "hmi"
    ? path.dirname(hmi)
    : undefined;
}

/** The project root of a screen layout file (`<root>/hmi/screen.toml`), none for others. */
export function screenFileRoot(file: string): string | undefined {
  const normalized = path.normalize(file);
  const hmi = path.dirname(normalized);
  return path.basename(normalized) === "screen.toml" && path.basename(hmi) === "hmi" ? path.dirname(hmi) : undefined;
}

/** What the quality checks read: the catalogue and the files' models, which the drawing needs, and the language server, which
 * decides the warnings (`trust-lsp.hmiQuality`). */
export interface QualitySource {
  catalogue(): Promise<Catalogue | undefined>;
  request: QualityRequest;
  page(text: string, file: string): Promise<HmiPageModel | undefined>;
  colors(text: string): Promise<HmiColorsModel | undefined>;
  /** The project's languages and texts, for the texts cut in each language (§14.5 item 6). */
  texts?(root: string): Promise<ProjectTexts | undefined>;
}

const languageServerQuality: QualitySource = {
  catalogue: languageServerCatalogue,
  texts: (root) => languageServerTexts(vscode.Uri.file(root)),
  request: async (command, argument) => {
    try {
      return await vscode.commands.executeCommand(command, argument);
    } catch {
      return undefined;
    }
  },
  page: async (text, file) => {
    const read = await languageServerPageFiles.read(text, file);
    return read.ok ? read.model : undefined;
  },
  colors: async (text) => {
    try {
      const answer = await vscode.commands.executeCommand<{ ok?: boolean; model?: { colors?: HmiColorsModel } }>(
        "trust-lsp.hmiFileParse",
        {
          kind: "screen",
          text,
          file: "hmi/screen.toml",
        },
      );
      return answer?.ok ? (answer.model?.colors ?? {}) : undefined;
    } catch {
      return undefined;
    }
  },
};

/** The 1-based line of an object's `id`, where its warnings go. */
function objectLine(lines: string[], id: string): number {
  const at = lines.findIndex((line) =>
    new RegExp(`^\\s*id\\s*=\\s*"${id.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}"\\s*$`).test(line),
  );
  return at + 1 || 1;
}

/** The 1-based line of a project colour in `[colors.<theme>]`. */
function colourLine(lines: string[], theme: string, token: string): number {
  const table = lines.findIndex((line) => line.trim() === `[colors.${theme}]`);
  for (let index = table + 1; table >= 0 && index < lines.length && !lines[index].trim().startsWith("["); index += 1) {
    if (new RegExp(`^\\s*"?${token}"?\\s*=`).test(lines[index])) return index + 1;
  }
  return table + 1 || 1;
}

/** The project of a source file: the nearest folder above it that has HMI pages, popups,
 * panels or pairs; a library has the last two. */
function projectOfSource(file: string): string | undefined {
  let folder = path.dirname(path.normalize(file));
  for (let depth = 0; depth < 32; depth += 1) {
    if (CHECKED_FOLDERS.some((kind) => fs.existsSync(path.join(folder, "hmi", kind)))) return folder;
    const parent = path.dirname(folder);
    if (parent === folder) return undefined;
    folder = parent;
  }
  return undefined;
}

export class HmiProblems implements vscode.Disposable {
  private readonly collection = vscode.languages.createDiagnosticCollection(HMI_PROBLEMS_SOURCE);
  private readonly timers = new Map<string, ReturnType<typeof setTimeout>>();
  /** Each checked file's problems by object id, for the canvas. */
  private readonly byObject = new Map<string, Record<string, string[]>>();
  /** Each checked file's write-policy points by object id and problem, as the language server names
   * them (§12.2.3), for the inspector's fix. */
  private readonly pointsByObject = new Map<string, Record<string, Record<string, string>>>();
  /** Each checked file's write-policy points by diagnostic (`<line>:<message>`), for the quick fix. */
  private readonly pointsByDiagnostic = new Map<string, Map<string, string>>();
  /** Each checked file's quality warnings by object id, for the canvas and the inspector. */
  private readonly warningsByObject = new Map<string, Record<string, QualityWarning[]>>();
  /** Each project's colour warnings, for the Colours section. */
  private readonly colourWarningsByRoot = new Map<string, QualityWarning[]>();
  /** The latest check of each file; an older check's answer is dropped. */
  private readonly versions = new Map<string, number>();
  private readonly changed = new vscode.EventEmitter<vscode.Uri>();
  private readonly subscriptions: vscode.Disposable[];
  /** A file's problems changed. */
  readonly onDidChange = this.changed.event;

  constructor(private readonly quality: QualitySource = languageServerQuality) {
    this.subscriptions = [
      vscode.workspace.onDidOpenTextDocument((document) => this.documentChanged(document, 0)),
      vscode.workspace.onDidChangeTextDocument((event) => this.documentChanged(event.document, CHANGE_DELAY_MS)),
      vscode.workspace.onDidSaveTextDocument((document) => this.saved(document)),
      vscode.workspace.onDidCloseTextDocument((document) => this.clearIfGone(document.uri)),
      vscode.workspace.onDidDeleteFiles((event) => {
        for (const uri of event.files) this.clearIfGone(uri);
      }),
    ];
    for (const document of vscode.workspace.textDocuments) this.documentChanged(document, 0);
    for (const folder of vscode.workspace.workspaceFolders ?? []) {
      if (projectOfSource(path.join(folder.uri.fsPath, "x")) === folder.uri.fsPath)
        void this.checkProject(folder.uri.fsPath);
    }
  }

  /** The problems of a page or popup file by object id, for the canvas. */
  objectPoints(uri: vscode.Uri): Record<string, Record<string, string>> {
    return this.pointsByObject.get(uri.toString()) ?? {};
  }

  /** The point a write-policy problem names, for its quick fix (§12.2.3). */
  pointOf(uri: vscode.Uri, diagnostic: vscode.Diagnostic): string | undefined {
    return this.pointsByDiagnostic.get(uri.toString())?.get(`${diagnostic.range.start.line}:${diagnostic.message}`);
  }

  objectProblems(uri: vscode.Uri): Record<string, string[]> {
    return this.byObject.get(uri.toString()) ?? {};
  }

  /** The quality warnings of a page or popup file by object id (§14.1 item 4). */
  objectWarnings(uri: vscode.Uri): Record<string, QualityWarning[]> {
    return this.warningsByObject.get(uri.toString()) ?? {};
  }

  /** The warnings about a project's own colours (§14.1 item 4). */
  colourWarnings(root: string): QualityWarning[] {
    return this.colourWarningsByRoot.get(path.normalize(root)) ?? [];
  }

  /** Checks every page, popup, faceplate and pair file of the project at `root`, and its screen
   * layout's colours. */
  async checkProject(root: string): Promise<void> {
    const screen = vscode.Uri.file(path.join(root, "hmi", "screen.toml"));
    const openScreen = vscode.workspace.textDocuments.find((document) => document.uri.toString() === screen.toString());
    try {
      await this.checkScreen(screen, openScreen?.getText() ?? fs.readFileSync(screen.fsPath, "utf8"));
    } catch {
      // A project without a screen layout has no project colours.
    }
    for (const kind of CHECKED_FOLDERS) {
      const folder = path.join(root, "hmi", kind);
      let names: string[] = [];
      try {
        names = fs.readdirSync(folder).filter((name) => name.endsWith(".toml"));
      } catch {
        continue;
      }
      for (const name of names.sort()) {
        const uri = vscode.Uri.file(path.join(folder, name));
        const open = vscode.workspace.textDocuments.find((document) => document.uri.toString() === uri.toString());
        let text: string;
        try {
          text = open?.getText() ?? fs.readFileSync(uri.fsPath, "utf8");
        } catch {
          continue;
        }
        await this.check(uri, text);
      }
    }
  }

  dispose(): void {
    this.timers.forEach((timer) => clearTimeout(timer));
    this.subscriptions.forEach((subscription) => subscription.dispose());
    this.collection.dispose();
    this.changed.dispose();
  }

  /** Undo can delete a created file after its final empty-buffer change was scheduled. */
  private clearIfGone(uri: vscode.Uri): boolean {
    if (
      uri.scheme !== "file" ||
      !(hmiFileRoot(uri.fsPath) || screenFileRoot(uri.fsPath) || textsFileRoot(uri.fsPath)) ||
      fs.existsSync(uri.fsPath)
    )
      return false;
    const key = uri.toString();
    if (
      vscode.workspace.textDocuments.some(
        (document) => !document.isClosed && document.isDirty && document.uri.toString() === key,
      )
    )
      return false;
    const timer = this.timers.get(key);
    if (timer) clearTimeout(timer);
    this.timers.delete(key);
    this.versions.set(key, (this.versions.get(key) ?? 0) + 1);
    this.collection.delete(uri);
    this.byObject.delete(key);
    this.pointsByObject.delete(key);
    this.pointsByDiagnostic.delete(key);
    this.warningsByObject.delete(key);
    const root = screenFileRoot(uri.fsPath);
    if (root) this.colourWarningsByRoot.delete(path.normalize(root));
    this.changed.fire(uri);
    return true;
  }

  private documentChanged(document: vscode.TextDocument, delay: number): void {
    const file = document.uri.fsPath;
    if (document.uri.scheme !== "file" || !(hmiFileRoot(file) || screenFileRoot(file) || textsFileRoot(file))) return;
    const key = document.uri.toString();
    const pending = this.timers.get(key);
    if (pending) clearTimeout(pending);
    this.timers.set(
      key,
      setTimeout(() => {
        this.timers.delete(key);
        if (this.clearIfGone(document.uri)) return;
        if (textsFileRoot(file)) void this.checkTexts(document.uri, document.getText());
        else
          void (screenFileRoot(file)
            ? this.checkScreen(document.uri, document.getText())
            : this.check(document.uri, document.getText()));
      }, delay),
    );
  }

  private saved(document: vscode.TextDocument): void {
    const file = document.uri.fsPath;
    if (document.uri.scheme !== "file" || hmiFileRoot(file)) return;
    // The write policy lives in runtime.toml, which the checks read from disk (§12.2.3).
    if (path.basename(file) === "runtime.toml") {
      if (fs.existsSync(path.join(path.dirname(file), "hmi"))) void this.checkProject(path.dirname(file));
      return;
    }
    if (!/\.st$/i.test(file)) return;
    const root = projectOfSource(file);
    if (!root) return;
    void this.checkProject(root);
  }

  /** The warnings about the project colours of a screen layout, at their lines. */
  private async checkScreen(uri: vscode.Uri, text: string): Promise<void> {
    const root = screenFileRoot(uri.fsPath);
    if (!root) return;
    const key = uri.toString();
    const version = (this.versions.get(key) ?? 0) + 1;
    this.versions.set(key, version);
    const [colors, catalogue] = await Promise.all([this.quality.colors(text), this.quality.catalogue()]);
    if (this.clearIfGone(uri) || this.versions.get(key) !== version || !colors || !catalogue) return;
    const { warnings, display } = await colourQualityWarnings(
      this.quality.request,
      vscode.Uri.file(root).toString(),
      colors,
      catalogue,
    );
    if (this.clearIfGone(uri) || this.versions.get(key) !== version) return;
    const lines = text.split(/\r?\n/);
    const warning = (line: number, message: string) => {
      const diagnostic = new vscode.Diagnostic(
        new vscode.Range(line, 0, line, lines[line]?.length ?? 0),
        message,
        vscode.DiagnosticSeverity.Warning,
      );
      diagnostic.source = HMI_PROBLEMS_SOURCE;
      return diagnostic;
    };
    this.collection.set(uri, [
      ...(display ? [warning(0, display)] : []),
      ...warnings.map((found) =>
        warning(
          Math.max(0, colourLine(lines, found.colour?.theme ?? "", found.colour?.token ?? "") - 1),
          found.message,
        ),
      ),
    ]);
    this.colourWarningsByRoot.set(path.normalize(root), warnings);
    this.changed.fire(uri);
  }

  /**
   * A text file's errors at their lines (§14.5 item 8); a change to its texts can cut a text on any
   * page, so the project's pages are checked again.
   */
  private async checkTexts(uri: vscode.Uri, text: string): Promise<void> {
    const root = textsFileRoot(uri.fsPath);
    if (!root) return;
    const key = uri.toString();
    const version = (this.versions.get(key) ?? 0) + 1;
    this.versions.set(key, version);
    const file = path.relative(root, uri.fsPath).split(path.sep).join("/");
    let answer: { ok?: boolean; errors?: Array<{ line?: number; message: string }> } | undefined;
    try {
      answer = await vscode.commands.executeCommand("trust-lsp.hmiFileParse", { kind: "texts", text, file });
    } catch {
      return;
    }
    if (this.clearIfGone(uri) || this.versions.get(key) !== version) return;
    const lines = text.split(/\r?\n/);
    this.collection.set(
      uri,
      (answer?.ok ? [] : (answer?.errors ?? [])).map((error) => {
        const index = Math.min(Math.max(0, (error.line ?? 1) - 1), Math.max(0, lines.length - 1));
        const diagnostic = new vscode.Diagnostic(
          new vscode.Range(index, 0, index, lines[index]?.length ?? 0),
          error.message,
          vscode.DiagnosticSeverity.Error,
        );
        diagnostic.source = HMI_PROBLEMS_SOURCE;
        return diagnostic;
      }),
    );
    this.changed.fire(uri);
    await this.checkProject(root);
  }

  /** The quality warnings of a page or popup file that loads, by object id; the texts cut are
   * looked for in each of the project's languages (§14.5 item 6). */
  private async pageWarnings(
    root: string,
    text: string,
    file: string,
    view: "page" | "pid",
  ): Promise<QualityWarning[]> {
    const [model, catalogue, texts] = await Promise.all([
      this.quality.page(text, file),
      this.quality.catalogue(),
      this.quality.texts?.(root).catch(() => undefined),
    ]);
    if (!model || !catalogue) return [];
    const languages = texts ? languageTables(texts) : undefined;
    return pageQualityWarnings(
      this.quality.request,
      vscode.Uri.file(root).toString(),
      model,
      catalogue,
      languages,
      view,
    );
  }

  private async check(uri: vscode.Uri, text: string): Promise<void> {
    const root = hmiFileRoot(uri.fsPath);
    if (!root) return;
    const key = uri.toString();
    const version = (this.versions.get(key) ?? 0) + 1;
    this.versions.set(key, version);
    const file = path.relative(root, uri.fsPath).split(path.sep).join("/");
    let answer: CheckAnswer | undefined;
    try {
      answer = await vscode.commands.executeCommand<CheckAnswer>("trust-lsp.hmiFileCheck", {
        authoring: true,
        remote: (await remoteVariableContext(root)).catalogue,
        text,
        file,
        root_uri: vscode.Uri.file(root).toString(),
        open: openHmiTexts(vscode.Uri.file(root)),
      });
    } catch {
      // The language server is not running yet; the next change or save checks again.
      return;
    }
    if (this.clearIfGone(uri) || this.versions.get(key) !== version) return;
    const lines = text.split(/\r?\n/);
    const at = (line: number, message: string): vscode.Diagnostic => {
      const index = Math.min(Math.max(0, line - 1), Math.max(0, lines.length - 1));
      const diagnostic = new vscode.Diagnostic(
        new vscode.Range(index, 0, index, lines[index]?.length ?? 0),
        message,
        vscode.DiagnosticSeverity.Error,
      );
      diagnostic.source = HMI_PROBLEMS_SOURCE;
      return diagnostic;
    };
    let diagnostics: vscode.Diagnostic[];
    const objects: Record<string, string[]> = {};
    const points: Record<string, Record<string, string>> = {};
    const byDiagnostic = new Map<string, string>();
    if (answer?.ok) {
      diagnostics = (answer.problems ?? []).map((problem) => {
        objects[problem.object] = [...(objects[problem.object] ?? []), problem.message];
        const diagnostic = at(problem.line, problem.message);
        if (problem.point) {
          points[problem.object] = { ...(points[problem.object] ?? {}), [problem.message]: problem.point };
          byDiagnostic.set(`${diagnostic.range.start.line}:${problem.message}`, problem.point);
        }
        return diagnostic;
      });
    } else {
      const located = (answer?.errors ?? []).filter((error) => typeof error.line === "number");
      // Without a line the program does not compile or the project cannot be read: wait for it.
      if (located.length === 0) return;
      diagnostics = located.map((error) => at(error.line ?? 1, error.message));
    }
    this.collection.set(uri, diagnostics);
    this.byObject.set(key, objects);
    this.pointsByObject.set(key, points);
    this.pointsByDiagnostic.set(key, byDiagnostic);
    this.changed.fire(uri);
    // A pair has no objects to warn about.
    if (!answer?.ok || path.basename(path.dirname(uri.fsPath)) === "pairs") return;
    // The quality checks' warnings follow, at each object's id line (§14.1 item 4); the errors
    // never wait for them, since they need the program's variables.
    const warnings = await this.pageWarnings(
      root,
      text,
      file,
      path.basename(path.dirname(uri.fsPath)) === "pid" ? "pid" : "page",
    );
    if (this.clearIfGone(uri) || this.versions.get(key) !== version) return;
    const warned: Record<string, QualityWarning[]> = {};
    const flagged = warnings.flatMap((warning) => {
      if (!warning.object) return [];
      warned[warning.object] = [...(warned[warning.object] ?? []), warning];
      const diagnostic = at(objectLine(lines, warning.object), warning.message);
      diagnostic.severity = vscode.DiagnosticSeverity.Warning;
      return [diagnostic];
    });
    this.collection.set(uri, [...diagnostics, ...flagged]);
    this.warningsByObject.set(key, warned);
    this.changed.fire(uri);
  }
}
