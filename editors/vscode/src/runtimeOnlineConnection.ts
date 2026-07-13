import * as vscode from "vscode";

import { DEBUG_TYPE } from "./debug/configuration";
import type { RuntimeStatusPayload } from "./io-panel/types";
import {
  classifyRuntimeStartFailure,
  runtimeStatusCheckFailure,
} from "./networkCanvas/runtimeFailures";
import { getControlAuthToken } from "./runtimeAuth";
import {
  isRuntimeControlAuthError,
  requestRuntimeStatus,
  runtimeControlAuthErrorKind,
} from "./runtimeControlClient";
import {
  MANAGED_RUNTIME_ID_FIELD,
  remoteDebugSessionName,
  runtimeDebugDisabled,
  runtimeNotReachableMessage,
  type RuntimeLifecycleResult,
} from "./runtimeLifecycleModel";
import { runtimeSourceOptionsForTarget } from "./runtimeSourceOptions";
import { getTrustConfiguration } from "./configuration";
import { probeEndpointReachable } from "./io-panel/status";
import { LIFECYCLE_START_ATTEMPT_FIELD } from "./debug/startAttempt";

export interface RuntimeOnlineConnectionOptions {
  readonly configurationTarget?: vscode.Uri;
  readonly configurationScope: vscode.ConfigurationTarget;
  readonly targetLabel?: string;
  readonly lifecycleAttemptId?: string;
  readonly managedRuntimeId?: string;
}

export async function startOnlineRuntimeConnection(
  status: RuntimeStatusPayload,
  options: RuntimeOnlineConnectionOptions,
): Promise<RuntimeLifecycleResult> {
  const config = getTrustConfiguration(options.configurationTarget);
  if (!status.endpointConfigured) {
    return {
      ok: false,
      failure: {
        kind: "failed_spawn",
        message: "Runtime endpoint not set.",
      },
    };
  }

  if (!status.endpointEnabled) {
    await config.update(
      "runtime.controlEndpointEnabled",
      true,
      options.configurationScope,
    );
  }

  const reachable = await probeEndpointReachable(status.endpoint);
  if (!reachable) {
    return {
      ok: false,
      failure: {
        kind: "stale_runtime",
        message: runtimeNotReachableMessage(status.endpoint),
        detail: status.endpoint,
      },
    };
  }

  const authToken = (await getControlAuthToken(status.endpoint)) ?? "";
  let runtimeInfo: unknown;
  try {
    runtimeInfo = await requestRuntimeStatus(
      status.endpoint,
      authToken || undefined,
      { timeoutMs: 1000 },
    );
  } catch (err) {
    if (isRuntimeControlAuthError(err)) {
      const authKind = runtimeControlAuthErrorKind(err);
      return {
        ok: false,
        failure: {
          kind: "workspace_permission",
          message:
            authKind === "missing" || !authToken
              ? "No auth token provided — this runtime requires one."
              : "Auth token rejected — check it and try again.",
        },
      };
    }
    return {
      ok: false,
      failure: runtimeStatusCheckFailure(err),
    };
  }
  if (runtimeDebugDisabled(runtimeInfo)) {
    return {
      ok: false,
      failure: {
        kind: "failed_spawn",
        message:
          "Remote debugging is disabled for this runtime. Open Devices & Connections or ask the runtime owner to enable debugging, then connect again.",
      },
    };
  }

  const folder = options.configurationTarget
    ? (vscode.workspace.getWorkspaceFolder(options.configurationTarget) ??
      vscode.workspace.workspaceFolders?.find(
        (candidate) =>
          candidate.uri.toString() === options.configurationTarget?.toString(),
      ))
    : vscode.workspace.workspaceFolders?.[0];
  const debugConfig: vscode.DebugConfiguration = {
    type: DEBUG_TYPE,
    request: "attach",
    name: remoteDebugSessionName(options.targetLabel, status.endpoint),
    endpoint: status.endpoint,
    authToken: authToken || undefined,
    targetLabel: options.targetLabel,
    internalConsoleOptions: "neverOpen",
    ...runtimeSourceOptionsForTarget(),
  };
  if (options.lifecycleAttemptId) {
    debugConfig[LIFECYCLE_START_ATTEMPT_FIELD] = options.lifecycleAttemptId;
  }
  if (options.managedRuntimeId?.trim()) {
    debugConfig[MANAGED_RUNTIME_ID_FIELD] = options.managedRuntimeId.trim();
  }
  if (folder) {
    debugConfig.cwd = folder.uri.fsPath;
  }
  try {
    const started = await vscode.debug.startDebugging(folder, debugConfig);
    if (!started) {
      throw new Error("Attach failed to start.");
    }
    return { ok: true, message: "Attached to runtime." };
  } catch (err) {
    return { ok: false, failure: classifyRuntimeStartFailure(err) };
  }
}
