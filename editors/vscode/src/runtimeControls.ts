import * as vscode from "vscode";

import {
  runtimeLifecycleService,
  type RuntimeLifecycleSnapshot,
} from "./runtimeLifecycle";
import { listManagedRuntimes, onDidChangeManagedRuntimes } from "./localRuntime";
import {
  getSelectedRuntimeId,
  onDidChangeSelectedRuntime,
} from "./selectedRuntime";
import {
  remoteLabelFromEndpoint,
  selectedRuntime,
  type RemoteRuntime,
  type RuntimeModelSnapshot,
  type SelectedRuntime,
} from "./trustHomeModel";

// §UX (reset 2026-06-22) — the status bar is PASSIVE. It shows the ACTIVE runtime's honest state and,
// on click, reveals the truST sidebar. It contributes NO Start/Stop command: there is exactly ONE
// run surface (the sidebar action row / canvas runtime node). Never fabricate running/connected.

export function registerRuntimeControls(context: vscode.ExtensionContext): void {
  const item = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    100
  );
  // Click only reveals the truST sidebar — it is not a run control.
  item.command = "trust.home.focus";
  item.tooltip = "Open the truST sidebar";
  context.subscriptions.push(item);

  async function refresh(): Promise<void> {
    const snapshot = await runtimeLifecycleService.snapshot();
    item.text = await statusText(context, snapshot);
    item.show();
  }

  context.subscriptions.push(
    runtimeLifecycleService.onDidChange(() => {
      void refresh();
    })
  );
  context.subscriptions.push(
    onDidChangeSelectedRuntime(() => {
      void refresh();
    })
  );
  context.subscriptions.push(
    onDidChangeManagedRuntimes(() => {
      void refresh();
    })
  );
  context.subscriptions.push(
    vscode.workspace.onDidChangeWorkspaceFolders(() => {
      void refresh();
    })
  );
  for (const pattern of ["**/trust-lsp.toml", "**/runtime.toml"]) {
    const watcher = vscode.workspace.createFileSystemWatcher(pattern);
    context.subscriptions.push(
      watcher,
      watcher.onDidCreate(() => {
        void refresh();
      }),
      watcher.onDidChange(() => {
        void refresh();
      }),
      watcher.onDidDelete(() => {
        void refresh();
      })
    );
  }

  void refresh();
}

async function statusText(
  context: vscode.ExtensionContext,
  snapshot: RuntimeLifecycleSnapshot
): Promise<string> {
  if (!(await workspaceHasTrustProject())) {
    return "$(circle-outline) truST: No project";
  }
  if (snapshot.status.runtimeState === "connected") {
    const label =
      snapshot.status.targetLabel?.trim() ||
      connectedEndpointLabel(snapshot.status.endpoint);
    return `$(plug) truST: ${label} connected`;
  }
  const remotes = readRemotes();
  const managed = await listManagedRuntimes(context);
  const selected = selectedRuntime({
    snapshot: toModelSnapshot(snapshot),
    remotes,
    managed,
    selectedId: getSelectedRuntimeId(),
  });
  return selectedStatusText(selected);
}

function connectedEndpointLabel(endpoint: string): string {
  const trimmed = endpoint.trim();
  if (!trimmed) {
    return "runtime";
  }
  if (/^unix:\/\//i.test(trimmed)) {
    return "runtime";
  }
  return remoteLabelFromEndpoint(trimmed);
}

async function workspaceHasTrustProject(): Promise<boolean> {
  if (!vscode.workspace.workspaceFolders?.length) {
    return false;
  }
  const found = await vscode.workspace.findFiles(
    "**/trust-lsp.toml",
    "**/node_modules/**",
    1
  );
  return found.length > 0;
}

function selectedStatusText(selected: SelectedRuntime): string {
  if (selected.status === "starting") {
    return `$(sync~spin) truST: ${statusTargetLabel(selected)} starting…`;
  }
  if (selected.status === "running") {
    return `$(circle-filled) truST: ${statusTargetLabel(selected)} running`;
  }
  if (selected.status === "connected") {
    return `$(plug) truST: ${statusTargetLabel(selected)} connected`;
  }
  if (selected.status === "unreachable") {
    return `$(warning) truST: ${statusTargetLabel(selected)} not reachable`;
  }
  if (selected.status === "disconnected") {
    return `$(circle-outline) truST: ${statusTargetLabel(selected)} not connected`;
  }
  return `$(circle-outline) truST: ${statusTargetLabel(selected)} stopped`;
}

function statusTargetLabel(selected: SelectedRuntime): string {
  switch (selected.kind) {
    case "remote":
      return remoteLabelFromEndpoint(selected.id);
    case "local":
      return selected.id;
    case "simulator":
    default:
      return "Simulator";
  }
}

function readRemotes(): RemoteRuntime[] {
  const endpoints =
    vscode.workspace
      .getConfiguration("trust-lsp")
      .get<string[]>("runtime.fleetEndpoints", []) ?? [];
  const seen = new Set<string>();
  const remotes: RemoteRuntime[] = [];
  for (const raw of endpoints) {
    const endpoint = (raw ?? "").trim();
    if (!endpoint || seen.has(endpoint)) {
      continue;
    }
    seen.add(endpoint);
    remotes.push({ id: endpoint, label: remoteLabelFromEndpoint(endpoint) });
  }
  return remotes;
}

function toModelSnapshot(snapshot: RuntimeLifecycleSnapshot): RuntimeModelSnapshot {
  return {
    runtimeMode: snapshot.status.runtimeMode,
    runtimeState: snapshot.status.runtimeState,
    endpoint: snapshot.status.endpoint,
    endpointConfigured: snapshot.status.endpointConfigured,
    endpointReachable: snapshot.status.endpointReachable,
    starting: snapshot.starting,
  };
}
