// Honest per-runtime controls for the Network Canvas runtime-node inspector (vscode-ux-overhaul-plan
// §8 P3b). Pure (no React/vscode) so it is unit-testable. The HARD rule: a remote runtime — one whose
// process we do NOT own — NEVER gets "Start"/"Stop". It gets "Connect"/"Disconnect" (we only manage
// OUR connection). The local Simulator is controlled only from the truST sidebar, so its canvas
// inspector deliberately has no Start/Stop action.

import type { LifecyclePhase } from "../../lifecycleEntryFailure";
import {
  runtimeOperationBlockReason,
  type RuntimeLockedAction,
} from "../../runtimeOperationPolicy";
import {
  AUTHORITY_CHECK_RUNTIME_NODE_ID,
  LOCAL_RUNTIME_NODE_ID,
} from "./types";

export interface RuntimeNodeControl {
  // The webview-to-panel message action. Simulator lifecycle is intentionally
  // absent: the truST sidebar is its only Start/Stop owner.
  readonly action:
    | "managedStart"
    | "managedStop"
    | "runtimeConnect"
    | "runtimeDisconnect"
    | "setAuthToken"
    | "setAsRunTarget"
    | "openRuntimeLogs"
    | "openRuntimeSettings"
    | "none";
  readonly label: string;
  readonly kind: "primary" | "secondary";
  readonly enabled: boolean;
  readonly disabledReason?: string;
}

export interface RuntimeNodeActionItem {
  readonly key: string;
  readonly label: string;
  readonly enabled: boolean;
  readonly title?: string;
  readonly onClick: () => void;
}

export interface RuntimeNodeControlLayout {
  readonly primary?: RuntimeNodeControl;
  readonly visibleSecondary: RuntimeNodeActionItem[];
  readonly overflowSecondary: RuntimeNodeActionItem[];
  readonly hasOverflow: boolean;
}

export interface RuntimeNodeControlsInput {
  readonly isLocal: boolean;
  readonly health: string; // the runtime's OWN health: "connected" | "starting" | "stopped" | "error" | …
  readonly attached: boolean; // does the extension hold a live connection to THIS runtime?
  readonly controlEndpoint?: string;
  // A managed local runtime (fleet.toml project we own — Phase 9): Start/Stop via the fleet lifecycle.
  readonly managed?: boolean;
  // Whether a log backend exists for this node (§0.6.12 — "Logs only when a log backend exists").
  // Remote logs are phase 14, so this is false for remotes until that lands.
  readonly logsAvailable?: boolean;
  // Auth failures need a direct credential recovery action in the inspector (S-23). This does not
  // change lifecycle ownership: Connect remains available as a retry, but Set auth token becomes the
  // primary next action because the error has already proven Connect cannot succeed without it.
  readonly authTokenRequired?: boolean;
  // The host lifecycle authority owns one transition/session at a time. The
  // webview mirrors that policy for immediate feedback; the host rechecks it.
  readonly lifecyclePhase?: LifecyclePhase;
  readonly operationInProgress?: boolean;
}

export interface RuntimeNodeControlsForNodeInput extends RuntimeNodeControlsInput {
  readonly nodeId: string;
}

/** Pending authority and Simulator nodes never expose canvas lifecycle controls. */
export function runtimeNodeControlsForNode(
  input: RuntimeNodeControlsForNodeInput,
): RuntimeNodeControl[] {
  if (
    input.nodeId === LOCAL_RUNTIME_NODE_ID ||
    input.nodeId === AUTHORITY_CHECK_RUNTIME_NODE_ID
  ) {
    return [];
  }
  const { nodeId: _nodeId, ...controlsInput } = input;
  return runtimeNodeControls(controlsInput);
}

export function runtimeNodeControls(
  input: RuntimeNodeControlsInput,
): RuntimeNodeControl[] {
  const primary = primaryControl(input);
  const controls: RuntimeNodeControl[] = [
    ...(primary ? [primary] : []),
    ...(input.authTokenRequired && input.controlEndpoint
      ? [
          ...(primary?.action === "setAuthToken"
            ? [
                {
                  action: "runtimeConnect" as const,
                  label: "Connect",
                  kind: "secondary" as const,
                  enabled: true,
                },
              ]
            : []),
          ...(primary?.action === "setAuthToken"
            ? []
            : [
                {
                  action: "setAuthToken" as const,
                  label: "Set auth token",
                  kind: "secondary" as const,
                  enabled: true,
                },
              ]),
        ]
      : []),
    // "Select as target" selects this runtime for the sidebar WITHOUT connecting (§0.5.11). Connecting
    // also sets the target, but this is the select-only path.
    {
      action: "setAsRunTarget",
      label: "Select as target",
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
  return controls.map((control) =>
    controlWithLifecyclePolicy(
      control,
      input.lifecyclePhase ?? "stopped",
      input.attached,
      input.operationInProgress ?? false,
    ),
  );
}

export const MAX_VISIBLE_RUNTIME_NODE_SECONDARY = 2;

export function runtimeNodeControlLayout(
  controls: readonly RuntimeNodeControl[] | undefined,
  onControl: ((control: RuntimeNodeControl) => void) | undefined,
  extraSecondary: readonly RuntimeNodeActionItem[] = [],
  showAllSecondary = false,
): RuntimeNodeControlLayout {
  if (!controls || !onControl) {
    return {
      primary: undefined,
      visibleSecondary: [],
      overflowSecondary: [],
      hasOverflow: false,
    };
  }
  const secondary = [
    ...controls
      .filter((control) => control.kind === "secondary")
      .map((control) => ({
        key: `${control.action}:${control.label}`,
        label: control.label,
        enabled: control.enabled,
        title: control.disabledReason,
        onClick: () => onControl(control),
      })),
    ...extraSecondary,
  ];
  const visibleSecondary = showAllSecondary
    ? secondary
    : secondary.slice(0, MAX_VISIBLE_RUNTIME_NODE_SECONDARY);
  return {
    primary: controls.find((control) => control.kind === "primary"),
    visibleSecondary,
    overflowSecondary: secondary.slice(MAX_VISIBLE_RUNTIME_NODE_SECONDARY),
    hasOverflow: secondary.length > MAX_VISIBLE_RUNTIME_NODE_SECONDARY,
  };
}

function controlWithLifecyclePolicy(
  control: RuntimeNodeControl,
  phase: LifecyclePhase,
  ownsAcceptedSession: boolean,
  operationInProgress: boolean,
): RuntimeNodeControl {
  const operation = lockedOperationForControl(control.action);
  if (!operation) {
    return control;
  }
  const disabledReason =
    control.action === "managedStop" &&
    phase !== "stopped" &&
    !ownsAcceptedSession
      ? runtimeOperationBlockReason(phase, "managed_start", operationInProgress)
      : runtimeOperationBlockReason(phase, operation, operationInProgress);
  return disabledReason
    ? { ...control, enabled: false, disabledReason }
    : control;
}

function lockedOperationForControl(
  action: RuntimeNodeControl["action"],
): RuntimeLockedAction | undefined {
  switch (action) {
    case "runtimeConnect":
      return "remote_connect";
    case "runtimeDisconnect":
      return "remote_disconnect";
    case "managedStart":
      return "managed_start";
    case "managedStop":
      return "managed_stop";
    case "setAsRunTarget":
      return "set_run_target";
    default:
      return undefined;
  }
}

function primaryControl(
  input: RuntimeNodeControlsInput,
): RuntimeNodeControl | undefined {
  if (input.managed) {
    // A managed local runtime project — we own the process → Start / Stop (never Connect).
    return input.health === "connected"
      ? { action: "managedStop", label: "Stop", kind: "primary", enabled: true }
      : {
          action: "managedStart",
          label: "Start",
          kind: "primary",
          enabled: true,
        };
  }
  if (input.isLocal) {
    // One lifecycle surface: the truST sidebar owns Simulator Start/Stop.
    return undefined;
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
  if (input.authTokenRequired && input.controlEndpoint) {
    return {
      action: "setAuthToken",
      label: "Set auth token",
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
