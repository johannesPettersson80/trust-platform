import type * as vscode from "vscode";

import { debugChannel } from "./debug/configuration";
import { simulatorControlFromDebugConfiguration } from "./simControl";
import {
  classifyRuntimeStartFailure,
  simulatorStartupIncompleteFailure,
} from "./networkCanvas/runtimeFailures";
import { requestRuntimeStatus } from "./runtimeControlClient";
import {
  DEBUG_STOP_REQUEST_TIMEOUT_MS,
  delay,
  isRecord,
  SESSION_WAIT_POLL_MS,
  structuredTextSessionKey,
  withTimeout,
  type RuntimeLifecycleResult,
  type RuntimeStartFailure,
} from "./runtimeLifecycleModel";

export async function waitForLifecycleSession(
  findSession: () => vscode.DebugSession | undefined,
  timeoutMs: number
): Promise<vscode.DebugSession | undefined> {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    const session = findSession();
    if (session) {
      return session;
    }
    await delay(SESSION_WAIT_POLL_MS);
  }
  return findSession();
}

export async function waitForSessionPresence(
  session: vscode.DebugSession,
  isTracked: (key: string) => boolean,
  expected: boolean,
  timeoutMs: number
): Promise<boolean> {
  const key = structuredTextSessionKey(session);
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    if (isTracked(key) === expected) {
      return true;
    }
    await delay(SESSION_WAIT_POLL_MS);
  }
  return isTracked(key) === expected;
}

/**
 * Proves that an announced debug session remains present for the complete
 * acceptance window. `waitForSessionPresence(..., true, ...)` is not a
 * stability check: it succeeds on the first observation. Startup uses this
 * dwell check so a DAP session that exits immediately after readiness cannot
 * be published as Running/Connected.
 */
export async function waitForSessionStable(
  session: vscode.DebugSession,
  isTracked: (key: string) => boolean,
  stabilityMs: number,
  pollMs = SESSION_WAIT_POLL_MS
): Promise<boolean> {
  const key = structuredTextSessionKey(session);
  if (!isTracked(key)) {
    return false;
  }
  const deadline = Date.now() + Math.max(0, stabilityMs);
  while (Date.now() < deadline) {
    const remaining = deadline - Date.now();
    await delay(Math.min(Math.max(1, pollMs), Math.max(1, remaining)));
    if (!isTracked(key)) {
      return false;
    }
  }
  return isTracked(key);
}

export async function waitForAttachedSessionReady(
  session: vscode.DebugSession,
  timeoutMs: number,
  isTracked: (key: string) => boolean,
  requestIoState: (
    session: vscode.DebugSession,
    timeoutMs: number
  ) => Promise<RuntimeLifecycleResult>
): Promise<RuntimeLifecycleResult> {
  const key = structuredTextSessionKey(session);
  const deadline = Date.now() + timeoutMs;
  let lastFailure: RuntimeStartFailure = {
    kind: "failed_spawn",
    message: "Runtime debug attachment is still loading I/O.",
  };
  do {
    if (!isTracked(key)) {
      return {
        ok: false,
        failure: {
          kind: "failed_spawn",
          message: "Runtime debug attachment ended before startup completed.",
        },
      };
    }
    const result = await requestIoState(
      session,
      Math.min(750, Math.max(1, deadline - Date.now()))
    );
    if (result.ok) {
      return result;
    }
    lastFailure = result.failure;
    await delay(SESSION_WAIT_POLL_MS);
  } while (Date.now() < deadline);
  return { ok: false, failure: lastFailure };
}

export async function waitForSimulatorSessionReady(
  session: vscode.DebugSession,
  timeoutMs: number,
  isTracked: (key: string) => boolean,
  requestIoState: (
    session: vscode.DebugSession,
    timeoutMs: number
  ) => Promise<RuntimeLifecycleResult>
): Promise<RuntimeLifecycleResult> {
  const key = structuredTextSessionKey(session);
  const deadline = Date.now() + timeoutMs;
  let lastFailure: RuntimeStartFailure = {
    kind: "failed_spawn",
    message: "Simulator did not finish loading its program and control channel.",
  };
  do {
    if (!isTracked(key)) {
      return {
        ok: false,
        failure: {
          kind: "failed_spawn",
          message: "Simulator stopped before startup completed.",
        },
      };
    }
    try {
      const readiness = await withTimeout(
        session.customRequest("trustSimulatorStatus"),
        Math.min(750, Math.max(1, deadline - Date.now())),
        "Simulator readiness request timed out."
      );
      if (!isRecord(readiness) || readiness.ready !== true) {
        lastFailure = {
          kind: "failed_spawn",
          message: "Simulator is still loading its program.",
        };
        await delay(SESSION_WAIT_POLL_MS);
        continue;
      }
    } catch (error) {
      lastFailure = classifyRuntimeStartFailure(error);
      await delay(SESSION_WAIT_POLL_MS);
      continue;
    }

    const control = simulatorControlFromDebugConfiguration(
      session.configuration
    );
    if (!control) {
      debugChannel().appendLine(
        "Simulator startup could not finish: debug session did not provide required control metadata."
      );
      return { ok: false, failure: simulatorStartupIncompleteFailure() };
    }
    try {
      await requestRuntimeStatus(control.endpoint, control.authToken, {
        timeoutMs: Math.min(750, Math.max(1, deadline - Date.now())),
      });
    } catch (error) {
      lastFailure = classifyRuntimeStartFailure(error);
      await delay(SESSION_WAIT_POLL_MS);
      continue;
    }

    const result = await requestIoState(
      session,
      Math.min(750, Math.max(1, deadline - Date.now()))
    );
    if (result.ok) {
      return result;
    }
    lastFailure = result.failure;
    await delay(SESSION_WAIT_POLL_MS);
  } while (Date.now() < deadline);
  return { ok: false, failure: lastFailure };
}

export async function terminateRejectedSession(
  session: vscode.DebugSession,
  stopDebugging: (session: vscode.DebugSession) => Thenable<unknown>
): Promise<void> {
  try {
    await withTimeout(
      stopDebugging(session),
      DEBUG_STOP_REQUEST_TIMEOUT_MS,
      "Terminating the rejected Structured Text session timed out."
    );
  } catch (error) {
    debugChannel().appendLine(
      `Failed to terminate rejected Structured Text session: ${
        error instanceof Error ? error.message : String(error)
      }`
    );
  }
}
