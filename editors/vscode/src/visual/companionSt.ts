import { getTrustConfiguration } from "../configuration";
import * as path from "path";
import * as vscode from "vscode";
import {
  generateBlocklyCompanionFunctionBlock,
  parseBlocklyWorkspaceText,
} from "./blocklyToSt";
import {
  collectLadderRuntimeGlobalDeclarations,
  generateLadderCompanionFunctionBlock,
  parseLadderProgramText,
} from "./ladderToSt";
import {
  generateStateChartCompanionFunctionBlock,
  parseStateChartText,
} from "./statechartToSt";
import {
  generateSfcCompanionFunctionBlock,
  parseSfcWorkspaceText,
} from "./sfcToSt";
import { collectDirectIoDeclarationsFromText } from "./directIo";
import {
  fbNameForSource,
  sanitizeIdentifier,
} from "./stNaming";

export type VisualSourceKind = "ladder" | "blockly" | "statechart" | "sfc";

const VISUAL_SUFFIXES: ReadonlyArray<{ kind: VisualSourceKind; suffix: string }> = [
  { kind: "ladder", suffix: ".ladder.json" },
  { kind: "blockly", suffix: ".blockly.json" },
  { kind: "statechart", suffix: ".statechart.json" },
  { kind: "sfc", suffix: ".sfc.json" },
];
const VISUAL_RUNTIME_WRAPPER_SUFFIX = ".visual.runtime.st";

function baseNameWithoutVisualSuffix(fileName: string): string {
  let baseName = fileName;
  for (const descriptor of VISUAL_SUFFIXES) {
    if (baseName.toLowerCase().endsWith(descriptor.suffix)) {
      baseName = baseName.slice(0, -descriptor.suffix.length);
    }
  }
  return baseName;
}

function autoGenerateEnabled(uri: vscode.Uri): boolean {
  return getTrustConfiguration(uri).get<boolean>(
    "visual.autoGenerateStCompanion",
    true
  );
}

export function openCompanionOnCreateEnabled(uri: vscode.Uri): boolean {
  return getTrustConfiguration(uri).get<boolean>(
    "visual.openStCompanionOnCreate",
    true
  );
}

export function visualSourceKindFor(
  documentUri: vscode.Uri
): VisualSourceKind | undefined {
  const lowerPath = documentUri.fsPath.toLowerCase();
  const descriptor = VISUAL_SUFFIXES.find(({ suffix }) =>
    lowerPath.endsWith(suffix)
  );
  return descriptor?.kind;
}

export function isVisualSourceUri(documentUri: vscode.Uri): boolean {
  return !!visualSourceKindFor(documentUri);
}

export function companionStUriFor(documentUri: vscode.Uri): vscode.Uri {
  const sourcePath = documentUri.fsPath;
  const directory = path.dirname(sourcePath);
  const sourceName = path.basename(sourcePath);
  const base = baseNameWithoutVisualSuffix(sourceName);
  const stFileName = `${base}.st`;
  return vscode.Uri.file(path.join(directory, stFileName));
}

export function visualRuntimeEntryUriFor(documentUri: vscode.Uri): vscode.Uri {
  const sourcePath = documentUri.fsPath;
  const directory = path.dirname(sourcePath);
  const sourceName = path.basename(sourcePath);
  const base = baseNameWithoutVisualSuffix(sourceName);
  const wrapperFileName = `${base}${VISUAL_RUNTIME_WRAPPER_SUFFIX}`;
  return vscode.Uri.file(path.join(directory, wrapperFileName));
}

function suffixForKind(sourceKind: VisualSourceKind): string {
  if (sourceKind === "ladder") {
    return "LADDER";
  }
  if (sourceKind === "blockly") {
    return "BLOCKLY";
  }
  if (sourceKind === "sfc") {
    return "SFC";
  }
  return "STATECHART";
}

function functionBlockTypeForVisualSource(
  sourceUri: vscode.Uri,
  sourceKind: VisualSourceKind
): string {
  const sourceName = path.basename(sourceUri.fsPath);
  const baseName = baseNameWithoutVisualSuffix(sourceName);
  return fbNameForSource(baseName, suffixForKind(sourceKind));
}

export function generateVisualRuntimeEntrySource(
  sourceUri: vscode.Uri,
  sourceKind: VisualSourceKind,
  sourceText?: string
): string {
  const sourceName = path.basename(sourceUri.fsPath);
  const baseName = baseNameWithoutVisualSuffix(sourceName);
  const baseId = sanitizeIdentifier(baseName, "Visual");
  const fbType = functionBlockTypeForVisualSource(sourceUri, sourceKind);
  const programType = sanitizeIdentifier(`PRG_${baseId}_VISUAL`, "PRG_Visual");
  const fbInstanceName = sanitizeIdentifier(`fb_${baseId}`, "fb_visual");
  const configName = sanitizeIdentifier(`CFG_${baseId}_VISUAL`, "CFG_Visual");
  const resourceName = sanitizeIdentifier(`RES_${baseId}_VISUAL`, "RES_Visual");
  const taskName = sanitizeIdentifier(`TASK_${baseId}_VISUAL`, "TASK_Visual");
  const programInstanceName = sanitizeIdentifier(
    `PLC_PRG_${baseId}`,
    "PLC_PRG_VISUAL"
  );
  const visualGlobals =
    typeof sourceText === "string"
      ? visualRuntimeGlobalDeclarations(sourceKind, sourceText)
      : [];
  const runtimeGlobals = [
    `  ${fbInstanceName} : ${fbType};`,
    ...visualGlobals,
  ];

  const output = [
    "(*",
    `  Auto-generated runtime entry for ${sourceName}.`,
    "  This wrapper is used to run visual editors through the Structured Text debugger path.",
    "*)",
    "",
    `PROGRAM ${programType}`,
    `${fbInstanceName}();`,
    "END_PROGRAM",
    "",
    `CONFIGURATION ${configName}`,
  ];

  if (runtimeGlobals.length > 0) {
    output.push("VAR_GLOBAL");
    output.push(...runtimeGlobals);
    output.push("END_VAR");
  }

  output.push(
    `  RESOURCE ${resourceName} ON PLC`,
    `    TASK ${taskName}(INTERVAL := T#20ms, PRIORITY := 1);`,
    `    PROGRAM ${programInstanceName} WITH ${taskName} : ${programType};`,
    "  END_RESOURCE",
    "END_CONFIGURATION",
    ""
  );

  return output.join("\n");
}

function visualBanner(sourceKind: VisualSourceKind, sourceUri: vscode.Uri): string {
  const sourceName = path.basename(sourceUri.fsPath);
  return [
    "(*",
    `  Auto-generated from ${sourceKind} source: ${sourceName}`,
    "  Source of truth is the visual file unless migrated fully to ST.",
    "*)",
    "",
  ].join("\n");
}

function generateVisualCompanionBody(
  sourceUri: vscode.Uri,
  sourceKind: VisualSourceKind,
  sourceText: string
): string {
  const sourceName = path.basename(sourceUri.fsPath);
  const baseName = baseNameWithoutVisualSuffix(sourceName);

  if (sourceKind === "ladder") {
    const program = parseLadderProgramText(sourceText);
    return generateLadderCompanionFunctionBlock(program, baseName);
  }

  if (sourceKind === "blockly") {
    const workspace = parseBlocklyWorkspaceText(sourceText);
    return generateBlocklyCompanionFunctionBlock(workspace, baseName);
  }

  if (sourceKind === "sfc") {
    const workspace = parseSfcWorkspaceText(sourceText);
    return generateSfcCompanionFunctionBlock(workspace, baseName);
  }

  const statechart = parseStateChartText(sourceText);
  return generateStateChartCompanionFunctionBlock(statechart, baseName);
}

async function readTextIfExists(uri: vscode.Uri): Promise<string | undefined> {
  try {
    const bytes = await vscode.workspace.fs.readFile(uri);
    return Buffer.from(bytes).toString("utf8");
  } catch {
    return undefined;
  }
}

async function readSourceText(uri: vscode.Uri): Promise<string> {
  const bytes = await vscode.workspace.fs.readFile(uri);
  return Buffer.from(bytes).toString("utf8");
}

export async function writeCompanionStFile(
  sourceUri: vscode.Uri,
  sourceKind: VisualSourceKind,
  stCode: string
): Promise<vscode.Uri> {
  const target = companionStUriFor(sourceUri);
  const content = `${visualBanner(sourceKind, sourceUri)}${stCode.trimEnd()}\n`;
  const existing = await readTextIfExists(target);
  if (existing === content) {
    return target;
  }
  await vscode.workspace.fs.writeFile(target, Buffer.from(content, "utf8"));
  return target;
}

export async function writeVisualRuntimeEntryFile(
  sourceUri: vscode.Uri,
  sourceKind: VisualSourceKind
): Promise<vscode.Uri> {
  const target = visualRuntimeEntryUriFor(sourceUri);
  const sourceText = await readSourceText(sourceUri);
  const content = generateVisualRuntimeEntrySource(
    sourceUri,
    sourceKind,
    sourceText
  );
  const existing = await readTextIfExists(target);
  if (existing === content) {
    return target;
  }
  await vscode.workspace.fs.writeFile(target, Buffer.from(content, "utf8"));
  return target;
}

function literalForLadderInitialValue(
  type: string,
  initialValue: unknown
): string | undefined {
  if (initialValue === undefined || initialValue === null) {
    return undefined;
  }

  const normalizedType = type.trim().toUpperCase();
  if (normalizedType === "BOOL") {
    if (typeof initialValue === "boolean") {
      return initialValue ? "TRUE" : "FALSE";
    }
    if (typeof initialValue === "number") {
      return initialValue !== 0 ? "TRUE" : "FALSE";
    }
    if (typeof initialValue === "string") {
      const normalized = initialValue.trim().toUpperCase();
      if (normalized === "TRUE" || normalized === "1") {
        return "TRUE";
      }
      if (normalized === "FALSE" || normalized === "0") {
        return "FALSE";
      }
    }
    return undefined;
  }

  if (normalizedType === "INT" || normalizedType === "DINT") {
    if (typeof initialValue === "number" && Number.isFinite(initialValue)) {
      return `${Math.trunc(initialValue)}`;
    }
    if (typeof initialValue === "string" && initialValue.trim().length > 0) {
      const parsed = Number(initialValue);
      if (Number.isFinite(parsed)) {
        return `${Math.trunc(parsed)}`;
      }
    }
    return undefined;
  }

  if (normalizedType === "REAL" || normalizedType === "LREAL") {
    if (typeof initialValue === "number" && Number.isFinite(initialValue)) {
      return `${initialValue}`;
    }
    if (typeof initialValue === "string" && initialValue.trim().length > 0) {
      const parsed = Number(initialValue);
      if (Number.isFinite(parsed)) {
        return `${parsed}`;
      }
    }
    return undefined;
  }

  if (normalizedType === "TIME") {
    if (typeof initialValue === "string" && initialValue.trim().length > 0) {
      return initialValue.trim();
    }
    if (typeof initialValue === "number" && Number.isFinite(initialValue)) {
      return `T#${Math.max(0, Math.trunc(initialValue))}ms`;
    }
    return undefined;
  }

  return undefined;
}

function ladderRuntimeGlobalDeclarations(sourceText: string): string[] {
  let program;
  try {
    program = parseLadderProgramText(sourceText);
  } catch {
    return [];
  }

  return collectLadderRuntimeGlobalDeclarations(program).map((declaration) => {
    const addressClause = declaration.address ? ` AT ${declaration.address}` : "";
    const initialLiteral = literalForLadderInitialValue(
      declaration.type,
      declaration.initialValue
    );
    const initialClause = initialLiteral ? ` := ${initialLiteral}` : "";

    return `  ${declaration.name}${addressClause} : ${declaration.type}${initialClause};`;
  });
}

function directIoRuntimeGlobalDeclarations(sourceText: string): string[] {
  return collectDirectIoDeclarationsFromText(sourceText).map(
    (declaration) =>
      `  ${declaration.name} AT ${declaration.address} : ${declaration.type};`
  );
}

function visualRuntimeGlobalDeclarations(
  sourceKind: VisualSourceKind,
  sourceText: string
): string[] {
  if (sourceKind === "ladder") {
    return ladderRuntimeGlobalDeclarations(sourceText);
  }
  return directIoRuntimeGlobalDeclarations(sourceText);
}

export async function syncVisualRuntimeEntryFromUri(
  sourceUri: vscode.Uri,
  options?: { force?: boolean; showErrors?: boolean }
): Promise<vscode.Uri | undefined> {
  if (sourceUri.scheme !== "file") {
    return undefined;
  }

  const sourceKind = visualSourceKindFor(sourceUri);
  if (!sourceKind) {
    return undefined;
  }

  const force = options?.force ?? false;
  if (!force && !autoGenerateEnabled(sourceUri)) {
    return undefined;
  }

  try {
    return await writeVisualRuntimeEntryFile(sourceUri, sourceKind);
  } catch (error) {
    if (options?.showErrors ?? true) {
      const reason = error instanceof Error ? error.message : String(error);
      void vscode.window.showErrorMessage(
        `Failed to generate runtime entry for ${path.basename(sourceUri.fsPath)}: ${reason}`
      );
    }
    return undefined;
  }
}

export async function syncVisualCompanionFromUri(
  sourceUri: vscode.Uri,
  options?: {
    force?: boolean;
    showErrors?: boolean;
    sourceText?: string;
  }
): Promise<vscode.Uri | undefined> {
  if (sourceUri.scheme !== "file") {
    return undefined;
  }

  const sourceKind = visualSourceKindFor(sourceUri);
  if (!sourceKind) {
    return undefined;
  }

  const force = options?.force ?? false;
  if (!force && !autoGenerateEnabled(sourceUri)) {
    return undefined;
  }

  try {
    const sourceText = options?.sourceText ?? (await readSourceText(sourceUri));
    const stCode = generateVisualCompanionBody(sourceUri, sourceKind, sourceText);
    const companionUri = await writeCompanionStFile(sourceUri, sourceKind, stCode);
    await syncVisualRuntimeEntryFromUri(sourceUri, options);
    return companionUri;
  } catch (error) {
    if (options?.showErrors ?? true) {
      const reason = error instanceof Error ? error.message : String(error);
      void vscode.window.showErrorMessage(
        `Failed to generate ST companion for ${path.basename(sourceUri.fsPath)}: ${reason}`
      );
    }
    return undefined;
  }
}

export async function syncVisualCompanionFromDocument(
  document: vscode.TextDocument,
  options?: { force?: boolean; showErrors?: boolean }
): Promise<vscode.Uri | undefined> {
  return syncVisualCompanionFromUri(document.uri, {
    ...options,
    sourceText: document.getText(),
  });
}

export async function openCompanionForVisualSource(
  sourceUri: vscode.Uri
): Promise<void> {
  const companionUri = companionStUriFor(sourceUri);
  const companionDocument = await vscode.workspace.openTextDocument(companionUri);
  await vscode.window.showTextDocument(companionDocument, { preview: false });
}

export function registerVisualCompanionSync(): vscode.Disposable {
  const saveSubscription = vscode.workspace.onDidSaveTextDocument((document) => {
    if (!isVisualSourceUri(document.uri)) {
      return;
    }
    void syncVisualCompanionFromDocument(document, {
      force: false,
      showErrors: true,
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
      const targetUri =
        parsedUri ?? vscode.window.activeTextEditor?.document.uri;
      if (!targetUri) {
        const visualSources = await vscode.workspace.findFiles(
          "**/*.{ladder,blockly,statechart,sfc}.json"
        );
        let syncedCount = 0;
        for (const visualSource of visualSources) {
          const synced = await syncVisualCompanionFromUri(visualSource, {
            force: true,
            showErrors: true,
          });
          if (synced) {
            syncedCount += 1;
          }
        }
        void vscode.window.showInformationMessage(
          `ST companion sync complete: ${syncedCount}/${visualSources.length} visual source files`
        );
        return;
      }
      if (!isVisualSourceUri(targetUri)) {
        void vscode.window.showWarningMessage(
          "Selected file is not a supported visual source (*.ladder.json, *.blockly.json, *.statechart.json, *.sfc.json)."
        );
        return;
      }
      const synced = await syncVisualCompanionFromUri(targetUri, {
        force: true,
        showErrors: true,
      });
      if (synced) {
        void vscode.window.showInformationMessage(
          `ST companion synced: ${path.basename(synced.fsPath)}`
        );
      }
    }
  );

  return vscode.Disposable.from(saveSubscription, syncCommand);
}
