import { execFile } from "child_process";
import * as vscode from "vscode";

import { getBinaryPath } from "./binary";
import type { ManagedLifecycleResult, ManagedRuntime } from "./localRuntimeModel";
import { projectRequest } from "./projectRequest";
import { resolveProjectControlEndpoint } from "./projectControlEndpoint";
import { setControlAuthToken } from "./runtimeAuth";

// §0.6.1 / Phase 9 — managed local runtimes: fleet.toml runtime projects on THIS computer whose process
// the extension owns. The language server lists them with their states, says whether a Start or a Stop
// reached its state and makes their logs readable (`trust-lsp.runTargets`, actions `managed`,
// `managedOutcome` and `managedLogs`, specification 25 §5.1 and §6.5); Start, Stop and the logs run
// `trust-runtime fleet …`. Every call returns empty/false on ANY failure (no fleet.toml, old binary, no
// answer) so the UI degrades honestly instead of breaking.

const RUN_TARGETS_COMMAND = "trust-lsp.runTargets";

const changeEmitter = new vscode.EventEmitter<void>();
export const onDidChangeManagedRuntimes = changeEmitter.event;

let logChannel: vscode.OutputChannel | undefined;
const importedRuntimeAuthTokens = new Map<string, string>();

export function registerLocalRuntime(context: vscode.ExtensionContext): void {
  context.subscriptions.push(changeEmitter);
  // Watch fleet.toml so a managed runtime added/removed OUTSIDE the extension — CLI
  // `fleet runtime add`, a hand-edit, or another tool — refreshes the Run-target dropdown and
  // the canvas's owned-runtime nodes, not only programmatic start/stop. Both surfaces already
  // listen on onDidChangeManagedRuntimes, so re-firing it is all that's needed.
  const fleetWatcher = vscode.workspace.createFileSystemWatcher("**/fleet.toml");
  fleetWatcher.onDidCreate(() => changeEmitter.fire());
  fleetWatcher.onDidChange(() => changeEmitter.fire());
  fleetWatcher.onDidDelete(() => changeEmitter.fire());
  context.subscriptions.push(fleetWatcher);
}

function binary(context: vscode.ExtensionContext): string {
  return getBinaryPath(context, "trust-runtime", "runtime.cli.path");
}

// The fleet root is the workspace folder that holds fleet.toml (where `fleet runtime add` registers).
async function fleetRoot(): Promise<string | undefined> {
  const found = await vscode.workspace.findFiles(
    "**/fleet.toml",
    "**/node_modules/**",
    1
  );
  return found.length > 0
    ? vscode.Uri.joinPath(found[0], "..").fsPath
    : undefined;
}

function runJson<T>(
  bin: string,
  args: string[],
  cwd: string
): Promise<T | undefined> {
  return new Promise((resolve) => {
    execFile(
      bin,
      args,
      { cwd, timeout: 15_000, maxBuffer: 16 * 1024 * 1024 },
      (_error, stdout) => {
        // start/stop/status print JSON to stdout even on non-zero exit; only spawn/parse failures → undefined.
        try {
          resolve(JSON.parse(stdout) as T);
        } catch {
          resolve(undefined);
        }
      }
    );
  });
}

export async function listManagedRuntimes(
  context: vscode.ExtensionContext
): Promise<ManagedRuntime[]> {
  const root = await fleetRoot();
  if (!root) {
    return [];
  }
  const answer = await projectRequest<{ runtimes: ManagedRuntime[] }>(RUN_TARGETS_COMMAND, {
    action: "managed",
    fleetRoot: root,
  });
  if (!answer.ok) {
    return [];
  }
  const managed = answer.runtimes.map(withoutNulls);
  await syncManagedRuntimeAuthTokens(binary(context), managed);
  return managed;
}

async function lifecycle(
  context: vscode.ExtensionContext,
  action: "start" | "stop",
  name: string
): Promise<ManagedLifecycleResult> {
  const root = await fleetRoot();
  if (!root) {
    return { ok: false, message: "No fleet runtime project found." };
  }
  const response = await runJson<unknown>(
    binary(context),
    ["fleet", "runtime", action, "--fleet-root", root, "--name", name, "--json"],
    root
  );
  changeEmitter.fire();
  const answer = await projectRequest<{ success: boolean } & Omit<ManagedLifecycleResult, "ok">>(RUN_TARGETS_COMMAND, {
    action: "managedOutcome",
    lifecycle: action,
    name,
    fleetRoot: root,
    response: response ?? null,
  });
  if (!answer.ok) {
    return { ok: false, message: answer.error };
  }
  const { success, ...outcome } = withoutNulls(answer);
  if (outcome.controlEndpoint && outcome.projectPath) {
    await importManagedRuntimeAuthToken(binary(context), outcome.projectPath);
  }
  return { ...outcome, ok: success };
}

export function startManagedRuntime(
  context: vscode.ExtensionContext,
  name: string
): Promise<ManagedLifecycleResult> {
  return lifecycle(context, "start", name);
}

export function stopManagedRuntime(
  context: vscode.ExtensionContext,
  name: string
): Promise<ManagedLifecycleResult> {
  return lifecycle(context, "stop", name);
}

export async function showManagedRuntimeLogs(
  context: vscode.ExtensionContext,
  name: string
): Promise<void> {
  const root = await fleetRoot();
  if (!root) {
    void vscode.window.showWarningMessage("No managed runtime to show logs for.");
    return;
  }
  if (!logChannel) {
    logChannel = vscode.window.createOutputChannel("truST Runtime");
    // Best-effort: tracked via the channel's own lifetime; VS Code disposes output channels on exit.
  }
  logChannel.clear();
  logChannel.show(true);
  execFile(
    binary(context),
    ["fleet", "runtime", "logs", "--fleet-root", root, "--name", name, "--lines", "200"],
    { cwd: root, timeout: 15_000, maxBuffer: 16 * 1024 * 1024 },
    (_error, stdout, stderr) => {
      void projectRequest<{ text: string }>(RUN_TARGETS_COMMAND, { action: "managedLogs", name, stdout, stderr }).then(
        (answer) => logChannel?.append(answer.ok ? answer.text : `${answer.error}\n`)
      );
    }
  );
}

// The token comes from `trust-runtime ctl --project <dir> endpoint --json` (spec 11 §6.8.0a), which
// reads the project's endpoint file and runtime.toml; the extension never parses them. The token is
// stored for the endpoint that the runtime reports, and imported again only when it changes.
async function importManagedRuntimeAuthToken(
  bin: string,
  projectPath: string
): Promise<void> {
  const resolved = await resolveProjectControlEndpoint(bin, projectPath);
  if (!resolved.ok) {
    return;
  }
  const endpoint = resolved.value.endpoint;
  const token = resolved.value.authToken;
  if (!token || importedRuntimeAuthTokens.get(endpoint) === token) {
    return;
  }
  await setControlAuthToken(endpoint, token);
  importedRuntimeAuthTokens.set(endpoint, token);
}

async function syncManagedRuntimeAuthTokens(
  bin: string,
  runtimes: ReadonlyArray<ManagedRuntime>
): Promise<void> {
  await Promise.all(
    runtimes.map(async (runtime) => {
      if (!runtime.controlEndpoint || !runtime.projectPath) {
        return;
      }
      await importManagedRuntimeAuthToken(bin, runtime.projectPath);
    })
  );
}

/** `value` without the members the language server sent as `null`, which the extension's types leave out. */
function withoutNulls<T extends object>(value: T): T {
  return Object.fromEntries(Object.entries(value).filter(([, member]) => member !== null)) as T;
}
