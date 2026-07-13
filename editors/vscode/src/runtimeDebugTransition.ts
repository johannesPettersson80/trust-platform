import type * as vscode from "vscode";

import { classifyRuntimeStartFailure } from "./networkCanvas/runtimeFailures";
import type { RuntimeLifecycleResult } from "./runtimeLifecycleModel";

export interface OwnedDebugTransitionOutcome {
  readonly result: RuntimeLifecycleResult;
  readonly session?: vscode.DebugSession;
}

export interface OwnedDebugTransitionDependencies {
  readonly start: () => Promise<RuntimeLifecycleResult>;
  readonly waitForSession: () => Promise<vscode.DebugSession | undefined>;
  readonly waitForReady: (
    session: vscode.DebugSession
  ) => Promise<RuntimeLifecycleResult>;
  readonly waitForStable: (session: vscode.DebugSession) => Promise<boolean>;
  readonly missingSessionMessage: string;
  readonly unstableSessionMessage: string;
  readonly successMessage: string;
}

export async function runOwnedDebugTransition(
  dependencies: OwnedDebugTransitionDependencies
): Promise<OwnedDebugTransitionOutcome> {
  try {
    const started = await dependencies.start();
    if (!started.ok) {
      return { result: started };
    }
    const session = await dependencies.waitForSession();
    if (!session) {
      return {
        result: {
          ok: false,
          failure: {
            kind: "readiness_timeout",
            message: dependencies.missingSessionMessage,
          },
        },
      };
    }
    const ready = await dependencies.waitForReady(session);
    if (!ready.ok) {
      return { result: ready, session };
    }
    if (!(await dependencies.waitForStable(session))) {
      return {
        result: {
          ok: false,
          failure: {
            kind: "failed_spawn",
            message: dependencies.unstableSessionMessage,
          },
        },
        session,
      };
    }
    return {
      result: { ok: true, message: dependencies.successMessage },
      session,
    };
  } catch (error) {
    return { result: { ok: false, failure: classifyRuntimeStartFailure(error) } };
  }
}
