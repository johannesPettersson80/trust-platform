import * as path from "path";
import * as vscode from "vscode";
import {
  runtimeSourceOptionsForTarget,
  type RuntimeSourceOptions,
} from "../runtimeSourceOptions";
import {
  resolveProjectControlEndpoint,
  type ProjectControlEndpointResult,
} from "../projectControlEndpoint";
import { workspaceProjectDir } from "../runtimeConnection";
import { projectRequest } from "../projectRequest";

export const DEBUG_TYPE = "structured-text";
const DEBUG_CHANNEL = "Structured Text Debugger";
const LAST_CONFIG_KEY = "trust-lsp.lastConfigurationUri";

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

export function preferredStructuredTextUri(): vscode.Uri | undefined {
  const active = vscode.window.activeTextEditor;
  if (active && active.document.languageId === "structured-text") {
    return active.document.uri;
  }
  return lastStructuredTextUri;
}

export function runtimeSourceOptions(target?: vscode.Uri): RuntimeSourceOptions {
  return runtimeSourceOptionsForTarget(target);
}

// The endpoint and token of a project's runtime come from `trust-runtime ctl --project <dir> endpoint
// --json` (spec 11 §6.8.0a); the extension does not read runtime.toml for them.
export async function resolveRuntimeControlForFolder(
  bin: string,
  folder?: vscode.WorkspaceFolder
): Promise<ProjectControlEndpointResult> {
  const projectDir = await workspaceProjectDir(folder);
  if (!projectDir) {
    return {
      ok: false,
      code: "no_project",
      message: "Attach needs a project with runtime.toml in the workspace folder.",
    };
  }
  return resolveProjectControlEndpoint(bin, projectDir);
}

const RUN_ENTRY_COMMAND = "trust-lsp.runEntry";

// The simulator's entry is the language server's answer (`trust-lsp.runEntry`, specification 25 §11.6): which files declare
// a CONFIGURATION and which one a session starts from, the PROGRAM types and the workspace folder of a default CONFIGURATION,
// writing it, and whether a CONFIGURATION names only PROGRAM types the simulator's sources declare. This module asks, shows
// the pickers and messages, and remembers the last choice.

type ProgramTypeOption = {
  name: string;
  uri: vscode.Uri;
};

type ProgramAnswer = { name: string; path: string };

function workspaceFolderPaths(): string[] {
  return (vscode.workspace.workspaceFolders ?? []).map((folder) => folder.uri.fsPath);
}

function filePath(uri: vscode.Uri | undefined): string | undefined {
  return uri?.scheme === "file" ? uri.fsPath : undefined;
}

export async function isConfigurationFile(uri: vscode.Uri): Promise<boolean> {
  const path = filePath(uri);
  if (!path) {
    return false;
  }
  const answer = await projectRequest<{ configuration: boolean }>(RUN_ENTRY_COMMAND, {
    action: "declares",
    path,
  });
  return answer.ok && answer.configuration;
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

function programOption(program: ProgramAnswer): ProgramTypeOption {
  return { name: program.name, uri: vscode.Uri.file(program.path) };
}

async function pickProgramTypeWithMode(
  mode: SelectionMode
): Promise<ProgramTypeOption | undefined> {
  const answer = await projectRequest<{ programs: ProgramAnswer[]; chosen: ProgramAnswer | null; note: string | null }>(
    RUN_ENTRY_COMMAND,
    {
      action: "programs",
      active: filePath(preferredStructuredTextUri()),
      options: runtimeSourceOptions(),
      interactive: isInteractiveMode(mode),
    }
  );
  if (!answer.ok) {
    vscode.window.showErrorMessage(answer.error);
    return undefined;
  }
  if (answer.note) {
    debugChannel().appendLine(answer.note);
  }
  if (answer.chosen) {
    return programOption(answer.chosen);
  }
  const picked = await vscode.window.showQuickPick(programPicks(answer.programs.map(programOption)), {
    placeHolder: "Select the PROGRAM type to run.",
    ignoreFocusOut: true,
  });
  return picked?.program;
}

async function pickWorkspaceFolderWithMode(
  program: ProgramTypeOption,
  mode: SelectionMode
): Promise<vscode.WorkspaceFolder | undefined> {
  const folders = vscode.workspace.workspaceFolders ?? [];
  const answer = await projectRequest<{ folder: string | null; pick: boolean }>(RUN_ENTRY_COMMAND, {
    action: "folder",
    programPath: filePath(program.uri),
    active: filePath(preferredStructuredTextUri()),
    folders: workspaceFolderPaths(),
    interactive: isInteractiveMode(mode),
  });
  if (answer.ok && answer.pick) {
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
  return answer.ok && answer.folder
    ? folders.find((folder) => folder.uri.fsPath === answer.folder)
    : undefined;
}

async function createDefaultConfigurationWithMode(
  program: ProgramTypeOption,
  mode: SelectionMode
): Promise<vscode.Uri | undefined> {
  const folder = await pickWorkspaceFolderWithMode(program, mode);
  if (!folder) {
    vscode.window.showErrorMessage("No workspace folder available.");
    return undefined;
  }
  const answer = await projectRequest<{ path: string }>(RUN_ENTRY_COMMAND, {
    action: "create",
    folder: folder.uri.fsPath,
    program: program.name,
  });
  if (!answer.ok) {
    vscode.window.showErrorMessage(answer.error);
    return undefined;
  }
  const configUri = vscode.Uri.file(answer.path);
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

/** The CONFIGURATION file remembered from the last start, as a path. */
function rememberedConfigurationPath(): string | undefined {
  const stored = workspaceState?.get<string>(LAST_CONFIG_KEY);
  if (!stored) {
    return undefined;
  }
  try {
    return filePath(vscode.Uri.parse(stored));
  } catch {
    return undefined;
  }
}

async function ensureConfigurationEntryWithMode(
  mode: SelectionMode
): Promise<vscode.Uri | undefined> {
  const answer = await projectRequest<{ configurations: string[]; chosen: string | null; note: string | null }>(
    RUN_ENTRY_COMMAND,
    {
      action: "configurations",
      folders: workspaceFolderPaths(),
      remembered: rememberedConfigurationPath(),
      active: filePath(preferredStructuredTextUri()),
      interactive: isInteractiveMode(mode),
    }
  );
  if (!answer.ok) {
    vscode.window.showErrorMessage(answer.error);
    return undefined;
  }
  if (answer.chosen) {
    if (answer.note) {
      debugChannel().appendLine(answer.note);
    }
    const chosen = vscode.Uri.file(answer.chosen);
    rememberConfiguration(chosen);
    return chosen;
  }
  if (answer.configurations.length > 0) {
    const picked = await vscode.window.showQuickPick(
      answer.configurations.map((configPath) => {
        const uri = vscode.Uri.file(configPath);
        return { label: path.basename(configPath), description: relativePathLabel(uri), uri };
      }),
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

export async function ensureConfigurationEntryAuto(): Promise<
  vscode.Uri | undefined
> {
  return ensureConfigurationEntryWithMode("auto");
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

export async function validateConfiguration(
  configUri: vscode.Uri
): Promise<boolean> {
  const answer = await projectRequest<{ problem: string | null }>(RUN_ENTRY_COMMAND, {
    action: "check",
    program: configUri.fsPath,
    options: runtimeSourceOptions(configUri),
  });
  if (!answer.ok) {
    vscode.window.showErrorMessage(answer.error);
    return false;
  }
  if (answer.problem) {
    vscode.window.showErrorMessage(answer.problem);
    return false;
  }
  return true;
}

export async function maybeReloadForEditor(
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
