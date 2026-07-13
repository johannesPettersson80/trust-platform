import type * as vscode from "vscode";

import { getControlAuthToken } from "../runtimeAuth";
import {
  resolveRuntimeTargetFromSettings,
  type RuntimeTarget,
} from "../runtimeTarget";
import { networkCanvasTrustConfig } from "./networkCanvasWorkspace";

export async function resolveNetworkCanvasFleetTargets(
  primary: RuntimeTarget,
  workspaceResource: vscode.Uri | undefined,
  endpointLabels: ReadonlyMap<string, string>
): Promise<RuntimeTarget[]> {
  const extra = networkCanvasTrustConfig(workspaceResource).get<string[]>(
    "runtime.fleetEndpoints",
    []
  );
  const endpoints = [
    ...new Set(
      (extra ?? [])
        .map((endpoint) => endpoint.trim())
        .filter(
          (endpoint) => endpoint.length > 0 && endpoint !== primary.endpoint
        )
    ),
  ];
  if (endpoints.length === 0) {
    return [primary];
  }
  const peers = await Promise.all(
    endpoints.map(async (endpoint) =>
      resolveRuntimeTargetFromSettings({
        mode: "online",
        endpoint,
        authToken: await getControlAuthToken(endpoint),
        endpointEnabled: true,
        label: endpointLabels.get(endpoint),
      }).catch(() => undefined)
    )
  );
  return [
    primary,
    ...peers.filter((peer): peer is RuntimeTarget => peer !== undefined),
  ];
}
