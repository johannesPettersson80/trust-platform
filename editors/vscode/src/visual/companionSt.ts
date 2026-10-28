// The Structured Text companion and runtime entry of each visual program (specification 17 §3,
// specification 25 §11.4). The language server writes their text (`trust-lsp.visualCompanion`);
// this module writes the two files beside the program, on save and on request, and opens them.

import { getTrustConfiguration } from "../configuration";
import * as path from "path";
import * as vscode from "vscode";
import { problemText, visualRequest, type VisualAnswer } from "./visualCommands";

interface GeneratedFile {
  file: string;
  text: string;
}

/** The answer of `trust-lsp.visualCompanion`. */
export interface CompanionAnswer extends VisualAnswer {
  kind?: string;
  notVisual?: boolean;
  companion?: GeneratedFile;
  runtimeEntry?: GeneratedFile;
  warnings?: string[];
}

interface SourcesAnswer extends VisualAnswer {
  glob?: string;
}

function autoGenerateEnabled(uri: vscode.Uri): boolean {
  return getTrustConfiguration(uri).get<boolean>("visual.autoGenerateStCompanion", true);
}

export function openCompanionOnCreateEnabled(uri: vscode.Uri): boolean {
  return getTrustConfiguration(uri).get<boolean>("visual.openStCompanionOnCreate", true);
}

let sourceGlob: string | undefined;

/** The glob of every visual program, as the language server names it. */
export async function visualSourceGlob(): Promise<string | undefined> {
  if (!sourceGlob) {
    const answer = await visualRequest<SourcesAnswer>("trust-lsp.visualSources", {});
    sourceGlob = answer.ok ? answer.glob : undefined;
  }
  return sourceGlob;
}

/** The companion and runtime entry of the program at `uri` whose text is `text`. */
export async function requestCompanion(uri: vscode.Uri, text: string): Promise<CompanionAnswer> {
  return visualRequest<CompanionAnswer>("trust-lsp.visualCompanion", { file: uri.fsPath, text });
}

async function readText(uri: vscode.Uri): Promise<string | undefined> {
  try {
    return Buffer.from(await vscode.workspace.fs.readFile(uri)).toString("utf8");
  } catch {
    return undefined;
  }
}

async function writeIfChanged(uri: vscode.Uri, text: string): Promise<void> {
  if ((await readText(uri)) === text) {
    return;
  }
  await vscode.workspace.fs.writeFile(uri, Buffer.from(text, "utf8"));
}

/**
 * Writes the companion and runtime entry of a language-server answer beside the program at
 * `sourceUri` and answers the companion's URI; a failed answer shows why (unless `showErrors` is
 * false), except for a file that is no visual program.
 */
async function writeCompanionFiles(
  sourceUri: vscode.Uri,
  answer: CompanionAnswer,
  showErrors: boolean
): Promise<vscode.Uri | undefined> {
  const name = path.basename(sourceUri.fsPath);
  if (!answer.ok || !answer.companion || !answer.runtimeEntry) {
    if (!answer.notVisual && showErrors) {
      void vscode.window.showErrorMessage(
        `Failed to generate ST companion for ${name}: ${problemText(answer)}`
      );
    }
    return undefined;
  }
  const folder = path.dirname(sourceUri.fsPath);
  const companionUri = vscode.Uri.file(path.join(folder, answer.companion.file));
  try {
    await writeIfChanged(companionUri, answer.companion.text);
    await writeIfChanged(vscode.Uri.file(path.join(folder, answer.runtimeEntry.file)), answer.runtimeEntry.text);
  } catch (error) {
    if (showErrors) {
      const reason = error instanceof Error ? error.message : String(error);
      void vscode.window.showErrorMessage(`Failed to write the ST companion of ${name}: ${reason}`);
    }
    return undefined;
  }
  return companionUri;
}

async function companionAnswerFor(sourceUri: vscode.Uri, sourceText?: string): Promise<CompanionAnswer> {
  const text = sourceText ?? (await readText(sourceUri));
  return text === undefined
    ? { ok: false, errors: [{ message: `${path.basename(sourceUri.fsPath)} cannot be read.` }] }
    : requestCompanion(sourceUri, text);
}

/**
 * Writes the companion and runtime entry of the visual program at `sourceUri` beside it and answers
 * the companion's URI. Without `force` it does nothing while the companion setting is off. A file
 * that is no visual program answers `undefined` silently; a program that cannot be translated shows
 * why (unless `showErrors` is false).
 */
export async function syncVisualCompanionFromUri(
  sourceUri: vscode.Uri,
  options?: { force?: boolean; showErrors?: boolean; sourceText?: string }
): Promise<vscode.Uri | undefined> {
  if (sourceUri.scheme !== "file") {
    return undefined;
  }
  if (!(options?.force ?? false) && !autoGenerateEnabled(sourceUri)) {
    return undefined;
  }
  const answer = await companionAnswerFor(sourceUri, options?.sourceText);
  return writeCompanionFiles(sourceUri, answer, options?.showErrors ?? true);
}

/**
 * Opens a companion. With `beside`, it opens in the group beside the active one and leaves the
 * focus where it is, so the visual editor stays in front.
 */
export async function openCompanion(
  companionUri: vscode.Uri,
  options?: { beside?: boolean }
): Promise<void> {
  const companionDocument = await vscode.workspace.openTextDocument(companionUri);
  await vscode.window.showTextDocument(
    companionDocument,
    options?.beside
      ? { viewColumn: vscode.ViewColumn.Beside, preserveFocus: true, preview: false }
      : { preview: false }
  );
}

async function isVisualSource(document: vscode.TextDocument): Promise<boolean> {
  const glob = await visualSourceGlob();
  return !!glob && vscode.languages.match({ scheme: "file", pattern: glob }, document) > 0;
}

export function registerVisualCompanionSync(): vscode.Disposable {
  const saveSubscription = vscode.workspace.onDidSaveTextDocument(async (document) => {
    if (!(await isVisualSource(document))) {
      return;
    }
    await syncVisualCompanionFromUri(document.uri, {
      force: false,
      showErrors: true,
      sourceText: document.getText(),
    });
  });

  const syncCommand = vscode.commands.registerCommand(
    "trust-lsp.visual.syncCompanionSt",
    async (uri?: vscode.Uri | string) => {
      const parsedUri =
        typeof uri === "string"
          ? uri.includes("://")
            ? vscode.Uri.parse(uri)
            : vscode.Uri.file(uri)
          : uri;
      const targetUri = parsedUri ?? vscode.window.activeTextEditor?.document.uri;
      if (!targetUri) {
        const glob = await visualSourceGlob();
        if (!glob) {
          void vscode.window.showErrorMessage("The truST language server is not running yet.");
          return;
        }
        const visualSources = await vscode.workspace.findFiles(glob);
        let syncedCount = 0;
        for (const visualSource of visualSources) {
          if (await syncVisualCompanionFromUri(visualSource, { force: true, showErrors: true })) {
            syncedCount += 1;
          }
        }
        void vscode.window.showInformationMessage(
          `ST companion sync complete: ${syncedCount}/${visualSources.length} visual source files`
        );
        return;
      }
      const answer = await companionAnswerFor(targetUri);
      if (answer.notVisual) {
        void vscode.window.showWarningMessage(problemText(answer));
        return;
      }
      const synced = await writeCompanionFiles(targetUri, answer, true);
      if (synced) {
        void vscode.window.showInformationMessage(`ST companion synced: ${path.basename(synced.fsPath)}`);
      }
    }
  );

  return vscode.Disposable.from(saveSubscription, syncCommand);
}
