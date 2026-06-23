// Pure model for the truST Run card — NO vscode import, so it's unit-testable standalone.
// The Run card (trustHomeView.ts WebviewView) renders these; the contract test asserts the dropdown
// options, the single state-specific action, and the honesty rules.
//
// Product goal (vscode-ux-overhaul-plan.md §0/§6): one runtime selector + ONE state-specific action.
// Button labels are LITERAL verbs (Start / Stop / Connect / Disconnect) — never "toggle"/"attach".
// HARD honesty rule: a remote NEVER renders "Stop" (we don't own its process) — it renders
// "Disconnect" (we only drop our own connection).

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
export type RuntimeAction = "start" | "stop" | "connect" | "disconnect" | "none";

export const SIMULATOR_RUNTIME_ID = "simulator";
export const LOCAL_RUNTIME_ID = "local";

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
}

export interface RuntimeModelInput {
  readonly snapshot: RuntimeModelSnapshot;
  readonly remotes: ReadonlyArray<RemoteRuntime>;
  readonly localSupported: boolean;
  readonly selectedId: string;
}

// The dropdown is SELECT-ONLY (§0.5.3): Simulator (default) → Local runtime (when supported) →
// configured remotes. There is NO "Add…/Connect…" entry — adding or connecting a runtime happens in
// Devices & Connections, never in the Run-bar dropdown.
export function runtimeOptions(
  remotes: ReadonlyArray<RemoteRuntime>,
  localSupported: boolean
): RuntimeOption[] {
  const options: RuntimeOption[] = [
    {
      id: SIMULATOR_RUNTIME_ID,
      label: "Simulator (this computer)",
      kind: "simulator",
    },
  ];
  if (localSupported) {
    options.push({
      id: LOCAL_RUNTIME_ID,
      label: "Local runtime (this computer)",
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
  const options = runtimeOptions(input.remotes, input.localSupported);
  const chosen =
    options.find((option) => option.id === input.selectedId) ?? options[0];
  switch (chosen.kind) {
    case "remote":
      return remoteRuntime(chosen, input.snapshot);
    case "local":
    case "simulator":
    default:
      return localOrSimulator(chosen, input.snapshot);
  }
}

// Simulator + persistent local runtime share start/stop semantics (we own the process).
function localOrSimulator(
  option: RuntimeOption,
  snapshot: RuntimeModelSnapshot
): SelectedRuntime {
  if (snapshot.starting) {
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
  snapshot: RuntimeModelSnapshot
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
  if (isActiveEndpoint && snapshot.endpointConfigured && !snapshot.endpointReachable) {
    return runtime(option, "unreachable", "Unreachable", {
      action: "connect",
      label: "Connect",
      enabled: true,
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
  primary: PrimaryAction
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

// Friendly host label from a control endpoint string (e.g. "tcp://raspberrypi:5680" → "raspberrypi").
export function remoteLabelFromEndpoint(endpoint: string): string {
  const trimmed = endpoint.trim();
  if (!trimmed) {
    return "runtime";
  }
  return (
    trimmed
      .replace(/^[a-z]+:\/\//i, "")
      .split("/")[0]
      .split(":")[0] || "runtime"
  );
}
