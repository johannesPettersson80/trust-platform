import * as vscode from "vscode";
import { prepareStarterProjectHandoff } from "./projectLanding";
import { openPendingProjectLanding } from "./projectLandingActivation";
import { openCreatedProjectFolder } from "./projectOpening";
export { FOCUS_MAIN_KEY, prepareStarterProjectHandoff } from "./projectLanding";

type SimulatedCancelAt = "folder" | "name" | "overwrite";

type NewProjectArgs = {
  targetUri?: vscode.Uri | string;
  baseUri?: vscode.Uri | string;
  projectName?: string;
  overwrite?: boolean;
  openWorkspace?: boolean;
  simulateCancelAt?: SimulatedCancelAt;
};

export const NEW_PROJECT_COMMAND = "trust-lsp.newProject";

const MAIN_ST_SOURCE = `PROGRAM Main
END_PROGRAM
`;

// A CONFIGURATION instantiates the program (RESOURCE + TASK + PROGRAM WITH) so the project actually
// runs AND so a brand-new project is clean — without an instance, the "unused program" lint (W009)
// flags Main on first open (F-02). The instance name must differ from the Main POU name because both
// declarations are imported into the project symbol table. INTERVAL matches runtime.toml.
const CONFIG_ST_SOURCE = `CONFIGURATION Config
RESOURCE MainRes ON PLC
    TASK MainTask (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM MainInstance WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
`;

const PROJECT_TOML_SOURCE = `include_paths = ["src"]
`;

const LAUNCH_JSON_SOURCE = `{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "structured-text",
      "request": "launch",
      "name": "truST Simulator",
      "program": "\${workspaceFolder}/src/config.st",
      "stopOnEntry": false
    }
  ]
}
`;

const VSCODE_SETTINGS_SOURCE = `{
  "debug.showInStatusBar": "never"
}
`;

// §0.5.15: a runnable simulator project = src/Main.st + trust-lsp.toml + runtime.toml + io.toml. The
// simulator (trust-debug) runs from trust-lsp.toml; runtime.toml + io.toml let Devices & Connections load
// the OFFLINE topology immediately (read by the bundled trust-runtime — phase 0 packaging) with simulated
// I/O, so the user never hand-edits TOML to get a running, configurable project.
const RUNTIME_CONTROL_ENDPOINT =
  process.platform === "win32"
    ? "tcp://127.0.0.1:9902"
    : "unix:///tmp/trust-runtime.sock";

// Full section set required by the runtime config parser (crates/trust-runtime/src/config/parser.rs) —
// retain/watchdog/fault are NOT optional. Mirrors the proven examples/network_canvas_demo/runtime.toml
// so the project loads offline (Devices & Connections topology) and `trust-runtime comm topology` passes.
const RUNTIME_TOML_SOURCE = `[bundle]
version = 1

[resource]
name = "MainRes"
cycle_interval_ms = 10

[runtime.control]
endpoint = "${RUNTIME_CONTROL_ENDPOINT}"
mode = "production"
debug_enabled = false

[runtime.web]
enabled = true
listen = "127.0.0.1:8080"
auth = "local"
tls = false

[runtime.tls]
mode = "disabled"
require_remote = false

[runtime.discovery]
enabled = false
service_name = "truST"
advertise = false
interfaces = []

[runtime.mesh]
enabled = false
listen = "0.0.0.0:5200"
tls = false
publish = []

[runtime.observability]
enabled = false
sample_interval_ms = 1000
mode = "all"
include = []
history_path = "history/historian.jsonl"
max_entries = 20000
prometheus_enabled = true
prometheus_path = "/metrics"

[runtime.log]
level = "info"

[runtime.retain]
mode = "file"
path = "retain.bin"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 1000
action = "halt"

[runtime.fault]
policy = "halt"
`;

const IO_TOML_SOURCE = `# Simulated I/O so the project runs with no hardware or brokers, on any machine. Devices & Connections
# ("Set up runtime…" → add a device) writes real drivers here when you wire one up.
[io]
driver = "simulated"
params = {}
`;

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

async function pathExists(uri: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(uri);
    return true;
  } catch {
    return false;
  }
}

async function isDirectory(uri: vscode.Uri): Promise<boolean> {
  try {
    const stat = await vscode.workspace.fs.stat(uri);
    return (stat.type & vscode.FileType.Directory) !== 0;
  } catch {
    return false;
  }
}

function validateProjectName(value: string): string | undefined {
  const trimmed = value.trim();
  if (!trimmed) {
    return "Project name is required.";
  }
  if (trimmed.includes("/") || trimmed.includes("\\")) {
    return "Project name must not contain path separators.";
  }
  if (trimmed === "." || trimmed === "..") {
    return "Project name is invalid.";
  }
  return undefined;
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

async function promptForProjectName(): Promise<string | undefined> {
  // VS Code checks the box as soon as it opens, so an empty box would say "required" before anything is typed; a selected
  // suggestion opens without a message and is replaced by the first key.
  return vscode.window.showInputBox({
    prompt: "Enter a name for the new Structured Text project",
    placeHolder: "my-st-project",
    value: "my-st-project",
    valueSelection: [0, "my-st-project".length],
    validateInput: validateProjectName,
  });
}

/** The files a new project writes, relative to its folder. */
const SCAFFOLD_FILES = [
  "src/Main.st",
  "src/config.st",
  "trust-lsp.toml",
  "runtime.toml",
  "io.toml",
  ".vscode/launch.json",
  ".vscode/settings.json",
];

/** The files a new project would replace in `targetUri`. */
async function existingScaffoldFiles(targetUri: vscode.Uri): Promise<string[]> {
  const found: string[] = [];
  for (const file of SCAFFOLD_FILES) {
    if (await pathExists(vscode.Uri.joinPath(targetUri, ...file.split("/")))) found.push(file);
  }
  return found;
}

async function confirmOverwrite(existing: string[]): Promise<boolean> {
  const selection = await vscode.window.showWarningMessage(
    `This folder already has ${existing.join(", ")}. Replace them with a new project's files? Your other files stay.`,
    { modal: true },
    "Replace",
    "Cancel"
  );
  return selection === "Replace";
}

async function writeScaffold(targetUri: vscode.Uri): Promise<void> {
  const srcUri = vscode.Uri.joinPath(targetUri, "src");
  const vscodeUri = vscode.Uri.joinPath(targetUri, ".vscode");
  await vscode.workspace.fs.createDirectory(srcUri);
  await vscode.workspace.fs.createDirectory(vscodeUri);
  const mainBuffer = Buffer.from(MAIN_ST_SOURCE);
  await vscode.workspace.fs.writeFile(
    vscode.Uri.joinPath(srcUri, "Main.st"),
    mainBuffer
  );
  await vscode.workspace.fs.writeFile(
    vscode.Uri.joinPath(srcUri, "config.st"),
    Buffer.from(CONFIG_ST_SOURCE)
  );
  await vscode.workspace.fs.writeFile(
    vscode.Uri.joinPath(targetUri, "trust-lsp.toml"),
    Buffer.from(PROJECT_TOML_SOURCE)
  );
  await vscode.workspace.fs.writeFile(
    vscode.Uri.joinPath(targetUri, "runtime.toml"),
    Buffer.from(RUNTIME_TOML_SOURCE)
  );
  await vscode.workspace.fs.writeFile(
    vscode.Uri.joinPath(targetUri, "io.toml"),
    Buffer.from(IO_TOML_SOURCE)
  );
  await vscode.workspace.fs.writeFile(
    vscode.Uri.joinPath(vscodeUri, "launch.json"),
    Buffer.from(LAUNCH_JSON_SOURCE)
  );
  await vscode.workspace.fs.writeFile(
    vscode.Uri.joinPath(vscodeUri, "settings.json"),
    Buffer.from(VSCODE_SETTINGS_SOURCE)
  );
}

// Focus the freshly-scaffolded Main.st. Called both inline (no reload) and on the next activation (after
// vscode.openFolder reloads the window). No-op when the pending file isn't in the current workspace.
export async function focusPendingMain(
  context: vscode.ExtensionContext
): Promise<void> {
  await openPendingProjectLanding(context);
}

async function resolveTargetUri(
  args?: NewProjectArgs
): Promise<vscode.Uri | undefined> {
  const directTarget = asUri(args?.targetUri);
  if (directTarget) {
    return directTarget;
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
  const rawName = args?.projectName ?? (await promptForProjectName());
  if (!rawName) {
    return undefined;
  }
  const trimmedName = rawName.trim();
  const validation = validateProjectName(trimmedName);
  if (validation) {
    vscode.window.showErrorMessage(validation);
    return undefined;
  }
  return vscode.Uri.joinPath(baseUri, trimmedName);
}

export function registerNewProjectCommand(
  context: vscode.ExtensionContext
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(
      NEW_PROJECT_COMMAND,
      async (args?: NewProjectArgs) => {
        const targetUri = await resolveTargetUri(args);
        if (!targetUri) {
          return false;
        }

        const exists = await pathExists(targetUri);
        if (exists) {
          if (!(await isDirectory(targetUri))) {
            vscode.window.showErrorMessage(
              `Target path exists and is not a directory: ${targetUri.fsPath}`
            );
            return false;
          }
          // Only files a new project writes are replaced, so only they are asked about: an empty
          // folder, or one with other files only, becomes a project at once.
          const existing = await existingScaffoldFiles(targetUri);
          if (existing.length > 0) {
            if (args?.simulateCancelAt === "overwrite") {
              return false;
            }
            const overwrite =
              args?.overwrite ?? (await confirmOverwrite(existing));
            if (!overwrite) {
              return false;
            }
          }
        }

        await writeScaffold(targetUri);

        const mainUri = vscode.Uri.joinPath(targetUri, "src", "Main.st");
        await prepareStarterProjectHandoff(context, mainUri.fsPath);

        const openWorkspace = args?.openWorkspace ?? true;
        if (openWorkspace) {
          // Opening the folder reloads the window; focusPendingMain (next activation) focuses Main.st.
          if (!(await openCreatedProjectFolder(targetUri))) return false;
        } else {
          // No reload — focus Main.st right now.
          await focusPendingMain(context);
        }

        vscode.window.showInformationMessage("truST project created.");
        return true;
      }
    )
  );
}
