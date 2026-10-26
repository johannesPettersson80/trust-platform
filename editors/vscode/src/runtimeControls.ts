import { getTrustConfiguration } from "./configuration";
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
  SIMULATOR_RUNTIME_ID,
  type RemoteRuntime,
  type RuntimeModelSnapshot,
  type SelectedRuntime,
} from "./trustHomeModel";
import { workspaceHasReadableTrustProject } from "./workspaceProject";
import {
  announceSimulationSpeedRead,
  isRunningSimulator,
  latestSimulatorSpeedRead,
  onDidChangeSimulationSpeed,
  readSelectedSimulatorSpeed,
  SPEED_REFRESH_INTERVAL_MS,
  type SimulatorStatusRead,
} from "./simulationSpeed";

/** What the status bar shows: its text, whether a simulator runs under the selected target, and whether its speed is behind. */
export interface StatusBarView {
  readonly text: string;
  readonly simulatorRunning: boolean;
  readonly speedBehind: boolean;
}

type StatusBarRuntime = Pick<
  RuntimeLifecycleSnapshot["status"],
  "runtimeState" | "runtimeMode" | "endpointReachable" | "endpoint"
>;

export function connectedRuntimeStatusText(status: StatusBarRuntime): string | undefined {
  const connected = status.runtimeState === "connected" ||
    (status.runtimeMode === "online" && status.endpointReachable);
  if (!connected) return undefined;
  return `$(plug) truST: ${connectedEndpointLabel(status.endpoint)} connected`;
}

// §UX (reset 2026-06-22) — the status bar is PASSIVE. It shows the ACTIVE runtime's honest state and,
// on click, reveals the truST sidebar. It contributes NO Start/Stop command: there is exactly ONE
// run surface (the sidebar action row / canvas runtime node). Never fabricate running/connected.

export function registerRuntimeControls(context: vscode.ExtensionContext): void {
  const item = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    100
  );
  // Click reveals the truST sidebar, whose Run section shows the speed and Change…. It is never a run control.
  item.command = "trust.home.focus";
  item.tooltip = "Open the truST sidebar";
  context.subscriptions.push(item);

  let speedTimer: NodeJS.Timeout | undefined;
  let refreshing = false;

  async function refresh(): Promise<void> {
    refreshing = true;
    try {
      const snapshot = await runtimeLifecycleService.snapshot();
      const view = await statusText(context, snapshot);
      item.text = view.text;
      item.command = "trust.home.focus";
      item.tooltip = view.simulatorRunning ? "Open the truST sidebar to see or change the simulation speed" : "Open the truST sidebar";
      item.backgroundColor = view.speedBehind
        ? new vscode.ThemeColor("statusBarItem.warningBackground")
        : undefined;
      item.show();
      followSpeed(view.simulatorRunning);
    } finally {
      refreshing = false;
    }
  }

  // While a simulator runs, its achieved speed is read at the existing 1.5 s canvas cadence; the timer stops with the run.
  function followSpeed(running: boolean): void {
    if (running && speedTimer === undefined) {
      speedTimer = setInterval(() => {
        if (!refreshing) void refresh();
      }, SPEED_REFRESH_INTERVAL_MS);
    } else if (!running && speedTimer !== undefined) {
      clearInterval(speedTimer);
      speedTimer = undefined;
    }
  }
  context.subscriptions.push({
    dispose: () => {
      if (speedTimer !== undefined) clearInterval(speedTimer);
    },
  });
  context.subscriptions.push(
    onDidChangeSimulationSpeed(() => {
      void refresh();
    })
  );

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

/** What the status bar shows for a lifecycle snapshot and the shared target selection. */
export async function statusText(
  context: vscode.ExtensionContext,
  snapshot: RuntimeLifecycleSnapshot
): Promise<StatusBarView> {
  if (!(await workspaceHasReadableTrustProject())) {
    return { text: "$(circle-outline) truST: No project", simulatorRunning: false, speedBehind: false };
  }
  const connected = connectedRuntimeStatusText(snapshot.status);
  if (connected) {
    const label = snapshot.status.targetLabel?.trim();
    return {
      text: label ? `$(plug) truST: ${label} connected` : connected,
      simulatorRunning: false,
      speedBehind: false,
    };
  }
  const remotes = readRemotes();
  const managed = await listManagedRuntimes(context);
  const selected = selectedRuntime({
    snapshot: toModelSnapshot(snapshot),
    remotes,
    managed,
    selectedId: getSelectedRuntimeId(),
  });
  const speed = isRunningSimulator(selected) ? await readSelectedSimulatorSpeed(snapshot) : undefined;
  // Each read that gets a status announces it. This read is the only source of the speed event: no timer of its own.
  if (speed) announceSimulationSpeedRead({ runtimeId: selected.id, ...speed });
  else if (selected.id === SIMULATOR_RUNTIME_ID && !isRunningSimulator(selected) && latestSimulatorSpeedRead()?.configured !== 1) {
    // A stopped simulator runs at no speed: its labels drop the speed and keep "Simulation" (specification 35, B9.2).
    announceSimulationSpeedRead({ runtimeId: selected.id, simulated: true, configured: 1, speedText: "", speedBehind: false });
  }
  return selectedStatusView(selected, speed);
}

/** The selected target's status, with the simulator's speed while it runs (specification 35, B9.2: "Simulator ×10"). The words,
 * the behind flag and the paused state are the runtime's: a scan held at a breakpoint is "paused", not behind. */
export function selectedStatusView(
  selected: SelectedRuntime,
  speed?: Pick<SimulatorStatusRead, "speedText" | "speedBehind" | "paused">
): StatusBarView {
  const running = isRunningSimulator(selected);
  if (!running || speed === undefined) {
    return { text: selectedStatusText(selected), simulatorRunning: running, speedBehind: false };
  }
  const text = speed.speedText;
  const [icon, state] = speed.paused ? ["$(debug-pause)", "paused"] : ["$(circle-filled)", "running"];
  return {
    text: `${icon} truST: ${statusTargetLabel(selected)}${text ? ` ${text}` : ""} ${state}`,
    simulatorRunning: true,
    speedBehind: !speed.paused && speed.speedBehind,
  };
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
    getTrustConfiguration().get<string[]>("runtime.fleetEndpoints", []) ?? [];
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
