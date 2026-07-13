import type { ValidityLine } from "./compileGate";
import type { RuntimeLifecycleResult } from "./runtimeLifecycle";
import type {
  RuntimeLifecycleSnapshot,
  RuntimeStartFailure,
} from "./runtimeLifecycleModel";
import type { SelectedRuntime } from "./trustHomeModel";

export const SET_AUTH_TOKEN_ACTION = "Set auth token";
export const OPEN_DEVICES_ACTION = "Open Devices & Connections";
export const OPEN_RUNTIME_TOML_ACTION = "Open runtime.toml";
export const OPEN_RUNTIME_LOGS_ACTION = "Open logs";

export function lifecycleFailureMatchesSelected(
  snapshot: RuntimeLifecycleSnapshot,
  selected: SelectedRuntime,
): boolean {
  const scope = snapshot.failureScope;
  if (!scope) {
    return false;
  }
  if (scope.kind === "simulator") {
    return selected.kind === "simulator";
  }
  return (
    selected.kind === "remote" &&
    (!scope.endpoint || scope.endpoint === selected.id)
  );
}

export function actionFailureMessage(
  selected: SelectedRuntime,
  result: RuntimeLifecycleResult & { ok: false },
): string {
  const reason = result.failure.message;
  switch (selected.primary.action) {
    case "start":
      return /^Simulator\b/i.test(reason.trim())
        ? reason
        : `Could not start the simulator: ${reason}`;
    case "stop":
      return `Could not stop: ${reason}`;
    case "connect":
      if (isRuntimeUnreachableFailure(reason)) {
        return `Could not connect to ${selected.label}. Runtime is not reachable. Open Devices & Connections to start or diagnose this runtime.`;
      }
      return `Could not connect to ${selected.label}: ${reason}`;
    case "disconnect":
      return `Could not disconnect: ${reason}`;
    default:
      return reason;
  }
}

export function startFailureChoices(failure: RuntimeStartFailure): string[] {
  if (failure.kind === "configuration") {
    return [OPEN_RUNTIME_TOML_ACTION];
  }
  return [OPEN_RUNTIME_LOGS_ACTION];
}

export function connectFailureChoices(
  result: RuntimeLifecycleResult & { ok: false },
): string[] {
  const text = `${result.failure.kind} ${result.failure.message} ${result.failure.detail ?? ""}`;
  if (isRuntimeUnreachableFailure(text)) {
    return [OPEN_DEVICES_ACTION];
  }
  if (isAuthTokenFailure(text)) {
    return [SET_AUTH_TOKEN_ACTION];
  }
  return [];
}

function isRuntimeUnreachableFailure(text: string): boolean {
  return /not reachable|unreachable|connection refused|econnrefused|timed out|timeout/i.test(
    text,
  );
}

function isAuthTokenFailure(text: string): boolean {
  return (
    /auth|token|credential|unauthori[sz]ed|permission denied/i.test(text) &&
    !isRuntimeUnreachableFailure(text)
  );
}

export function isReloadSuccess(value: unknown): boolean {
  return isRecord(value) && value.ok === true;
}

export function reloadFailureMessage(
  value: unknown,
  validity: ValidityLine,
): string {
  if (
    isRecord(value) &&
    value.gated === true &&
    typeof value.message === "string" &&
    value.message.trim()
  ) {
    return value.message;
  }
  if (!validity.ok) {
    return "Fix the errors shown in Problems, then try again.";
  }
  if (
    isRecord(value) &&
    typeof value.message === "string" &&
    value.message.trim()
  ) {
    return summarizeReloadMessage(value.message);
  }
  return "Update did not report success. Keep the simulator running, fix any compile errors, and try again.";
}

function summarizeReloadMessage(message: string): string {
  const firstLine =
    message
      .trim()
      .split(/\r?\n/)
      .find((line) => line.trim())
      ?.trim() ?? "";
  if (!firstLine) {
    return "Update did not report a reason.";
  }
  const sourceErrorCount = message
    .split(/\r?\n/)
    .filter((line) => /\.(st|pou):/i.test(line)).length;
  if (sourceErrorCount > 0) {
    return `Compile failed — ${sourceErrorCount} error${sourceErrorCount === 1 ? "" : "s"}. Open Problems, then try again.`;
  }
  if (firstLine.length <= 160) {
    return firstLine;
  }
  return `${firstLine.slice(0, 157).trimEnd()}...`;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
