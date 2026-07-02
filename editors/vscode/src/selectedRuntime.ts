import * as vscode from "vscode";
import { createHash } from "crypto";
import * as fs from "fs";
import * as path from "path";

import { SIMULATOR_RUNTIME_ID } from "./trustHomeModel";

// §0.5.11 — the ONE selected-run-target source of truth. The Run bar dropdown, the graph node's "Set as
// run target", AND "Connect" all read/write this single store, so connecting (or selecting) a runtime
// anywhere is reflected everywhere. No second copy.

const KEY = "trust.home.selectedRuntime";
const GLOBAL_KEY_PREFIX = "trust.home.selectedRuntime.workspace.";
const PERSIST_FILE = "selected-runtime-by-workspace.json";

let ctx: vscode.ExtensionContext | undefined;
const emitter = new vscode.EventEmitter<void>();
let persistedTargets: Record<string, string> | undefined;

/** Fires whenever the selected run target changes (from the Run bar OR a graph node). */
export const onDidChangeSelectedRuntime = emitter.event;

export function initSelectedRuntimeStore(context: vscode.ExtensionContext): void {
  ctx = context;
  context.subscriptions.push(emitter);
}

export function getSelectedRuntimeId(): string {
  const globalKey = workspaceScopedGlobalKey();
  return (
    ctx?.workspaceState.get<string>(KEY) ??
    ctx?.globalState.get<string>(globalKey) ??
    readPersistedTargets()[globalKey] ??
    SIMULATOR_RUNTIME_ID
  );
}

export async function setSelectedRuntimeId(id: string): Promise<void> {
  if (!ctx || !id) {
    return;
  }

  const globalKey = workspaceScopedGlobalKey();
  const workspaceValue = ctx.workspaceState.get<string>(KEY);
  const globalValue = ctx.globalState.get<string>(globalKey);
  const persistedValue = readPersistedTargets()[globalKey];
  if (workspaceValue === id && globalValue === id && persistedValue === id) {
    return;
  }

  await ctx.workspaceState.update(KEY, id);
  await ctx.globalState.update(globalKey, id);
  writePersistedTarget(globalKey, id);
  emitter.fire();
}

function workspaceScopedGlobalKey(): string {
  const roots =
    vscode.workspace.workspaceFolders
      ?.map((folder) => folder.uri.toString())
      .sort()
      .join("|") || "no-workspace";
  const digest = createHash("sha1").update(roots).digest("hex").slice(0, 16);
  return `${GLOBAL_KEY_PREFIX}${digest}`;
}

function readPersistedTargets(): Record<string, string> {
  if (!ctx) {
    return {};
  }
  if (persistedTargets) {
    return persistedTargets;
  }
  const filePath = persistedTargetsPath();
  try {
    const raw = fs.readFileSync(filePath, "utf8");
    const parsed = JSON.parse(raw);
    persistedTargets = isStringRecord(parsed) ? parsed : {};
  } catch {
    persistedTargets = {};
  }
  return persistedTargets;
}

function writePersistedTarget(globalKey: string, id: string): void {
  if (!ctx) {
    return;
  }
  const targets = { ...readPersistedTargets(), [globalKey]: id };
  persistedTargets = targets;
  const filePath = persistedTargetsPath();
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${JSON.stringify(targets, null, 2)}\n`, "utf8");
}

function persistedTargetsPath(): string {
  if (!ctx) {
    return "";
  }
  return path.join(ctx.globalStorageUri.fsPath, PERSIST_FILE);
}

function isStringRecord(value: unknown): value is Record<string, string> {
  if (!value || typeof value !== "object") {
    return false;
  }
  return Object.values(value).every((entry) => typeof entry === "string");
}
