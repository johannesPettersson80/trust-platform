import * as vscode from "vscode";

import {
  runtimeLifecycleService,
  type RuntimeLifecycleSnapshot,
} from "./runtimeLifecycle";
import { remoteLabelFromEndpoint } from "./trustHomeModel";

// §UX (reset 2026-06-22) — the status bar is PASSIVE. It shows the ACTIVE runtime's honest state and,
// on click, reveals the Run card. It contributes NO Start/Stop command: there is exactly ONE run
// surface (the Run card / canvas runtime node). Never fabricate running/connected.

export function registerRuntimeControls(context: vscode.ExtensionContext): void {
  const item = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    100
  );
  // Click only reveals the Run card — it is not a run control.
  item.command = "trust.home.focus";
  item.tooltip = "Open the truST Run panel";
  context.subscriptions.push(item);

  async function refresh(): Promise<void> {
    const snapshot = await runtimeLifecycleService.snapshot();
    item.text = statusText(snapshot);
    item.show();
  }

  context.subscriptions.push(
    runtimeLifecycleService.onDidChange(() => {
      void refresh();
    })
  );

  void refresh();
}

function statusText(snapshot: RuntimeLifecycleSnapshot): string {
  if (snapshot.starting) {
    return "$(sync~spin) truST: starting…";
  }
  const { runtimeMode, runtimeState, endpoint } = snapshot.status;
  if (runtimeMode === "online" && runtimeState === "connected") {
    return `$(plug) truST: ${remoteLabelFromEndpoint(endpoint)} connected`;
  }
  if (runtimeMode === "simulate" && runtimeState === "running") {
    return "$(circle-filled) truST: Simulator running";
  }
  return "$(circle-outline) truST: Simulator stopped";
}
