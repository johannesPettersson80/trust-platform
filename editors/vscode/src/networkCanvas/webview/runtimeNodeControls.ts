// Honest per-runtime controls for the Network Canvas runtime-node inspector (vscode-ux-overhaul-plan
// §8 P3b). Pure (no React/vscode) so it is unit-testable. The HARD rule: a remote runtime — one whose
// process we do NOT own — NEVER gets "Start"/"Stop". It gets "Connect"/"Disconnect" (we only manage
// OUR connection). The local simulator (we own the process) gets "Start"/"Stop".

export interface RuntimeNodeControl {
  // The webview→panel message action. Local lifecycle reuses the existing canvas actions
  // ("startLocalSimulator"/"stopLocalSimulator"/"openRuntimeLogs"/"openRuntimeSettings"); remote uses
  // dedicated messages ("runtimeConnect"/"runtimeDisconnect"). "none" = no-op (disabled progress).
  readonly action:
    | "startLocalSimulator"
    | "stopLocalSimulator"
    | "runtimeConnect"
    | "runtimeDisconnect"
    | "setAsRunTarget"
    | "openRuntimeLogs"
    | "openRuntimeSettings"
    | "none";
  readonly label: string;
  readonly kind: "primary" | "secondary";
  readonly enabled: boolean;
}

export interface RuntimeNodeControlsInput {
  readonly isLocal: boolean;
  readonly health: string; // the runtime's OWN health: "connected" | "pending" | "stopped" | "error" | …
  readonly attached: boolean; // does the extension hold a live connection to THIS runtime?
  readonly controlEndpoint?: string;
  // Whether a log backend exists for this node (§0.6.12 — "Logs only when a log backend exists").
  // Remote logs are phase 14, so this is false for remotes until that lands.
  readonly logsAvailable?: boolean;
}

export function runtimeNodeControls(
  input: RuntimeNodeControlsInput
): RuntimeNodeControl[] {
  return [
    primaryControl(input),
    // "Set as run target" selects this runtime for the Run bar WITHOUT connecting (§0.5.11). Connecting
    // also sets the target, but this is the select-only path.
    {
      action: "setAsRunTarget",
      label: "Set as run target",
      kind: "secondary",
      enabled: true,
    },
    // Logs only when a log backend exists for this node (remote logs = phase 14).
    ...(input.logsAvailable
      ? [
          {
            action: "openRuntimeLogs" as const,
            label: "Logs",
            kind: "secondary" as const,
            enabled: true,
          },
        ]
      : []),
    {
      action: "openRuntimeSettings",
      label: "Settings",
      kind: "secondary",
      enabled: true,
    },
  ];
}

function primaryControl(input: RuntimeNodeControlsInput): RuntimeNodeControl {
  if (input.isLocal) {
    // We own the local simulator process → Start / Stop.
    if (input.health === "pending") {
      return { action: "none", label: "Starting…", kind: "primary", enabled: false };
    }
    if (input.health === "connected") {
      return {
        action: "stopLocalSimulator",
        label: "Stop",
        kind: "primary",
        enabled: true,
      };
    }
    return {
      action: "startLocalSimulator",
      label: "Start",
      kind: "primary",
      enabled: true,
    };
  }
  // Remote runtime: we do NOT own the process → Connect / Disconnect, NEVER Stop.
  if (input.attached) {
    return {
      action: "runtimeDisconnect",
      label: "Disconnect",
      kind: "primary",
      enabled: true,
    };
  }
  return {
    action: "runtimeConnect",
    label: "Connect",
    kind: "primary",
    // Can only connect if we know where to connect.
    enabled: !!input.controlEndpoint,
  };
}
