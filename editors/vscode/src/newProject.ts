import { execFile } from "child_process";
import * as path from "path";
import * as vscode from "vscode";

import { getBinaryPath } from "./binary";
import { prepareStarterProjectHandoff } from "./projectLanding";
import { openPendingProjectLanding } from "./projectLandingActivation";
import { openCreatedProjectFolder } from "./projectOpening";
export { FOCUS_MAIN_KEY, prepareStarterProjectHandoff } from "./projectLanding";

type SimulatedCancelAt = "start" | "folder" | "name" | "overwrite";

// The starts of Create Project (spec 25 §3.2, spec 48 RU-51).
export type ProjectStart = "structured-text" | "rust" | "rust-first";

type NewProjectArgs = {
  start?: ProjectStart;
  targetUri?: vscode.Uri | string;
  baseUri?: vscode.Uri | string;
  projectName?: string;
  overwrite?: boolean;
  openWorkspace?: boolean;
  simulateCancelAt?: SimulatedCancelAt;
};

// What the picker shows for each start, the flag of `trust-runtime new` that makes it, and the name the box
// suggests. Which files a start writes is the command's (spec 25 §3.2).
type StartChoice = vscode.QuickPickItem & {
  readonly start: ProjectStart;
  readonly flags: readonly string[];
  readonly suggestion: string;
  readonly kindOfProject: string;
};

export const START_CHOICES: readonly StartChoice[] = [
  {
    start: "structured-text",
    label: "Structured Text",
    detail: "A program in src/Main.st and the configuration that runs it.",
    flags: [],
    suggestion: "my-st-project",
    kindOfProject: "Structured Text project",
  },
  {
    start: "rust",
    label: "Structured Text with Rust",
    detail: "That project with a Rust crate in rust/: a function block that src/Main.st calls.",
    flags: ["--rust"],
    suggestion: "my-rust-project",
    kindOfProject: "Structured Text project with Rust",
  },
  {
    start: "rust-first",
    label: "Rust-first",
    detail: "The crate is the project: its Rust programs, and trust.toml with their tasks.",
    flags: ["--rust-first"],
    suggestion: "my-rust-cell",
    kindOfProject: "Rust-first project",
  },
];

function choiceOf(start: ProjectStart): StartChoice {
  return START_CHOICES.find((choice) => choice.start === start) ?? START_CHOICES[0];
}

// The parent folder, the name and the start that `trust-runtime new --parent <folder> --name <name>` takes.
type ProjectTarget = {
  readonly parent: string;
  readonly name: string;
  readonly start: ProjectStart;
};

// The answer of `trust-runtime new --json` (spec 25 §3.2): the project folder and the file to focus, or
// an error code and message. The command reports an error with a non-zero exit, so stdout is read either way.
type CreateAnswer =
  | { readonly ok: true; readonly project: string; readonly focus: string }
  | { readonly ok: false; readonly code: string; readonly message: string };

export const NEW_PROJECT_COMMAND = "trust-lsp.newProject";

// The Rust CLI owns the project: `trust-runtime new` writes every file, checks the name and reports an
// existing folder that is not empty, a target that is not a directory, or an I/O failure (spec 11 §6.8.0a,
// spec 25 §3.2). This module only prompts, runs the command, asks before it overwrites and opens the result.

function asUri(value?: vscode.Uri | string): vscode.Uri | undefined {
  if (!value) {
    return undefined;
  }
  if (value instanceof vscode.Uri) {
    return value;
  }
  try {
    if (value.includes("://")) {
      return vscode.Uri.parse(value);
    }
    return vscode.Uri.file(value);
  } catch {
    return undefined;
  }
}

async function promptForBaseFolder(): Promise<vscode.Uri | undefined> {
  const selected = await vscode.window.showOpenDialog({
    canSelectFiles: false,
    canSelectFolders: true,
    canSelectMany: false,
    openLabel: "Select Parent Folder",
  });
  return selected?.[0];
}

async function promptForStart(): Promise<ProjectStart | undefined> {
  const picked = await vscode.window.showQuickPick(START_CHOICES, {
    title: "Create Project",
    placeHolder: "What should the new project start as?",
  });
  return picked?.start;
}

async function promptForProjectName(start: ProjectStart): Promise<string | undefined> {
  // The box opens with the start's selected suggestion, which the first key replaces (acceptance finding F10).
  // The name is passed on as typed: `trust-runtime new` checks it and its refusal is shown.
  const choice = choiceOf(start);
  return vscode.window.showInputBox({
    prompt: `Enter a name for the new ${choice.kindOfProject}`,
    placeHolder: choice.suggestion,
    value: choice.suggestion,
    valueSelection: [0, choice.suggestion.length],
  });
}

async function confirmOverwrite(projectPath: string): Promise<boolean> {
  const selection = await vscode.window.showWarningMessage(
    `This folder is not empty: ${projectPath}\nReplace a new project's files in it? Your other files stay.`,
    { modal: true },
    "Replace",
    "Cancel"
  );
  return selection === "Replace";
}

function createArgs(target: ProjectTarget, force: boolean): string[] {
  return [
    "new",
    `--parent=${target.parent}`,
    `--name=${target.name}`,
    ...choiceOf(target.start).flags,
    ...(force ? ["--force"] : []),
    "--json",
  ];
}

// Runs `trust-runtime new` and reads the answer it prints. When stdout holds no answer, the command could
// not be started or failed before it reported, and its output is shown instead.
function runCreate(bin: string, args: string[]): Promise<CreateAnswer> {
  return new Promise((resolve) => {
    execFile(
      bin,
      args,
      { timeout: 60_000, maxBuffer: 4 * 1024 * 1024 },
      (error, stdout, stderr) => {
        const answer = parseCreateAnswer(String(stdout));
        if (answer) {
          resolve(answer);
          return;
        }
        const detail =
          String(stderr).trim() ||
          String(stdout).trim() ||
          error?.message ||
          "no answer from trust-runtime";
        resolve({
          ok: false,
          code: "unknown",
          message: `The project could not be created: ${detail}`,
        });
      }
    );
  });
}

// Undefined when stdout holds no answer of the documented shape.
function parseCreateAnswer(stdout: string): CreateAnswer | undefined {
  const parsed = parseJsonObject(stdout);
  if (!parsed) {
    return undefined;
  }
  if (isRecord(parsed.error)) {
    return {
      ok: false,
      code: stringField(parsed.error.code) ?? "unknown",
      message:
        stringField(parsed.error.message) ??
        "The project could not be created.",
    };
  }
  const project = stringField(parsed.project);
  const focus = stringField(parsed.focus);
  if (project === undefined || focus === undefined) {
    return undefined;
  }
  return { ok: true, project, focus };
}

function parseJsonObject(text: string): Record<string, unknown> | undefined {
  try {
    const value: unknown = JSON.parse(text);
    return isRecord(value) ? value : undefined;
  } catch {
    return undefined;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringField(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

// Focus the freshly-created project's file: src/Main.st, or the crate's source of a Rust start. Called both inline
// (no reload) and on the next activation (after vscode.openFolder reloads the window). No-op when the pending file
// isn't in the current workspace.
export async function focusPendingMain(
  context: vscode.ExtensionContext
): Promise<void> {
  await openPendingProjectLanding(context);
}

// The start first, since the name's suggestion is the start's; a caller that names the folder and no start
// makes the Structured Text start. A targetUri is split into its parent folder and its name. The name typed at
// the prompt is passed on as it was typed: the command checks it, so the extension neither trims nor validates it.
async function resolveTarget(
  args?: NewProjectArgs
): Promise<ProjectTarget | undefined> {
  const directTarget = asUri(args?.targetUri);
  if (args?.simulateCancelAt === "start") {
    return undefined;
  }
  const start =
    args?.start ?? (directTarget ? "structured-text" : await promptForStart());
  if (!start) {
    return undefined;
  }
  if (directTarget) {
    return {
      parent: path.dirname(directTarget.fsPath),
      name: path.basename(directTarget.fsPath),
      start,
    };
  }

  if (args?.simulateCancelAt === "folder") {
    return undefined;
  }
  const baseUri = asUri(args?.baseUri) ?? (await promptForBaseFolder());
  if (!baseUri) {
    return undefined;
  }

  if (args?.simulateCancelAt === "name") {
    return undefined;
  }
  const name = args?.projectName ?? (await promptForProjectName(start));
  if (name === undefined) {
    return undefined;
  }
  return { parent: baseUri.fsPath, name, start };
}

export function registerNewProjectCommand(
  context: vscode.ExtensionContext
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(
      NEW_PROJECT_COMMAND,
      async (args?: NewProjectArgs) => {
        const target = await resolveTarget(args);
        if (!target) {
          return false;
        }

        const bin = getBinaryPath(context, "trust-runtime", "runtime.cli.path");
        let answer = await runCreate(bin, createArgs(target, false));
        if (!answer.ok && answer.code === "exists") {
          if (args?.simulateCancelAt === "overwrite") {
            return false;
          }
          const overwrite =
            args?.overwrite ??
            (await confirmOverwrite(path.join(target.parent, target.name)));
          if (!overwrite) {
            return false;
          }
          answer = await runCreate(bin, createArgs(target, true));
        }
        if (!answer.ok) {
          vscode.window.showErrorMessage(answer.message);
          return false;
        }

        const projectDir = path.resolve(target.parent, answer.project);
        const focusPath = path.resolve(projectDir, answer.focus);
        // The command names the project: a Rust start's focus, rust/src/lib.rs, lies deeper in it than src/Main.st.
        await prepareStarterProjectHandoff(context, focusPath, projectDir);

        const openWorkspace = args?.openWorkspace ?? true;
        if (openWorkspace) {
          // Opening the folder reloads the window; focusPendingMain (next activation) focuses the project's file.
          if (!(await openCreatedProjectFolder(vscode.Uri.file(projectDir)))) return false;
        } else {
          // No reload: focus the project's file right now.
          await focusPendingMain(context);
        }

        vscode.window.showInformationMessage("truST project created.");
        return true;
      }
    )
  );
}
