import * as vscode from "vscode";

import { SIMULATOR_RUNTIME_ID } from "./trustHomeModel";

// §0.5.11 — the ONE selected-run-target source of truth. The Run bar dropdown, the graph node's "Set as
// run target", AND "Connect" all read/write this single store, so connecting (or selecting) a runtime
// anywhere is reflected everywhere. No second copy.

const KEY = "trust.home.selectedRuntime";

let ctx: vscode.ExtensionContext | undefined;
const emitter = new vscode.EventEmitter<void>();

/** Fires whenever the selected run target changes (from the Run bar OR a graph node). */
export const onDidChangeSelectedRuntime = emitter.event;

export function initSelectedRuntimeStore(context: vscode.ExtensionContext): void {
  ctx = context;
  context.subscriptions.push(emitter);
}

export function getSelectedRuntimeId(): string {
  return (
    ctx?.workspaceState.get<string>(KEY, SIMULATOR_RUNTIME_ID) ??
    SIMULATOR_RUNTIME_ID
  );
}

export async function setSelectedRuntimeId(id: string): Promise<void> {
  if (!ctx || !id) {
    return;
  }
  if (getSelectedRuntimeId() === id) {
    return;
  }
  await ctx.workspaceState.update(KEY, id);
  emitter.fire();
}
