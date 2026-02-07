import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { getBinaryPath } from "./binary";

const DEBUG_TYPE = "structured-text";
const DEBUG_CHANNEL = "Structured Text Debugger";
const LAUNCH_WARN_DELAY_MS = 1500;
const ST_GLOB = "**/*.{st,ST,pou,POU}";
const ST_EXCLUDE_GLOB = "**/{node_modules,target,.git}/**";
const PRAGMA_SCAN_LINES = 20;
const LAST_CONFIG_KEY = "trust-lsp.lastConfigurationUri";

type RuntimeSourceOptions = {
  runtimeIncludeGlobs?: string[];
  runtimeExcludeGlobs?: string[];
  runtimeIgnorePragmas?: string[];
  runtimeRoot?: string;
};

type RuntimeControlConfig = {
  endpoint?: string;
  authToken?: string;
};

let output: vscode.OutputChannel | undefined;
let workspaceState: vscode.Memento | undefined;

function debugChannel(): vscode.OutputChannel {
  if (!output) {
    output = vscode.window.createOutputChannel(DEBUG_CHANNEL);
  }
  return output;
}

function captureStructuredTextEditor(editor: vscode.TextEditor | undefined): void {
  if (!editor) {
    return;
  }
  if (editor.document.languageId === "structured-text") {
    lastStructuredTextUri = editor.document.uri;
  }
}

function preferredStructuredTextUri(): vscode.Uri | undefined {
  const active = vscode.window.activeTextEditor;
  if (active && active.document.languageId === "structured-text") {
    return active.document.uri;
  }
  return lastStructuredTextUri;
}

function runtimeSourceOptions(target?: vscode.Uri): RuntimeSourceOptions {
  const config = vscode.workspace.getConfiguration("trust-lsp");
  const includeGlobs = normalizeStringArray(
    config.get<unknown>("runtime.includeGlobs")
  );
  const effectiveIncludeGlobs =
    includeGlobs.length > 0 ? includeGlobs : ["**/*.{st,ST,pou,POU}"];
  const excludeGlobs = normalizeStringArray(
    config.get<unknown>("runtime.excludeGlobs")
  );
  const ignorePragmas = normalizeStringArray(
    config.get<unknown>("runtime.ignorePragmas")
  );
  const folder = target
    ? vscode.workspace.getWorkspaceFolder(target)
    : vscode.workspace.workspaceFolders?.[0];
  const runtimeRoot = folder?.uri.fsPath;
  return {
    runtimeIncludeGlobs: effectiveIncludeGlobs,
    runtimeExcludeGlobs: excludeGlobs,
    runtimeIgnorePragmas: ignorePragmas,
    runtimeRoot,
  };
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

function loadRuntimeControlConfig(
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

function normalizeStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .map((item) => (typeof item === "string" ? item.trim() : ""))
    .filter((item) => item.length > 0);
}

async function findStructuredTextUris(): Promise<vscode.Uri[]> {
  const workspaceFolders = vscode.workspace.workspaceFolders;
  if (!workspaceFolders || workspaceFolders.length === 0) {
    return [];
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

async function findConfigurationUris(): Promise<vscode.Uri[]> {
  const uris = await findStructuredTextUris();
  const configs: vscode.Uri[] = [];
  for (const uri of uris) {
    const text = await readStructuredText(uri);
    if (text && containsConfiguration(text)) {
      configs.push(uri);
    }
  }
  return configs;
}

async function isConfigurationFile(uri: vscode.Uri): Promise<boolean> {
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
  mode: SelectionMode
): Promise<ProgramTypeOption | undefined> {
  const preferred = preferredStructuredTextUri();
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

  const programs = await collectProgramTypes();
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
  const active = preferredStructuredTextUri();
  const activeFolderPath = active
    ? vscode.workspace.getWorkspaceFolder(active)?.uri.fsPath
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
  configs: vscode.Uri[]
): vscode.Uri | undefined {
  const active = preferredStructuredTextUri();
  if (!active) {
    return undefined;
  }
  const activeFolder = vscode.workspace.getWorkspaceFolder(active);
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
  mode: SelectionMode
): Promise<vscode.Uri | undefined> {
  const configs = await findConfigurationUris();
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
    const fromState = pickConfigurationFromState(configs);
    if (fromState) {
      return fromState;
    }
    const fromActive = pickConfigurationFromActiveFolder(configs);
    const picked =
      fromActive ?? configs.sort((a, b) => a.fsPath.localeCompare(b.fsPath))[0];
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

  const program = await pickProgramTypeWithMode(mode);
  if (!program) {
    return undefined;
  }
  const created = await createDefaultConfigurationWithMode(program, mode);
  rememberConfiguration(created);
  return created;
}

async function ensureConfigurationEntryAuto(): Promise<vscode.Uri | undefined> {
  return ensureConfigurationEntryWithMode("auto");
}

async function ensureConfigurationEntry(): Promise<vscode.Uri | undefined> {
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

async function validateConfiguration(
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

async function maybeReloadForEditor(
  editor: vscode.TextEditor | undefined
): Promise<void> {
  if (!editor || editor.document.languageId !== "structured-text") {
    return;
  }
  const session = vscode.debug.activeDebugSession;
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
    debugChannel().appendLine(`Auto-reloaded program: ${program}`);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    debugChannel().appendLine(`Auto-reload failed: ${message}`);
  }
}

type LaunchFallbackState = {
  seenLaunch: boolean;
  fallbackTimer?: NodeJS.Timeout;
};

const launchFallbackState = new Map<string, LaunchFallbackState>();
let lastStructuredTextUri: vscode.Uri | undefined;
const lastReloadedProgram = new Map<string, string>();








function resolveAdapterCommand(
  config: vscode.WorkspaceConfiguration,
  context: vscode.ExtensionContext
): string {
  return getBinaryPath(context, "trust-debug", "debug.adapter.path");
}

async function ensureAdapterCommand(
  config: vscode.WorkspaceConfiguration,
  context: vscode.ExtensionContext
): Promise<string | undefined> {
  const command = resolveAdapterCommand(config, context);

  if (path.isAbsolute(command) && !fs.existsSync(command)) {
    void vscode.window.showErrorMessage(
      `Structured Text debug adapter not found at '${command}'. ` +
        `Install the extension from the Marketplace or set trust-lsp.debug.adapter.path.`,
      "Open Settings"
    ).then((choice) => {
      if (choice === "Open Settings") {
        void vscode.commands.executeCommand(
          "workbench.action.openSettings",
          "trust-lsp.debug.adapter.path"
        );
      }
    });
    return undefined;
  }

  return command;
}

function adapterEnv(
  config: vscode.WorkspaceConfiguration
): Record<string, string> {
  const overrides =
    config.get<Record<string, string>>("debug.adapter.env") ?? {};
  return {
    ...(process.env as Record<string, string>),
    ...overrides,
  };
}

class StructuredTextDebugAdapterFactory
  implements vscode.DebugAdapterDescriptorFactory, vscode.Disposable
{
  constructor(private readonly context: vscode.ExtensionContext) {}

  dispose(): void {
    // No resources to dispose yet.
  }

  createDebugAdapterDescriptor(
    _session: vscode.DebugSession
  ): vscode.ProviderResult<vscode.DebugAdapterDescriptor> {
    const config = vscode.workspace.getConfiguration("trust-lsp");
    debugChannel().appendLine("createDebugAdapterDescriptor called");
    return ensureAdapterCommand(config, this.context).then((command) => {
      if (!command) {
        debugChannel().appendLine(
          "No debug adapter command resolved; aborting session."
        );
        return undefined;
      }
      debugChannel().appendLine(`Launching adapter: ${command}`);
      const args = config.get<string[]>("debug.adapter.args") ?? [];
      const options: vscode.DebugAdapterExecutableOptions = {
        env: adapterEnv(config),
      };
      return new vscode.DebugAdapterExecutable(command, args, options);
    });
  }
}

class StructuredTextDebugConfigurationProvider
  implements vscode.DebugConfigurationProvider
{
  async resolveDebugConfiguration(
    folder: vscode.WorkspaceFolder | undefined,
    config: vscode.DebugConfiguration
  ): Promise<vscode.DebugConfiguration | null | undefined> {
    if (!config.type && !config.request && !config.name) {
      config.type = DEBUG_TYPE;
      config.request = "launch";
      config.name = "Debug Structured Text";
    }

    if (!config.type) {
      config.type = DEBUG_TYPE;
    }
    if (!config.request) {
      config.request = "launch";
    }
    if (!config.name) {
      config.name = "Debug Structured Text";
    }

    if (config.request === "attach") {
      if (!config.endpoint) {
        const controlConfig = loadRuntimeControlConfig(folder);
        if (!controlConfig?.endpoint) {
          vscode.window.showErrorMessage(
            "Attach requires runtime.control.endpoint in runtime.toml."
          );
          return null;
        }
        config.endpoint = controlConfig.endpoint;
        if (controlConfig.authToken && !config.authToken) {
          config.authToken = controlConfig.authToken;
        }
      }
      const runtimeOptions = runtimeSourceOptions();
      Object.assign(config, runtimeOptions);
    } else {
      if (!config.program) {
        const configUri = await ensureConfigurationEntryAuto();
        if (!configUri) {
          return null;
        }
        config.program = configUri.fsPath;
      } else {
        const programUri = vscode.Uri.file(config.program);
        if (!(await isConfigurationFile(programUri))) {
          const configUri = await ensureConfigurationEntryAuto();
          if (!configUri) {
            return null;
          }
          config.program = configUri.fsPath;
        }
      }
    }

    if (!config.cwd && folder) {
      config.cwd = folder.uri.fsPath;
    }

    debugChannel().appendLine(
      `Resolved debug config: type=${config.type} request=${config.request} program=${config.program ?? "<none>"} cwd=${config.cwd ?? "<none>"}`
    );

    return config;
  }

  resolveDebugConfigurationWithSubstitutedVariables(
    _folder: vscode.WorkspaceFolder | undefined,
    config: vscode.DebugConfiguration
  ): vscode.DebugConfiguration | null | undefined {
    debugChannel().appendLine(
      `Resolved debug config (substituted): type=${config.type} request=${config.request} program=${config.program ?? "<none>"} cwd=${config.cwd ?? "<none>"}`
    );
    return config;
  }
}

class StructuredTextDebugAdapterTrackerFactory
  implements vscode.DebugAdapterTrackerFactory
{
  createDebugAdapterTracker(
    session: vscode.DebugSession
  ): vscode.ProviderResult<vscode.DebugAdapterTracker> {
    if (session.type !== DEBUG_TYPE) {
      return undefined;
    }
    const interestingCommands = new Set([
      "initialize",
      "launch",
      "configurationDone",
      "setBreakpoints",
      "threads",
      "stackTrace",
      "scopes",
      "variables",
      "continue",
      "pause",
      "disconnect",
    ]);
    const interestingEvents = new Set([
      "initialized",
      "stopped",
      "continued",
      "terminated",
      "exited",
      "output",
    ]);
    const sessionId = session.id ?? session.name;
    const state: LaunchFallbackState = {
      seenLaunch: false,
    };
    launchFallbackState.set(sessionId, state);
    const channel = debugChannel();
    const formatMessage = (value: unknown): string => {
      try {
        return JSON.stringify(value);
      } catch (err) {
        return String(err);
      }
    };
    channel.appendLine(`Debug adapter tracker attached: ${session.name}`);
    return {
      onWillReceiveMessage: (message) => {
        channel.appendLine(`[DAP <-] ${formatMessage(message)}`);
        const command = (message as { command?: string }).command;
        if (command && interestingCommands.has(command)) {
          channel.appendLine(`[DAP <-] ${command}`);
        }
        if (command === "launch") {
          state.seenLaunch = true;
        }
      },
      onDidSendMessage: (message) => {
        channel.appendLine(`[DAP ->] ${formatMessage(message)}`);
        const event = (message as { event?: string }).event;
        if (event && interestingEvents.has(event)) {
          channel.appendLine(`[DAP ->] event ${event}`);
        }
        if (event === "initialized" && !state.fallbackTimer) {
          state.fallbackTimer = setTimeout(() => {
            const current = launchFallbackState.get(sessionId);
            if (!current || current.seenLaunch) {
              return;
            }
            channel.appendLine(
              "[DAP] launch not seen after initialized; waiting for VS Code"
            );
          }, LAUNCH_WARN_DELAY_MS);
        }
      },
      onError: (error) => {
        channel.appendLine(`[DAP] error: ${error}`);
      },
      onExit: (code, signal) => {
        channel.appendLine(
          `[DAP] exit: code=${code ?? "<none>"} signal=${signal ?? "<none>"}`
        );
        const current = launchFallbackState.get(sessionId);
        if (current?.fallbackTimer) {
          clearTimeout(current.fallbackTimer);
        }
        launchFallbackState.delete(sessionId);
      },
    };
  }
}

export function registerDebugAdapter(
  context: vscode.ExtensionContext
): void {
  workspaceState = context.workspaceState;
  captureStructuredTextEditor(vscode.window.activeTextEditor);
  const factory = new StructuredTextDebugAdapterFactory(context);
  context.subscriptions.push(
    vscode.debug.registerDebugAdapterDescriptorFactory(DEBUG_TYPE, factory)
  );
  context.subscriptions.push(factory);
  debugChannel().appendLine("Structured Text debug adapter factory registered.");

  const provider = new StructuredTextDebugConfigurationProvider();
  context.subscriptions.push(
    vscode.debug.registerDebugConfigurationProvider(DEBUG_TYPE, provider)
  );

  const trackerFactory = new StructuredTextDebugAdapterTrackerFactory();
  context.subscriptions.push(
    vscode.debug.registerDebugAdapterTrackerFactory(DEBUG_TYPE, trackerFactory)
  );

  const stringifySession = (session: vscode.DebugSession): string => {
    try {
      return JSON.stringify(session.configuration);
    } catch (err) {
      return String(err);
    }
  };
  context.subscriptions.push(
    vscode.debug.onDidStartDebugSession((session) => {
      debugChannel().appendLine(
        `Debug session started: ${session.name} type=${session.type} config=${stringifySession(session)}`
      );
      if (session.type === DEBUG_TYPE) {
        const program =
          typeof session.configuration?.program === "string"
            ? session.configuration.program
            : undefined;
        if (program) {
          const sessionId = session.id ?? session.name;
          lastReloadedProgram.set(sessionId, program);
        }
      }
    })
  );
  context.subscriptions.push(
    vscode.debug.onDidTerminateDebugSession((session) => {
      debugChannel().appendLine(
        `Debug session terminated: ${session.name} type=${session.type} config=${stringifySession(session)}`
      );
      if (session.type === DEBUG_TYPE) {
        const sessionId = session.id ?? session.name;
        const current = launchFallbackState.get(sessionId);
        if (current?.fallbackTimer) {
          clearTimeout(current.fallbackTimer);
        }
        launchFallbackState.delete(sessionId);
        lastReloadedProgram.delete(sessionId);
      }
    })
  );
  context.subscriptions.push(
    vscode.debug.onDidChangeActiveDebugSession((session) => {
      if (session) {
        debugChannel().appendLine(
          `Debug session active: ${session.name} type=${session.type}`
        );
      } else {
        debugChannel().appendLine("Debug session active: <none>");
      }
    })
  );

  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      captureStructuredTextEditor(editor);
      void maybeReloadForEditor(editor);
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "trust-lsp.debug.start",
      async (programOverride?: string | vscode.Uri) => {
        let programUri: vscode.Uri | undefined;
        let folder: vscode.WorkspaceFolder | undefined;

        if (typeof programOverride === "string" && programOverride.trim()) {
          programUri = vscode.Uri.file(programOverride);
        } else if (programOverride instanceof vscode.Uri) {
          programUri = programOverride;
        }

        if (programUri) {
          if (!(await isConfigurationFile(programUri))) {
            vscode.window.showErrorMessage(
              "Debugging requires a CONFIGURATION entry file."
            );
            return;
          }
        } else {
          programUri = await ensureConfigurationEntryAuto();
          if (!programUri) {
            return;
          }
        }

        folder = vscode.workspace.getWorkspaceFolder(programUri);
        if (!folder) {
          folder = vscode.workspace.workspaceFolders?.[0];
        }

        const diagnostics = vscode.languages.getDiagnostics(programUri);
        if (
          diagnostics.some(
            (diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Error
          )
        ) {
          vscode.window.showErrorMessage(
            "Configuration has errors. Fix them before starting a debug session."
          );
          return;
        }
        if (!(await validateConfiguration(programUri))) {
          return;
        }

        const program = programUri.fsPath;
        debugChannel().appendLine(`Start debugging command: program=${program}`);

        const runtimeOptions = runtimeSourceOptions(programUri);
        const config: vscode.DebugConfiguration = {
          type: DEBUG_TYPE,
          request: "launch",
          name: "Debug Structured Text",
          program,
          ...runtimeOptions,
        };

        if (folder) {
          config.cwd = folder.uri.fsPath;
        }

        const pendingTimer = setTimeout(() => {
          const active = vscode.debug.activeDebugSession;
          debugChannel().appendLine(
            `startDebugging still pending after 5s: active=${active?.name ?? "<none>"} type=${active?.type ?? "<none>"} config=${JSON.stringify(config)}`
          );
        }, 5000);
        void vscode.debug.startDebugging(folder, config).then(
          (started) => {
            clearTimeout(pendingTimer);
            debugChannel().appendLine(
              `startDebugging result: ${started} folder=${folder?.name ?? "<none>"} config=${JSON.stringify(config)}`
            );
          },
          (err: unknown) => {
            clearTimeout(pendingTimer);
            debugChannel().appendLine(
              `startDebugging error: ${err instanceof Error ? err.message : String(err)} folder=${folder?.name ?? "<none>"} config=${JSON.stringify(config)}`
            );
          }
        );
      }
    )
  );
  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.debug.attach", async () => {
      const folder = vscode.workspace.workspaceFolders?.[0];
      const controlConfig = loadRuntimeControlConfig(folder);
      if (!controlConfig?.endpoint) {
        vscode.window.showErrorMessage(
          "Attach requires runtime.control.endpoint in runtime.toml."
        );
        return;
      }
      const runtimeOptions = runtimeSourceOptions();
      const config: vscode.DebugConfiguration = {
        type: DEBUG_TYPE,
        request: "attach",
        name: "Attach Structured Text",
        endpoint: controlConfig.endpoint,
        authToken: controlConfig.authToken,
        ...runtimeOptions,
      };
      if (folder) {
        config.cwd = folder.uri.fsPath;
      }
      void vscode.debug.startDebugging(folder, config);
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "trust-lsp.debug.ensureConfiguration",
      async () => {
        await ensureConfigurationEntry();
      }
    )
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.debug.reload", async () => {
      const session = vscode.debug.activeDebugSession;
      if (!session || session.type !== DEBUG_TYPE) {
        vscode.window.showErrorMessage(
          "No active Structured Text debug session to reload."
        );
        return;
      }

      const config = session.configuration ?? {};
      const program =
        typeof config.program === "string" ? config.program : undefined;
      const preferred = preferredStructuredTextUri();
      const activeFile = preferred?.fsPath;

      try {
        const target =
          program && program.trim().length > 0
            ? vscode.Uri.file(program)
            : preferred;
        const runtimeOptions = runtimeSourceOptions(target);
        await session.customRequest("stReload", {
          program: program ?? activeFile,
          ...runtimeOptions,
        });
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        vscode.window.showErrorMessage(`Hot reload failed: ${message}`);
      }
    })
  );
}
