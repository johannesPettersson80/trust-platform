import * as vscode from "vscode";

import type { CheckProgramResponse } from "./checkProgramModel";
import type { LocalSimulatorDebugStart } from "./localSimulatorDebugStart";
import {
  prepareLocalSimulatorProject,
  type LocalSimulatorPreparationResult,
} from "./localSimulatorPreparation";
import {
  runOwnedDebugTransition,
  type OwnedDebugTransitionOutcome,
} from "./runtimeDebugTransition";
import {
  waitForLifecycleSession,
  waitForSessionStable,
} from "./runtimeSessionReadiness";
import {
  DEBUG_START_COMMAND_TIMEOUT_MS,
  SESSION_WAIT_TIMEOUT_MS,
  withTimeout,
  type RuntimeLifecycleResult,
} from "./runtimeLifecycleModel";

export type LocalSimulatorProjectValidator = (
  projectRoot: vscode.Uri | undefined,
) => Promise<CheckProgramResponse | undefined>;

type FailedLocalSimulatorPreparationResult = Extract<
  LocalSimulatorPreparationResult,
  { readonly ok: false }
>;

export type LocalSimulatorStartCoordination =
  | { readonly kind: "cancelled"; readonly result: RuntimeLifecycleResult }
  | {
      readonly kind: "preparation";
      readonly result: FailedLocalSimulatorPreparationResult;
    }
  | {
      readonly kind: "transition";
      readonly outcome: OwnedDebugTransitionOutcome;
    };

export interface LocalSimulatorStartCoordinatorDependencies {
  readonly attemptId: string;
  readonly projectRoot: vscode.Uri | undefined;
  readonly validateProject?: LocalSimulatorProjectValidator;
  readonly isAttemptActive: (attemptId: string) => boolean;
  readonly setRuntimeMode: (
    mode: "simulate",
    target: vscode.Uri | undefined,
  ) => Promise<void>;
  readonly executeDebugStart: LocalSimulatorDebugStart;
  readonly sessionForAttempt: (
    attemptId: string,
  ) => vscode.DebugSession | undefined;
  readonly waitForReady: (
    session: vscode.DebugSession,
    timeoutMs: number,
  ) => Promise<RuntimeLifecycleResult>;
  readonly hasSession: (key: string) => boolean;
  readonly sessionStabilityMs: number;
}

/**
 * Owns the complete local Simulator preparation-to-DAP transition. Lifecycle
 * state remains committed by RuntimeLifecycleService after this coordinator
 * returns, keeping state authority separate from project/DAP orchestration.
 */
export async function coordinateLocalSimulatorStart(
  dependencies: LocalSimulatorStartCoordinatorDependencies,
): Promise<LocalSimulatorStartCoordination> {
  const initialSave = dependencies.validateProject
    ? await saveDirtyProjectDocuments(dependencies.projectRoot)
    : { ok: true as const };
  if (!initialSave.ok) {
    return preparationFailure(initialSave.message);
  }

  let validationFailure: string | undefined;
  const preparation = await prepareLocalSimulatorProject(
    dependencies.projectRoot?.fsPath,
    {
      validateProject: async () => {
        if (!dependencies.validateProject) {
          return undefined;
        }
        const projectValidation = await dependencies.validateProject(
          dependencies.projectRoot,
        );
        if (!projectValidation) {
          validationFailure =
            "Simulator Compile did not return a validation report. Check the Structured Text Debugger output, then start again.";
          return undefined;
        }
        return projectValidation;
      },
    },
  );

  if (!dependencies.isAttemptActive(dependencies.attemptId)) {
    return {
      kind: "cancelled",
      result: {
        ok: false,
        failure: {
          kind: "stale_runtime",
          message: "Simulator start was cancelled before launch.",
        },
      },
    };
  }
  if (validationFailure) {
    return preparationFailure(validationFailure);
  }
  if (!preparation.ok) {
    return { kind: "preparation", result: preparation };
  }

  const outcome = await runOwnedDebugTransition({
    start: async () => {
      await dependencies.setRuntimeMode("simulate", dependencies.projectRoot);
      const started = await withTimeout(
        dependencies.executeDebugStart(
          dependencies.attemptId,
          dependencies.projectRoot,
        ),
        DEBUG_START_COMMAND_TIMEOUT_MS,
        "Start debugging timed out. Check the runtime port or target settings.",
      );
      return started
        ? { ok: true, message: "Simulator debug session launched." }
        : {
            ok: false,
            failure: {
              kind: "stale_runtime" as const,
              message:
                "Simulator could not start. The logs show what blocked startup.",
            },
          };
    },
    waitForSession: () =>
      waitForLifecycleSession(
        () => dependencies.sessionForAttempt(dependencies.attemptId),
        SESSION_WAIT_TIMEOUT_MS,
      ),
    waitForReady: (session) =>
      dependencies.waitForReady(session, SESSION_WAIT_TIMEOUT_MS),
    waitForStable: (session) =>
      waitForSessionStable(
        session,
        (key) => dependencies.hasSession(key),
        dependencies.sessionStabilityMs,
      ),
    missingSessionMessage: "Timed out waiting for the Simulator debug session.",
    unstableSessionMessage:
      "Simulator stopped during startup. Check the runtime port or target settings.",
    successMessage: "Simulator running.",
  });
  return { kind: "transition", outcome };
}

async function saveDirtyProjectDocuments(
  projectRoot: vscode.Uri | undefined,
): Promise<{ readonly ok: true } | { readonly ok: false; readonly message: string }> {
  if (!projectRoot) {
    return { ok: true };
  }
  const projectKey = projectRoot.toString();
  for (const document of vscode.workspace.textDocuments) {
    if (!document.isDirty || document.uri.scheme !== "file") {
      continue;
    }
    const folder = vscode.workspace.getWorkspaceFolder(document.uri);
    if (folder?.uri.toString() !== projectKey) {
      continue;
    }
    if (!(await document.save())) {
      return {
        ok: false,
        message: `Save ${vscode.workspace.asRelativePath(document.uri)} before starting the Simulator.`,
      };
    }
  }
  return { ok: true };
}

function preparationFailure(message: string): LocalSimulatorStartCoordination {
  return {
    kind: "preparation",
    result: {
      ok: false,
      failure: { kind: "failed_spawn", message },
    },
  };
}
