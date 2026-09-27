import type { LoopMessageScope } from "./messages";
import type { RuntimeLifecycleResult, RuntimeLifecycleSnapshot } from "../runtimeLifecycle";

export interface ControlStudioRuntimeStartScope {
  schemaVersion: 1;
  projectId: string;
  editorSessionId: string;
  loopId: string;
  loopRevision: string;
  runtimeId: string;
  runtimeMode: "simulate";
}

export type ControlStudioRuntimeStartResult =
  | { readonly ok: true; readonly message: string }
  | { readonly ok: false; readonly failure: { readonly kind: "invalid_scope" | "not_simulator" | "already_starting" | "start_failed"; readonly message: string } };

export interface ControlStudioRuntimeLifecycle {
  snapshot(): Promise<Pick<RuntimeLifecycleSnapshot, "status" | "starting">>;
  startLocalSimulator(): Promise<RuntimeLifecycleResult>;
}

/** Creates the one scoped host action used by the Control Studio Start simulator button. */
export function createControlStudioRuntimeStartHandler(lifecycle: ControlStudioRuntimeLifecycle) {
  let pending: Promise<ControlStudioRuntimeStartResult> | undefined;
  let pendingScope: string | undefined;

  return async function start(request: unknown, expected: ControlStudioRuntimeStartScope): Promise<ControlStudioRuntimeStartResult> {
    if (!sameScope(request, expected)) {
      return failure("invalid_scope", "The Control Studio scope or loop revision changed. Reopen the current loop before starting the simulator.");
    }
    const scopeKey = JSON.stringify(expected);
    if (pending) return pendingScope === scopeKey ? pending : failure("already_starting", "Another simulator start is still finishing. Wait and retry.");
    pendingScope = scopeKey;
    const operation = (async () => {
      const snapshot = await lifecycle.snapshot();
      if (snapshot.status.runtimeMode !== "simulate") {
        return failure("not_simulator", "Start is available only for the local simulator. Connect to a physical runtime explicitly.");
      }
      if (snapshot.status.running) return { ok: true, message: "Simulator is already running." } as const;
      if (snapshot.starting) return failure("already_starting", "Simulator start is already in progress.");
      return lifecycle.startLocalSimulator()
        .then(result => result.ok
          ? { ok: true, message: result.message || "Simulator started." } as const
          : failure("start_failed", result.failure.message))
        .catch(error => failure("start_failed", error instanceof Error ? error.message : String(error)));
    })()
      .catch(error => failure("start_failed", error instanceof Error ? error.message : String(error)))
      .finally(() => { pending = undefined; pendingScope = undefined; });
    pending = operation;
    return operation;
  };
}

function sameScope(value: unknown, expected: ControlStudioRuntimeStartScope): value is ControlStudioRuntimeStartScope {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Record<string, unknown>;
  return candidate.schemaVersion === 1 && candidate.projectId === expected.projectId &&
    candidate.editorSessionId === expected.editorSessionId && candidate.loopId === expected.loopId &&
    candidate.loopRevision === expected.loopRevision && candidate.runtimeId === expected.runtimeId &&
    candidate.runtimeMode === "simulate";
}

function failure(kind: Extract<ControlStudioRuntimeStartResult, { ok: false }>["failure"]["kind"], message: string): ControlStudioRuntimeStartResult {
  return { ok: false, failure: { kind, message } };
}

export async function routeControlStudioRuntimeStart(message: unknown, scope: LoopMessageScope, runtimeId: string,
  start: ReturnType<typeof createControlStudioRuntimeStartHandler>, post: (value: Record<string, unknown>) => Promise<unknown>): Promise<boolean> {
  const raw = message as Record<string, unknown> | undefined;
  if (raw?.kind !== "control-studio.runtime.start") return false;
  if (raw.schemaVersion !== 1 || raw.projectId !== scope.projectId || raw.editorSessionId !== scope.editorSessionId || raw.loopId !== scope.loopId || raw.expectedLoopRevision !== scope.loopRevision) return true;
  const payload = raw.payload as Record<string, unknown> | undefined;
  const result = await start({ schemaVersion: raw.schemaVersion, projectId: raw.projectId, editorSessionId: raw.editorSessionId,
    loopId: raw.loopId, loopRevision: raw.expectedLoopRevision, runtimeId: payload?.runtimeId, runtimeMode: payload?.runtimeMode },
    { schemaVersion: 1, ...scope, runtimeId, runtimeMode: "simulate" });
  await post({ kind: "control-studio.runtime.start.result", schemaVersion: 1, ...scope, runtimeId, result });
  return true;
}
