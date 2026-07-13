import type * as vscode from "vscode";

import {
  resolveRuntimeTarget,
  resolveRuntimeTargetFromSettings,
  type RuntimeTarget,
} from "../runtimeTarget";
import { simulatorControlFromDebugConfiguration } from "../simControl";

/** Resolve the canvas control target without falling away from an accepted simulator session. */
export async function resolveNetworkCanvasRuntimeTarget(
  workspaceResource: vscode.Uri | undefined,
  debugConfiguration: vscode.DebugConfiguration | undefined,
): Promise<RuntimeTarget> {
  const simulatorControl =
    simulatorControlFromDebugConfiguration(debugConfiguration);
  if (!simulatorControl) {
    return resolveRuntimeTarget(workspaceResource);
  }
  return resolveRuntimeTargetFromSettings({
    mode: "online",
    endpoint: simulatorControl.endpoint,
    authToken: simulatorControl.authToken,
    endpointEnabled: true,
    label: "Simulator",
  });
}
