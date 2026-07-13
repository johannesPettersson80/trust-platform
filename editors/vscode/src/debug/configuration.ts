import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import {
  runtimeSourceOptionsForTarget,
  type RuntimeSourceOptions,
} from "../runtimeSourceOptions";

export const DEBUG_TYPE = "structured-text";
const DEBUG_CHANNEL = "Structured Text Debugger";
const ST_GLOB = "**/*.{st,ST,pou,POU}";
const ST_EXCLUDE_GLOB = "**/{node_modules,target,.git}/**";
const PRAGMA_SCAN_LINES = 20;
const LAST_CONFIG_KEY = "trust-lsp.lastConfigurationUri";

type RuntimeControlConfig = {
  endpoint?: string;
  authToken?: string;
};

let output: vscode.OutputChannel | undefined;
let workspaceState: vscode.Memento | undefined;
let lastStructuredTextUri: vscode.Uri | undefined;
const lastReloadedProgram = new Map<string, string>();

export function initializeDebugConfigurationState(
  state: vscode.Memento | undefined
): void {
  workspaceState = state;
}

export function debugChannel(): vscode.OutputChannel {
  if (!output) {
    output = vscode.window.createOutputChannel(DEBUG_CHANNEL);
  }
  return output;
}

export function captureStructuredTextEditor(
  editor: vscode.TextEditor | undefined
): void {
  if (!editor) {
    return;
  }
  if (editor.document.languageId === "structured-text") {
    lastStructuredTextUri = editor.document.uri;
  }
}

export function preferredStructuredTextUri(
  preferredFolder?: vscode.WorkspaceFolder
): vscode.Uri | undefined {
  const active = vscode.window.activeTextEditor;
  if (
    active &&
    active.document.languageId === "structured-text" &&
    uriBelongsToFolder(active.document.uri, preferredFolder)
  ) {
    return active.document.uri;
  }
  return lastStructuredTextUri &&
    uriBelongsToFolder(lastStructuredTextUri, preferredFolder)
    ? lastStructuredTextUri
    : undefined;
}

function uriBelongsToFolder(
  uri: vscode.Uri,
  preferredFolder?: vscode.WorkspaceFolder
): boolean {
  if (!preferredFolder) {
    return true;
  }
  return (
    vscode.workspace.getWorkspaceFolder(uri)?.uri.toString() ===
    preferredFolder.uri.toString()
  );
}

export function runtimeSourceOptions(target?: vscode.Uri): RuntimeSourceOptions {
  return runtimeSourceOptionsForTarget(target);
}

function findRuntimeToml(folder?: vscode.WorkspaceFolder): string | undefined {
  if (!folder) {
    return undefined;
  }
  const root = folder.uri.fsPath;
  const direct = path.join(root, "runtime.toml");
  if (fs.existsSync(direct)) {
    return direct;
  }
  const bundle = path.join(root, "bundle", "runtime.toml");
  if (fs.existsSync(bundle)) {
    return bundle;
  }
  return undefined;
}

export function loadRuntimeControlConfig(
  folder?: vscode.WorkspaceFolder
): RuntimeControlConfig | undefined {
  const runtimeToml = findRuntimeToml(folder);
  if (!runtimeToml) {
    return undefined;
  }
  try {
    const text = fs.readFileSync(runtimeToml, "utf8");
    return parseRuntimeControl(text);
  } catch {
    return undefined;
  }
}

function parseRuntimeControl(text: string): RuntimeControlConfig {
  const config: RuntimeControlConfig = {};
  let section = "";
  const lines = text.split(/\r?\n/);
  for (const raw of lines) {
    const line = stripInlineComment(raw).trim();
    if (!line) {
      continue;
    }
    if (line.startsWith("[") && line.endsWith("]")) {
      section = line.slice(1, -1).trim();
      continue;
    }
    if (section !== "runtime.control") {
      continue;
    }
    const match = line.match(/^([A-Za-z0-9_]+)\s*=\s*(.+)$/);
    if (!match) {
      continue;
    }
    const key = match[1];
    const value = parseTomlString(match[2]);
    if (!value) {
      continue;
    }
    if (key === "endpoint") {
      config.endpoint = value;
    } else if (key === "auth_token") {
      config.authToken = value;
    }
  }
  return config;
}

function stripInlineComment(line: string): string {
  let inSingle = false;
  let inDouble = false;
  for (let i = 0; i < line.length; i += 1) {
    const ch = line[i];
    if (ch === "'" && !inDouble) {
      inSingle = !inSingle;
    } else if (ch === '"' && !inSingle) {
      inDouble = !inDouble;
    } else if (ch === "#" && !inSingle && !inDouble) {
      return line.slice(0, i);
    }
  }
  return line;
}

function parseTomlString(value: string): string | undefined {
  const trimmed = value.trim();
  if (
    (trimmed.startsWith('"') && trimmed.endsWith('"')) ||
    (trimmed.startsWith("'") && trimmed.endsWith("'"))
  ) {
    return trimmed.slice(1, -1);
  }
  return undefined;
}

async function findStructuredTextUris(
  preferredFolder?: vscode.WorkspaceFolder
): Promise<vscode.Uri[]> {
  const workspaceFolders = vscode.workspace.workspaceFolders;
  if (!workspaceFolders || workspaceFolders.length === 0) {
    return [];
  }
  if (preferredFolder) {
    return vscode.workspace.findFiles(
      new vscode.RelativePattern(preferredFolder, ST_GLOB),
      new vscode.RelativePattern(preferredFolder, ST_EXCLUDE_GLOB)
    );
  }
  return vscode.workspace.findFiles(ST_GLOB, ST_EXCLUDE_GLOB);
}

async function readStructuredText(
  uri: vscode.Uri
): Promise<string | undefined> {
  const openDoc = vscode.workspace.textDocuments.find(
    (doc) => doc.uri.toString() === uri.toString()
  );
  if (openDoc) {
    return openDoc.getText();
  }
  try {
    const data = await vscode.workspace.fs.readFile(uri);
    return new TextDecoder("utf-8").decode(data);
  } catch {
    return undefined;
  }
}

function containsConfiguration(source: string): boolean {
  return /\bCONFIGURATION\b/i.test(source);
}

async function findConfigurationUris(
  preferredFolder?: vscode.WorkspaceFolder
): Promise<vscode.Uri[]> {
  const uris = await findStructuredTextUris(preferredFolder);
  const configs: vscode.Uri[] = [];
  for (const uri of uris) {
    const text = await readStructuredText(uri);
    if (text && containsConfiguration(text)) {
      configs.push(uri);
    }
  }
  return configs;
}

export async function isConfigurationFile(uri: vscode.Uri): Promise<boolean> {
  const text = await readStructuredText(uri);
  return !!text && containsConfiguration(text);
}

type ProgramTypeOption = {
  name: string;
  uri: vscode.Uri;
};

function buildGlobAlternation(globs: string[]): string | undefined {
  const normalized = globs.map((glob) => glob.trim()).filter(Boolean);
  if (normalized.length === 0) {
    return undefined;
  }
  if (normalized.length === 1) {
    return normalized[0];
  }
  return `{${normalized.join(",")}}`;
}

async function hasRuntimeIgnorePragma(
  uri: vscode.Uri,
  pragmas: string[]
): Promise<boolean> {
  if (pragmas.length === 0) {
    return false;
  }
  const text = await readStructuredText(uri);
  if (!text) {
    return false;
  }
  const lines = text.split(/\r?\n/).slice(0, PRAGMA_SCAN_LINES);
  for (const line of lines) {
    for (const pragma of pragmas) {
      if (pragma && line.includes(pragma)) {
        return true;
      }
    }
  }
  return false;
}

async function collectRuntimeSourceUris(
  target?: vscode.Uri
): Promise<vscode.Uri[]> {
  const runtimeOptions = runtimeSourceOptions(target);
  const includeGlobs = runtimeOptions.runtimeIncludeGlobs ?? [];
  const excludeGlobs = runtimeOptions.runtimeExcludeGlobs ?? [];
  const ignorePragmas = runtimeOptions.runtimeIgnorePragmas ?? [];
  const runtimeRoot = runtimeOptions.runtimeRoot;
  if (!runtimeRoot) {
    return [];
  }
  const baseUri = vscode.Uri.file(runtimeRoot);
  const excludePattern = buildGlobAlternation(excludeGlobs);
  const exclude = excludePattern
    ? new vscode.RelativePattern(baseUri, excludePattern)
    : undefined;
  const patterns = includeGlobs.length > 0 ? includeGlobs : [ST_GLOB];

  const candidates: vscode.Uri[] = [];
  for (const include of patterns) {
    const pattern = new vscode.RelativePattern(baseUri, include);
    const matches = await vscode.workspace.findFiles(pattern, exclude);
    candidates.push(...matches);
  }

  const unique = new Map<string, vscode.Uri>();
  for (const candidate of candidates) {
    unique.set(candidate.fsPath, candidate);
  }
  if (target?.fsPath) {
    unique.set(target.fsPath, target);
  }

  const filtered: vscode.Uri[] = [];
  for (const candidate of unique.values()) {
    if (target && candidate.fsPath === target.fsPath) {
      filtered.push(candidate);
      continue;
    }
    if (await hasRuntimeIgnorePragma(candidate, ignorePragmas)) {
      continue;
    }
    filtered.push(candidate);
  }
  return filtered;
}

function collectProgramTypesFromSource(
  source: string,
  uri: vscode.Uri
): ProgramTypeOption[] {
  const programRegex =
    /\bPROGRAM\s+([A-Za-z_][A-Za-z0-9_]*)\b(?!\s+WITH\b)/gi;
  const results: ProgramTypeOption[] = [];
  let match: RegExpExecArray | null;
  while ((match = programRegex.exec(source)) !== null) {
    const name = match[1];
    if (name) {
      results.push({ name, uri });
    }
  }
  return results;
}

async function collectProgramTypes(
  sourceUris?: vscode.Uri[]
): Promise<ProgramTypeOption[]> {
  const uris = sourceUris ?? (await collectRuntimeSourceUris());
  const programs = new Map<string, ProgramTypeOption>();
  for (const uri of uris) {
    const text = await readStructuredText(uri);
    if (!text) {
      continue;
    }
    for (const entry of collectProgramTypesFromSource(text, uri)) {
      if (!programs.has(entry.name)) {
        programs.set(entry.name, entry);
      }
    }
  }
  return Array.from(programs.values());
}

function relativePathLabel(uri: vscode.Uri): string {
  const workspaceFolder = vscode.workspace.getWorkspaceFolder(uri);
  if (!workspaceFolder) {
    return uri.fsPath;
  }
  const relative = path.relative(workspaceFolder.uri.fsPath, uri.fsPath);
  return relative || path.basename(uri.fsPath);
}

type SelectionMode = "interactive" | "auto";

function isInteractiveMode(mode: SelectionMode): boolean {
  return mode === "interactive";
}

export function selectWorkspaceFolderPathForMode(
  mode: SelectionMode,
  folders: readonly string[],
  preferredPath?: string,
  activePath?: string
): string | undefined {
  if (preferredPath) {
    return preferredPath;
  }
  if (folders.length === 0) {
    return undefined;
  }
  if (folders.length === 1) {
    return folders[0];
  }
  if (mode === "interactive") {
    return undefined;
  }
  if (activePath && folders.includes(activePath)) {
    return activePath;
  }
  return folders[0];
}

function programPicks(programs: ProgramTypeOption[]): Array<{
  label: string;
  description: string;
  program: ProgramTypeOption;
}> {
  return programs.map((program) => ({
    label: `PROGRAM ${program.name}`,
    description: relativePathLabel(program.uri),
    program,
  }));
}

async function pickProgramTypeWithMode(
  mode: SelectionMode,
  preferredFolder?: vscode.WorkspaceFolder
): Promise<ProgramTypeOption | undefined> {
  const preferred = preferredStructuredTextUri(preferredFolder);
  if (preferred) {
    const text = await readStructuredText(preferred);
    if (text) {
      const programs = collectProgramTypesFromSource(text, preferred);
      if (programs.length > 0) {
        if (!isInteractiveMode(mode)) {
          return programs[0];
        }
        if (programs.length === 1) {
          return programs[0];
        }
        const picked = await vscode.window.showQuickPick(programPicks(programs), {
          placeHolder: "Select the PROGRAM type to run.",
          ignoreFocusOut: true,
        });
        return picked?.program;
      }
    }
  }

  const programs = await collectProgramTypes(
    preferredFolder ? await findStructuredTextUris(preferredFolder) : undefined
  );
  if (programs.length === 0) {
    vscode.window.showErrorMessage(
      "No PROGRAM declarations found to create a configuration."
    );
    return undefined;
  }
  if (isInteractiveMode(mode)) {
    const picked = await vscode.window.showQuickPick(programPicks(programs), {
      placeHolder: "Select the PROGRAM type to run.",
      ignoreFocusOut: true,
    });
    return picked?.program;
  }
  programs.sort((a, b) => a.name.localeCompare(b.name));
  if (programs.length > 1) {
    debugChannel().appendLine(
      `Multiple PROGRAM types found; using ${programs[0].name}.`
    );
  }
  return programs[0];
}

async function pickWorkspaceFolderWithMode(
  preferred: vscode.WorkspaceFolder | undefined,
  mode: SelectionMode
): Promise<vscode.WorkspaceFolder | undefined> {
  const folders = vscode.workspace.workspaceFolders ?? [];
  if (preferred) {
    return preferred;
  }
  if (folders.length === 1) {
    return folders[0];
  }
  if (folders.length === 0) {
    return undefined;
  }
  if (isInteractiveMode(mode)) {
    const picked = await vscode.window.showQuickPick(
      folders.map((folder) => ({
        label: folder.name,
        description: folder.uri.fsPath,
        folder,
      })),
      {
        placeHolder: "Select a workspace folder for the configuration.",
        ignoreFocusOut: true,
      }
    );
    return picked?.folder;
  }
  const activeFolderPath = vscode.window.activeTextEditor
    ? vscode.workspace.getWorkspaceFolder(
        vscode.window.activeTextEditor.document.uri
      )?.uri.fsPath
    : undefined;
  const selectedPath = selectWorkspaceFolderPathForMode(
    mode,
    folders.map((folder) => folder.uri.fsPath),
    undefined,
    activeFolderPath
  );
  return folders.find((folder) => folder.uri.fsPath === selectedPath);
}

async function nextConfigurationUri(
  folder: vscode.WorkspaceFolder
): Promise<vscode.Uri> {
  const baseName = "configuration";
  for (let index = 0; index < 100; index += 1) {
    const suffix = index === 0 ? "" : `_${index + 1}`;
    const candidate = vscode.Uri.joinPath(
      folder.uri,
      `${baseName}${suffix}.st`
    );
    try {
      await vscode.workspace.fs.stat(candidate);
    } catch {
      return candidate;
    }
  }
  return vscode.Uri.joinPath(folder.uri, "configuration.st");
}

async function createDefaultConfigurationWithMode(
  program: ProgramTypeOption,
  mode: SelectionMode
): Promise<vscode.Uri | undefined> {
  const preferredFolder = vscode.workspace.getWorkspaceFolder(program.uri);
  const folder = await pickWorkspaceFolderWithMode(preferredFolder, mode);
  if (!folder) {
    vscode.window.showErrorMessage("No workspace folder available.");
    return undefined;
  }

  const configUri = await nextConfigurationUri(folder);
  const content = [
    "CONFIGURATION Conf",
    "  RESOURCE Res ON PLC",
    "    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);",
    `    PROGRAM P1 WITH MainTask : ${program.name};`,
    "  END_RESOURCE",
    "END_CONFIGURATION",
    "",
  ].join("\n");

  await vscode.workspace.fs.writeFile(
    configUri,
    Buffer.from(content, "utf8")
  );
  if (isInteractiveMode(mode)) {
    const doc = await vscode.workspace.openTextDocument(configUri);
    await vscode.window.showTextDocument(doc, { preview: false });
  } else {
    debugChannel().appendLine(
      `Created default configuration at ${configUri.fsPath}`
    );
  }
  return configUri;
}

function rememberConfiguration(uri: vscode.Uri | undefined): void {
  if (!uri || !workspaceState) {
    return;
  }
  void workspaceState.update(LAST_CONFIG_KEY, uri.toString());
}

function pickConfigurationFromState(
  configs: vscode.Uri[]
): vscode.Uri | undefined {
  const stored = workspaceState?.get<string>(LAST_CONFIG_KEY);
  if (!stored) {
    return undefined;
  }
  return configs.find((config) => config.toString() === stored);
}

function pickConfigurationFromActiveFolder(
  configs: vscode.Uri[],
  preferredFolder?: vscode.WorkspaceFolder
): vscode.Uri | undefined {
  const activeDocument = vscode.window.activeTextEditor?.document.uri;
  const activeConfiguration = activeDocument
    ? configs.find((config) => config.toString() === activeDocument.toString())
    : undefined;
  if (activeConfiguration) {
    return activeConfiguration;
  }
  const activeFolder =
    preferredFolder ??
    (vscode.window.activeTextEditor
      ? vscode.workspace.getWorkspaceFolder(
          vscode.window.activeTextEditor.document.uri
        )
      : undefined);
  if (!activeFolder) {
    return undefined;
  }
  const sameFolder = configs.filter(
    (config) =>
      vscode.workspace.getWorkspaceFolder(config)?.uri.fsPath ===
      activeFolder.uri.fsPath
  );
  if (sameFolder.length === 1) {
    return sameFolder[0];
  }
  return undefined;
}

async function ensureConfigurationEntryWithMode(
  mode: SelectionMode,
  preferredFolder?: vscode.WorkspaceFolder
): Promise<vscode.Uri | undefined> {
  const configs = await findConfigurationUris(preferredFolder);
  if (configs.length === 1) {
    rememberConfiguration(configs[0]);
    return configs[0];
  }
  if (configs.length > 1) {
    if (isInteractiveMode(mode)) {
      const picked = await vscode.window.showQuickPick(
        configs.map((config) => ({
          label: path.basename(config.fsPath),
          description: relativePathLabel(config),
          uri: config,
        })),
        {
          placeHolder: "Multiple CONFIGURATION files found. Select one to run.",
          ignoreFocusOut: true,
        }
      );
      if (picked?.uri) {
        rememberConfiguration(picked.uri);
      }
      return picked?.uri;
    }
    const fromActive = pickConfigurationFromActiveFolder(
      configs,
      preferredFolder
    );
    if (fromActive) {
      rememberConfiguration(fromActive);
      return fromActive;
    }
    const fromState = pickConfigurationFromState(configs);
    const picked =
      fromState ?? configs.sort((a, b) => a.fsPath.localeCompare(b.fsPath))[0];
    debugChannel().appendLine(
      `Multiple CONFIGURATION files found; using ${picked.fsPath}.`
    );
    rememberConfiguration(picked);
    return picked;
  }

  if (isInteractiveMode(mode)) {
    const create = await vscode.window.showInformationMessage(
      "No CONFIGURATION found. Create a default configuration?",
      "Create",
      "Cancel"
    );
    if (create !== "Create") {
      return undefined;
    }
  }

  const program = await pickProgramTypeWithMode(mode, preferredFolder);
  if (!program) {
    return undefined;
  }
  const created = await createDefaultConfigurationWithMode(program, mode);
  rememberConfiguration(created);
  return created;
}

export async function ensureConfigurationEntryAuto(
  preferredFolder?: vscode.WorkspaceFolder
): Promise<
  vscode.Uri | undefined
> {
  return ensureConfigurationEntryWithMode("auto", preferredFolder);
}

export async function ensureConfigurationEntry(): Promise<
  vscode.Uri | undefined
> {
  return ensureConfigurationEntryWithMode("interactive");
}

export async function __testEnsureConfigurationEntryAuto(): Promise<
  vscode.Uri | undefined
> {
  return ensureConfigurationEntryAuto();
}

export async function __testCreateDefaultConfigurationAuto(
  programName: string,
  programUri: vscode.Uri
): Promise<vscode.Uri | undefined> {
  return createDefaultConfigurationWithMode(
    { name: programName, uri: programUri },
    "auto"
  );
}

function extractProgramTypesFromConfiguration(source: string): string[] {
  const regex =
    /\bPROGRAM\s+[A-Za-z_][A-Za-z0-9_]*(?:\s+WITH\s+[A-Za-z_][A-Za-z0-9_]*)?\s*:\s*([A-Za-z_][A-Za-z0-9_\.]*)/gi;
  const types: string[] = [];
  let match: RegExpExecArray | null;
  while ((match = regex.exec(source)) !== null) {
    if (match[1]) {
      types.push(match[1]);
    }
  }
  return types;
}

export async function validateConfiguration(
  configUri: vscode.Uri
): Promise<boolean> {
  const text = await readStructuredText(configUri);
  if (!text) {
    vscode.window.showErrorMessage("Failed to read CONFIGURATION file.");
    return false;
  }
  const types = extractProgramTypesFromConfiguration(text);
  if (types.length === 0) {
    vscode.window.showErrorMessage(
      "CONFIGURATION has no PROGRAM entries. Add a PROGRAM binding."
    );
    return false;
  }
  const sourceUris = await collectRuntimeSourceUris(configUri);
  const programTypes = await collectProgramTypes(sourceUris);
  const available = new Set(
    programTypes.map((entry) => entry.name.toUpperCase())
  );
  const missing = types.filter(
    (typeName) => !available.has(typeName.toUpperCase())
  );
  if (missing.length > 0) {
    vscode.window.showErrorMessage(
      `Unknown PROGRAM type(s): ${missing.join(
        ", "
      )}. Check that the file defining them is in the workspace and included in runtime sources.`
    );
    return false;
  }
  return true;
}

export async function maybeReloadForEditor(
  editor: vscode.TextEditor | undefined,
  acceptedSession?: vscode.DebugSession,
): Promise<void> {
  if (!editor || editor.document.languageId !== "structured-text") {
    return;
  }
  const session = acceptedSession;
  if (!session || session.type !== DEBUG_TYPE) {
    return;
  }
  const config = session.configuration ?? {};
  const configuredProgram =
    typeof config.program === "string" && config.program.trim().length > 0
      ? config.program
      : undefined;
  const programUri = configuredProgram
    ? vscode.Uri.file(configuredProgram)
    : editor.document.uri;
  if (!(await isConfigurationFile(programUri))) {
    return;
  }
  const program = programUri.fsPath;
  const sessionId = session.id ?? session.name;
  if (lastReloadedProgram.get(sessionId) === program) {
    return;
  }
  try {
    const runtimeOptions = runtimeSourceOptions(programUri);
    await session.customRequest("stReload", { program, ...runtimeOptions });
    lastReloadedProgram.set(sessionId, program);
    debugChannel().appendLine(`Automatically updated running program: ${program}`);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    debugChannel().appendLine(`Automatic update failed: ${message}`);
  }
}

export function markSessionProgram(session: vscode.DebugSession): void {
  if (session.type !== DEBUG_TYPE) {
    return;
  }
  const program =
    typeof session.configuration?.program === "string"
      ? session.configuration.program
      : undefined;
  if (!program) {
    return;
  }
  const sessionId = session.id ?? session.name;
  lastReloadedProgram.set(sessionId, program);
}

export function clearSessionProgram(session: vscode.DebugSession): void {
  const sessionId = session.id ?? session.name;
  lastReloadedProgram.delete(sessionId);
}
