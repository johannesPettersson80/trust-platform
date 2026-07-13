import type * as vscode from "vscode";

import { getTrustConfiguration } from "./configuration";
import { classifyRuntimeStartFailure } from "./networkCanvas/runtimeFailures";
import {
  DEBUG_STOP_REQUEST_TIMEOUT_MS,
  runtimeOperationConflict,
  runtimeTargetForSession,
  SESSION_WAIT_TIMEOUT_MS,
  withTimeout,
  type RuntimeLifecycleOperationKind,
  type RuntimeLifecycleResult,
  type RuntimeLifecycleSnapshot,
  type RuntimeLifecycleTarget,
} from "./runtimeLifecycleModel";
import { waitForSessionPresence } from "./runtimeSessionReadiness";

type ExclusiveOperationResult<T> =
  | { readonly acquired: true; readonly value: T }
  | { readonly acquired: false; readonly reason: string };

export interface RuntimeStopOperationDependencies {
  readonly activeOperation: () => string | undefined;
  readonly getSession: () => vscode.DebugSession | undefined;
  readonly runExclusive: <T>(
    kind: RuntimeLifecycleOperationKind,
    target: RuntimeLifecycleTarget,
    operation: (operationId: string) => Thenable<T>
  ) => Promise<ExclusiveOperationResult<T>>;
  readonly executeStop: (session: vscode.DebugSession) => Thenable<unknown>;
  readonly hasSession: (key: string) => boolean;
  readonly snapshot: () => Promise<RuntimeLifecycleSnapshot>;
  readonly runtimeConfigTarget: () => vscode.Uri | undefined;
  readonly runtimeConfigScope: (
    target: vscode.Uri | undefined
  ) => vscode.ConfigurationTarget;
  readonly markStopped: (message: string) => RuntimeLifecycleResult;
  readonly emitChanged: () => void;
}

/** Owns Stop/Disconnect from lease acquisition through exact-session exit. */
export async function runRuntimeStopOperation(
  operationId: string | undefined,
  dependencies: RuntimeStopOperationDependencies
): Promise<RuntimeLifecycleResult> {
  if (operationId) {
    if (dependencies.activeOperation() !== operationId) {
      return runtimeOperationConflict(
        "The runtime stop operation is no longer active."
      );
    }
    return stopOwnedRuntime(dependencies);
  }
  const session = dependencies.getSession();
  const attached = session?.configuration.request === "attach";
  const operation = await dependencies.runExclusive(
    attached ? "remote_disconnect" : "local_stop",
    session ? runtimeTargetForSession(session) : { kind: "simulator" },
    () => stopOwnedRuntime(dependencies)
  );
  return operation.acquired
    ? operation.value
    : runtimeOperationConflict(operation.reason);
}

async function stopOwnedRuntime(
  dependencies: RuntimeStopOperationDependencies
): Promise<RuntimeLifecycleResult> {
  const activeSession = dependencies.getSession();
  if (activeSession) {
    try {
      await withTimeout(
        dependencies.executeStop(activeSession),
        DEBUG_STOP_REQUEST_TIMEOUT_MS,
        "Stopping the Structured Text debug session timed out."
      );
    } catch (error) {
      if (
        await waitForSessionPresence(
          activeSession,
          dependencies.hasSession,
          false,
          SESSION_WAIT_TIMEOUT_MS
        )
      ) {
        return dependencies.markStopped("Runtime stopped.");
      }
      return { ok: false, failure: classifyRuntimeStartFailure(error) };
    }
    if (
      await waitForSessionPresence(
        activeSession,
        dependencies.hasSession,
        false,
        SESSION_WAIT_TIMEOUT_MS
      )
    ) {
      return dependencies.markStopped("Runtime stopped.");
    }
    return {
      ok: false,
      failure: {
        kind: "stale_runtime",
        message: "Runtime did not stop. Check the Structured Text debug session.",
      },
    };
  }

  const snapshot = await dependencies.snapshot();
  if (snapshot.status.runtimeState === "connected") {
    const target = dependencies.runtimeConfigTarget();
    const config = getTrustConfiguration(target);
    await config.update(
      "runtime.controlEndpointEnabled",
      false,
      dependencies.runtimeConfigScope(target)
    );
    dependencies.emitChanged();
    return { ok: true, message: "Runtime endpoint disabled." };
  }
  return dependencies.markStopped("Runtime already stopped.");
}
