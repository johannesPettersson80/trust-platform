import { getTrustConfiguration } from "./configuration";
import * as vscode from "vscode";

import {
  isStructuralRuntimeLifecycleChange,
  runtimeLifecycleService,
  type RuntimeLifecycleSnapshot,
} from "./runtimeLifecycle";
import {
  listManagedRuntimes,
  onDidChangeManagedRuntimes,
} from "./localRuntime";
import {
  getSelectedRuntimeId,
  onDidChangeSelectedRuntime,
} from "./selectedRuntime";
import {
  remoteLabelFromEndpoint,
  runtimeOptions,
  selectedRuntime,
  SIMULATOR_RUNTIME_ID,
  type RemoteRuntime,
  type SelectedRuntime,
} from "./trustHomeModel";
import { workspaceHasReadableTrustProject } from "./workspaceProject";
import { LatestOnlyRevision } from "./latestOnlyRevision";
import {
  runtimeAuthoritySelection,
  runtimeModelSnapshotForLifecycle,
} from "./runtimeAuthoritySelection";

// §UX (reset 2026-06-22) — the status bar is PASSIVE. It shows the ACTIVE runtime's honest state and,
// on click, reveals the truST sidebar. It contributes NO Start/Stop command: there is exactly ONE
// run surface (the sidebar action row). The canvas is passive lifecycle status for the local
// Simulator. Never fabricate running/connected.

export function registerRuntimeControls(
  context: vscode.ExtensionContext,
): void {
  const item = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    100,
  );
  // Click only reveals the truST sidebar — it is not a run control.
  item.command = "trust.home.focus";
  item.tooltip = "Open the truST sidebar";
  context.subscriptions.push(item);
  const refreshRevision = new LatestOnlyRevision();

  async function refresh(): Promise<void> {
    const revision = refreshRevision.begin();
    const snapshot = await runtimeLifecycleService.snapshot();
    const text = await statusText(context, snapshot);
    if (!refreshRevision.isCurrent(revision)) {
      return;
    }
    item.text = text;
    item.show();
  }

  context.subscriptions.push(
    runtimeLifecycleService.onDidChange((change) => {
      if (isStructuralRuntimeLifecycleChange(change)) {
        void refresh();
      }
    }),
  );
  context.subscriptions.push(
    onDidChangeSelectedRuntime(() => {
      void refresh();
    }),
  );
  context.subscriptions.push(
    onDidChangeManagedRuntimes(() => {
      void refresh();
    }),
  );
  context.subscriptions.push(
    vscode.workspace.onDidChangeWorkspaceFolders(() => {
      void refresh();
    }),
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
      }),
    );
  }

  void refresh();
}

async function statusText(
  context: vscode.ExtensionContext,
  snapshot: RuntimeLifecycleSnapshot,
): Promise<string> {
  if (!(await workspaceHasReadableTrustProject())) {
    return "$(circle-outline) truST: No project";
  }
  const remotes = readRemotes();
  const managed = await listManagedRuntimes(context);
  const stored = getSelectedRuntimeId();
  const configuredSelectedId = runtimeOptions(remotes, managed).some(
    (option) => option.id === stored,
  )
    ? stored
    : SIMULATOR_RUNTIME_ID;
  const authority = runtimeAuthoritySelection(
    snapshot,
    remotes,
    managed,
    configuredSelectedId,
  );
  const selected = selectedRuntime({
    snapshot: runtimeModelSnapshotForLifecycle(snapshot, authority.target),
    remotes: authority.remotes,
    managed,
    selectedId: authority.selectedId,
    managedSessionId: authority.managedSessionId,
  });
  return selectedStatusText(selected);
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
      return selected.label;
    case "local":
      return selected.id;
    case "simulator":
    default:
      return "Simulator";
  }
}

function readRemotes(): RemoteRuntime[] {
  const endpoints =
    getTrustConfiguration(runtimeLifecycleService.runtimeConfigTarget()).get<
      string[]
    >("runtime.fleetEndpoints", []) ?? [];
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
