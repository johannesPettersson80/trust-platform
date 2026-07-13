// Pure model for the truST sidebar — NO vscode import, so it's unit-testable standalone.
// The sidebar (trustHomeView.ts WebviewView) renders these; the contract test asserts the dropdown
// options, the single state-specific action, and the honesty rules.
//
// Product goal (vscode-ux-overhaul-plan.md §0/§6): one runtime selector + ONE state-specific action.
// Button labels are LITERAL verbs (Start / Stop / Connect / Disconnect) — never "toggle"/"attach".
// HARD honesty rule: a remote NEVER renders "Stop" (we don't own its process) — it renders
// "Disconnect" (we only drop our own connection).

import { managedRuntimeLabel, type ManagedRuntime } from "./localRuntimeModel";

export type RuntimeKind = "simulator" | "local" | "remote";

export type RuntimeStatus =
  | "stopped"
  | "starting"
  | "running"
  | "connected"
  | "disconnected"
  | "unreachable";

// The single primary verb for the selected runtime's current state. `none` = no actionable button
// (e.g. mid-transition while starting/connecting).
export type RuntimeAction =
  "start" | "stop" | "connect" | "disconnect" | "none";

export const SIMULATOR_RUNTIME_ID = "simulator";

export interface RuntimeOption {
  readonly id: string;
  readonly label: string;
  readonly kind: RuntimeKind;
}

export interface PrimaryAction {
  readonly action: RuntimeAction;
  // The button text — a literal verb: "Start" | "Stop" | "Connect" | "Disconnect", or a disabled
  // progress label ("Starting…" | "Connecting…").
  readonly label: string;
  readonly enabled: boolean;
  // When the action is disabled for a known reason (e.g. unreachable), the line shown under the button
  // so the user knows why + what to do next (§0.5.10). Omitted when there's nothing to explain.
  readonly hint?: string;
}

export interface SelectedRuntime {
  readonly id: string;
  readonly label: string;
  readonly kind: RuntimeKind;
  readonly status: RuntimeStatus;
  readonly statusLabel: string; // human-facing: "Stopped" | "Running" | "Connected" | …
  readonly primary: PrimaryAction;
}

export interface RemoteRuntime {
  readonly id: string; // the control endpoint string — its identity
  readonly label: string; // friendly host name
}

// The honest slice of RuntimeStatusPayload the model needs, plus the starting flag.
export interface RuntimeModelSnapshot {
  readonly runtimeMode: "simulate" | "online";
  readonly runtimeState: "running" | "connected" | "stopped";
  readonly endpoint: string;
  readonly endpointConfigured: boolean;
  readonly endpointReachable: boolean;
  readonly starting: boolean;
  readonly transitionTargetId?: string;
}

export interface RuntimeModelInput {
  readonly snapshot: RuntimeModelSnapshot;
  readonly remotes: ReadonlyArray<RemoteRuntime>;
  // Managed local runtimes (fleet.toml projects on this computer we own — Phase 9). Each becomes a
  // select-only dropdown entry of kind "local" with Start/Stop driven by its own reported state.
  readonly managed: ReadonlyArray<ManagedRuntime>;
  readonly selectedId: string;
  /** A matching accepted attach remains authoritative until that session exits. */
  readonly managedSessionId?: string;
}

// The dropdown is SELECT-ONLY (§0.5.3): Simulator (default) → managed local runtimes → configured
// remotes. There is NO "Add…/Connect…" entry — adding/connecting happens in Devices & Connections.
export function runtimeOptions(
  remotes: ReadonlyArray<RemoteRuntime>,
  managed: ReadonlyArray<ManagedRuntime>,
): RuntimeOption[] {
  const options: RuntimeOption[] = [
    {
      id: SIMULATOR_RUNTIME_ID,
      label: "Simulator",
      kind: "simulator",
    },
  ];
  for (const local of managed) {
    options.push({
      id: local.name,
      label: managedRuntimeLabel(local.name),
      kind: "local",
    });
  }
  for (const remote of remotes) {
    options.push({ id: remote.id, label: remote.label, kind: "remote" });
  }
  return options;
}

// Resolve the selected runtime + its single state-specific action. Falls back to the simulator if the
// stored selection is no longer in the inventory (removed/stale).
export function selectedRuntime(input: RuntimeModelInput): SelectedRuntime {
  const options = runtimeOptions(input.remotes, input.managed);
  const chosen =
    options.find((option) => option.id === input.selectedId) ?? options[0];
  switch (chosen.kind) {
    case "remote":
      return remoteRuntime(chosen, input.snapshot);
    case "local":
      return managedRuntime(
        chosen,
        input.managed,
        input.snapshot,
        input.managedSessionId,
      );
    case "simulator":
    default:
      return simulatorRuntime(chosen, input.snapshot);
  }
}

// A managed local runtime: we own the process → Start/Stop (never Connect), driven by its reported state.
function managedRuntime(
  option: RuntimeOption,
  managed: ReadonlyArray<ManagedRuntime>,
  snapshot: RuntimeModelSnapshot,
  managedSessionId: string | undefined,
): SelectedRuntime {
  if (snapshot.starting && snapshot.transitionTargetId === option.id) {
    return runtime(option, "starting", "Starting…", {
      action: "none",
      label: "Starting…",
      enabled: false,
    });
  }
  const running =
    managed.find((local) => local.name === option.id)?.state === "running";
  if (!running && managedSessionId === option.id) {
    return runtime(option, "connected", "Live Values still connected", {
      action: "stop",
      label: "Stop",
      enabled: true,
      hint: "The runtime process stopped, but its Live Values session is still connected. Stop again to retry cleanup.",
    });
  }
  return running
    ? runtime(option, "running", "Running", {
        action: "stop",
        label: "Stop",
        enabled: true,
      })
    : runtime(option, "stopped", "Stopped", {
        action: "start",
        label: "Start",
        enabled: true,
      });
}

// The simulator: we own the (debug) process → Start/Stop, driven by the lifecycle snapshot.
function simulatorRuntime(
  option: RuntimeOption,
  snapshot: RuntimeModelSnapshot,
): SelectedRuntime {
  if (
    snapshot.starting &&
    snapshot.transitionTargetId === SIMULATOR_RUNTIME_ID
  ) {
    return runtime(option, "starting", "Starting…", {
      action: "none",
      label: "Starting…",
      enabled: false,
    });
  }
  const running =
    snapshot.runtimeMode === "simulate" && snapshot.runtimeState === "running";
  if (running) {
    return runtime(option, "running", "Running", {
      action: "stop",
      label: "Stop",
      enabled: true,
    });
  }
  return runtime(option, "stopped", "Stopped", {
    action: "start",
    label: "Start",
    enabled: true,
  });
}

function remoteRuntime(
  option: RuntimeOption,
  snapshot: RuntimeModelSnapshot,
): SelectedRuntime {
  const isActiveEndpoint = snapshot.endpoint === option.id;
  const connected =
    isActiveEndpoint &&
    snapshot.runtimeMode === "online" &&
    snapshot.runtimeState === "connected";
  if (connected) {
    // HONESTY: never "Stop" a remote we don't own — only drop our connection.
    return runtime(option, "connected", "Connected", {
      action: "disconnect",
      label: "Disconnect",
      enabled: true,
    });
  }
  if (snapshot.starting && isActiveEndpoint) {
    return runtime(option, "starting", "Connecting…", {
      action: "none",
      label: "Connecting…",
      enabled: false,
    });
  }
  if (
    isActiveEndpoint &&
    snapshot.endpointConfigured &&
    !snapshot.endpointReachable
  ) {
    // Known-unreachable (we probed and it's down): Connect is DISABLED with a reason — never a button
    // that just fails (§0.5.10). Diagnosing/starting a remote happens in Devices & Connections.
    return runtime(option, "unreachable", "Not reachable", {
      action: "connect",
      label: "Connect",
      enabled: false,
      hint: "Not reachable — open Devices & Connections to start or diagnose this runtime.",
    });
  }
  return runtime(option, "disconnected", "Not connected", {
    action: "connect",
    label: "Connect",
    enabled: true,
  });
}

function runtime(
  option: RuntimeOption,
  status: RuntimeStatus,
  statusLabel: string,
  primary: PrimaryAction,
): SelectedRuntime {
  return {
    id: option.id,
    label: option.label,
    kind: option.kind,
    status,
    statusLabel,
    primary,
  };
}

// Friendly label from a control endpoint string. Keep the port when present so two runtimes on the
// same host do not both render as "127.0.0.1" in the Target dropdown.
export function remoteLabelFromEndpoint(endpoint: string): string {
  const trimmed = endpoint.trim();
  if (!trimmed) {
    return "runtime";
  }
  if (/^unix:\/\//i.test(trimmed)) {
    return "runtime";
  }
  const withoutScheme = trimmed.replace(/^[a-z]+:\/\//i, "");
  return withoutScheme.split("/")[0] || withoutScheme || "runtime";
}
