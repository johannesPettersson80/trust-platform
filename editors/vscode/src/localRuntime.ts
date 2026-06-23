import { execFile } from "child_process";
import * as vscode from "vscode";

import { getBinaryPath } from "./binary";
import {
  toManagedRuntimes,
  type FleetListResponse,
  type FleetRuntimeStatusResponse,
  type ManagedRuntime,
} from "./localRuntimeModel";

// §0.6.1 / Phase 9 — managed local runtimes: fleet.toml runtime projects on THIS computer whose process
// the extension owns. Shells `trust-runtime fleet …` (offline; no running runtime required to list/stop).
// Every call returns empty/false on ANY failure (no fleet.toml, old binary, bad JSON) so the UI degrades
// honestly instead of breaking.

const changeEmitter = new vscode.EventEmitter<void>();
export const onDidChangeManagedRuntimes = changeEmitter.event;

let logChannel: vscode.OutputChannel | undefined;

export function registerLocalRuntime(context: vscode.ExtensionContext): void {
  context.subscriptions.push(changeEmitter);
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
  const bin = binary(context);
  const list = await runJson<FleetListResponse>(
    bin,
    ["fleet", "list", "--fleet-root", root, "--json"],
    root
  );
  if (!list?.runtimes?.length) {
    return [];
  }
  const statuses = new Map<string, FleetRuntimeStatusResponse>();
  await Promise.all(
    list.runtimes.map(async (entry) => {
      if (!entry?.name) {
        return;
      }
      const status = await runJson<FleetRuntimeStatusResponse>(
        bin,
        ["fleet", "runtime", "status", "--fleet-root", root, "--name", entry.name, "--json"],
        root
      );
      if (status) {
        statuses.set(entry.name, status);
      }
    })
  );
  return toManagedRuntimes(list, statuses);
}

async function lifecycle(
  context: vscode.ExtensionContext,
  action: "start" | "stop",
  name: string
): Promise<boolean> {
  const root = await fleetRoot();
  if (!root) {
    return false;
  }
  const result = await runJson<FleetRuntimeStatusResponse>(
    binary(context),
    ["fleet", "runtime", action, "--fleet-root", root, "--name", name, "--json"],
    root
  );
  changeEmitter.fire();
  return !!result;
}

export function startManagedRuntime(
  context: vscode.ExtensionContext,
  name: string
): Promise<boolean> {
  return lifecycle(context, "start", name);
}

export function stopManagedRuntime(
  context: vscode.ExtensionContext,
  name: string
): Promise<boolean> {
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
      logChannel?.append(stdout || stderr || `No logs available for ${name}.\n`);
    }
  );
}
