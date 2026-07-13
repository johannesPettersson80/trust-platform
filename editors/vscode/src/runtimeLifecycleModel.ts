import type * as vscode from "vscode";

import type { AdsLiveValuesState } from "./adsLiveValuesModel";
import type { RuntimeStatusPayload } from "./io-panel/types";
import type { IoState } from "./io-panel/types";
import type {
  NetworkCanvasRuntimeFailure as RuntimeStartFailure,
  NetworkCanvasRuntimeFailureKind as RuntimeStartFailureKind,
} from "./networkCanvas/runtimeFailures";

export const SESSION_WAIT_TIMEOUT_MS = 8000;
export const SESSION_WAIT_POLL_MS = 100;
export const SESSION_START_STABILITY_MS = 2500;
export const DEBUG_START_COMMAND_TIMEOUT_MS = 20000;
export const DEBUG_STOP_REQUEST_TIMEOUT_MS = 2500;
export const IO_NEXT_SCAN_TIMEOUT_MS = 1200;
export const IO_NEXT_SCAN_POLL_MS = 60;
export const MANAGED_RUNTIME_ID_FIELD = "__trustManagedRuntimeId";

export type { RuntimeStartFailure, RuntimeStartFailureKind };

export type RuntimeLifecycleResult =
  | { readonly ok: true; readonly message: string }
  | { readonly ok: false; readonly failure: RuntimeStartFailure };

export function runtimeOperationConflict(
  message: string,
): RuntimeLifecycleResult {
  return {
    ok: false,
    failure: { kind: "failed_spawn", message },
  };
}

export type RuntimeLifecycleSnapshot = {
  readonly status: RuntimeStatusPayload;
  readonly ioState: IoState;
  readonly adsState: AdsLiveValuesState;
  readonly starting: boolean;
  readonly operation?: RuntimeLifecycleOperationState;
  readonly transitionTarget?: RuntimeLifecycleTarget;
  readonly activeTarget?: RuntimeLifecycleTarget;
  readonly failure?: RuntimeStartFailure;
  readonly failureScope?: RuntimeLifecycleFailureScope;
};

export type RuntimeLifecycleOperationKind =
  | "compile"
  | "apply_changes"
  | "local_start"
  | "local_stop"
  | "remote_connect"
  | "remote_disconnect"
  | "managed_start"
  | "managed_stop";

export type RuntimeLifecycleTarget =
  | { readonly kind: "simulator" }
  | {
      readonly kind: "remote";
      readonly endpoint: string;
      readonly label?: string;
    }
  | {
      readonly kind: "managed";
      readonly id: string;
      readonly endpoint?: string;
    };

export interface RuntimeLifecycleOperationState {
  readonly id: string;
  readonly kind: RuntimeLifecycleOperationKind;
  readonly target: RuntimeLifecycleTarget;
}

export function runtimeOperationChangesPhase(
  kind: RuntimeLifecycleOperationKind,
): boolean {
  return ![
    "compile",
    "apply_changes",
    "local_stop",
    "remote_disconnect",
    "managed_stop",
  ].includes(kind);
}

export type RuntimeLifecycleFailureScope =
  | { readonly kind: "simulator" }
  | { readonly kind: "remote"; readonly endpoint?: string };

export interface RuntimeLifecycleChange {
  readonly kind: "lifecycle" | "io";
}

export function isStructuralRuntimeLifecycleChange(
  change: RuntimeLifecycleChange,
): boolean {
  return change.kind === "lifecycle";
}

export const EMPTY_IO_STATE: IoState = {
  inputs: [],
  outputs: [],
  memory: [],
};

export async function withTimeout<T>(
  promise: Thenable<T>,
  timeoutMs: number,
  timeoutMessage: string,
): Promise<T> {
  let timer: NodeJS.Timeout | undefined;
  try {
    return await Promise.race([
      promise,
      new Promise<T>((_, reject) => {
        timer = setTimeout(() => reject(new Error(timeoutMessage)), timeoutMs);
      }),
    ]);
  } finally {
    if (timer) {
      clearTimeout(timer);
    }
  }
}

export function structuredTextSessionKey(session: vscode.DebugSession): string {
  return session.id ?? session.name;
}

export function runtimeFailureScopeForSession(
  session: vscode.DebugSession,
): RuntimeLifecycleFailureScope {
  if (session.configuration.request === "attach") {
    const endpoint = session.configuration.endpoint;
    return {
      kind: "remote",
      ...(typeof endpoint === "string" && endpoint.trim()
        ? { endpoint: endpoint.trim() }
        : {}),
    };
  }
  return { kind: "simulator" };
}

export function runtimeTargetForSession(
  session: vscode.DebugSession,
): RuntimeLifecycleTarget {
  if (session.configuration.request !== "attach") {
    return { kind: "simulator" };
  }
  const endpoint = session.configuration.endpoint;
  const managedRuntimeId = session.configuration[MANAGED_RUNTIME_ID_FIELD];
  if (typeof managedRuntimeId === "string" && managedRuntimeId.trim()) {
    return {
      kind: "managed",
      id: managedRuntimeId.trim(),
      ...(typeof endpoint === "string" && endpoint.trim()
        ? { endpoint: endpoint.trim() }
        : {}),
    };
  }
  const label = session.configuration.targetLabel;
  return {
    kind: "remote",
    endpoint: typeof endpoint === "string" ? endpoint.trim() : "",
    ...(typeof label === "string" && label.trim()
      ? { label: label.trim() }
      : {}),
  };
}

export function normalizeIoState(value: unknown): IoState {
  if (!isRecord(value)) {
    return EMPTY_IO_STATE;
  }
  const scan =
    typeof value.scan === "number" && Number.isFinite(value.scan)
      ? value.scan
      : undefined;
  return {
    scan,
    inputs: normalizeIoEntries(value.inputs),
    outputs: normalizeIoEntries(value.outputs),
    memory: normalizeIoEntries(value.memory),
  };
}

function normalizeIoEntries(value: unknown): IoState["inputs"] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .map((entry): IoState["inputs"][number] | undefined => {
      if (!isRecord(entry)) {
        return undefined;
      }
      const address = typeof entry.address === "string" ? entry.address : "";
      const rawValue = typeof entry.value === "string" ? entry.value : "";
      if (!address || rawValue.length === 0) {
        return undefined;
      }
      const normalized: IoState["inputs"][number] = {
        address,
        value: formatIoValue(rawValue),
        forced: entry.forced === true,
      };
      if (typeof entry.name === "string") {
        normalized.name = entry.name;
      }
      if (typeof entry.source === "string") {
        normalized.source = entry.source;
      }
      const valueType =
        typeof entry.valueType === "string"
          ? entry.valueType
          : typeof entry.value_type === "string"
            ? entry.value_type
            : typeof entry.type === "string"
              ? entry.type
              : undefined;
      if (valueType) {
        normalized.valueType = valueType.toUpperCase();
      }
      return normalized;
    })
    .filter((entry): entry is IoState["inputs"][number] => entry !== undefined);
}

function formatIoValue(rawValue: string): string {
  const trimmed = rawValue.trim();
  const boolMatch = /^Bool\((true|false)\)$/i.exec(trimmed);
  if (boolMatch) {
    return boolMatch[1].toLowerCase() === "true" ? "TRUE" : "FALSE";
  }
  const simpleValue =
    /^(?:S?Int|DInt|LInt|U?Int|UDInt|ULInt|Real|LReal|Byte|Word|DWord|LWord)\((.*)\)$/i.exec(
      trimmed,
    );
  if (simpleValue) {
    return simpleValue[1];
  }
  return rawValue;
}

export function runtimeDebugDisabled(value: unknown): boolean {
  if (!isRecord(value)) {
    return false;
  }
  if (value.debug_enabled === false) {
    return true;
  }
  const controlStatus = value.control_status;
  return isRecord(controlStatus) && controlStatus.debug_enabled === false;
}

export function runtimeNotReachableMessage(endpoint: string): string {
  if (endpoint.trim().startsWith("unix://")) {
    return "Local runtime is stopped. Start it to connect.";
  }
  return `Runtime is not reachable at ${shortRuntimeEndpointLabel(endpoint)}.`;
}

export function remoteDebugSessionName(
  targetLabel: string | undefined,
  endpoint: string,
): string {
  const label = targetLabel?.trim() || shortRuntimeEndpointLabel(endpoint);
  return label ? `truST Remote (${label})` : "truST Remote";
}

export function shortRuntimeEndpointLabel(endpoint: string): string {
  const text = endpoint.trim();
  if (!text) {
    return "the configured endpoint";
  }
  if (text.startsWith("tcp://")) {
    try {
      return new URL(text).host || "the configured endpoint";
    } catch {
      return "the configured endpoint";
    }
  }
  if (text.startsWith("unix://")) {
    return "the local control socket";
  }
  return text;
}

export function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
