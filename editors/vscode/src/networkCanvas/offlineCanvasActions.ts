import * as vscode from "vscode";

import type { FleetTopologyResponse } from "./fleetTopology";
import { buildExposeApplyParams } from "./exposeConfig";
import { countLabel, protocolDisplayName } from "./canvasLabels";
import { offlineBrowseSymbols, offlineCommApply } from "./offlineComm";

export interface OfflineCanvasActionContext {
  extensionContext: vscode.ExtensionContext | undefined;
  projectDir: string | undefined;
  topology: FleetTopologyResponse | undefined;
  postMessage(message: unknown): void;
  refresh(): Promise<void>;
}

/** Owns project-file browse and endpoint edits; live runtime mutation remains a separate owner. */
export async function browseOfflineSymbols(
  context: OfflineCanvasActionContext,
  message: Record<string, unknown>
): Promise<void> {
  if (!context.extensionContext) return;
  const protocol = typeof message.protocol === "string" ? message.protocol : "ads";
  const target = isRecord(message.target) ? message.target : {};
  const kind = message.kind === "channels" || message.kind === "nodes" ? message.kind : "symbols";
  const connectionName = typeof target.name === "string" ? target.name : undefined;
  const result = await offlineBrowseSymbols(
    context.extensionContext,
    protocol,
    target,
    kind,
    connectionName,
    context.projectDir
  );
  context.postMessage({
    type: "symbolTree",
    tree: result?.tree ?? [],
    routeMissing: result?.route?.status === "missing",
    routePlan: result?.route?.route_plan,
    error: result?.error,
  });
}

export async function addOfflineExpose(
  context: OfflineCanvasActionContext,
  message: Record<string, unknown>
): Promise<void> {
  const protocol = typeof message.protocol === "string" ? message.protocol : "";
  const paths = Array.isArray(message.paths)
    ? message.paths.filter((path): path is string => typeof path === "string")
    : [];
  if (!context.extensionContext || !context.projectDir || !protocol || paths.length === 0) return;
  const current = findEndpointParams(context.topology, protocol);
  if (!current) {
    await vscode.window.showWarningMessage(
      `Configure the ${protocolDisplayName(protocol)} first, then choose globals to expose.`
    );
    return;
  }
  const { names, params } = buildExposeApplyParams(current, paths, Boolean(message.writable));
  const result = await offlineCommApply(
    context.extensionContext,
    context.projectDir,
    protocol,
    params,
    "upsert"
  );
  if (result?.applied) {
    const restart = result.lifecycle_effect === "restart_required" ? " Restart to apply." : "";
    await vscode.window.showInformationMessage(
      `${protocolDisplayName(protocol)}: exposed ${countLabel(names.length, "global")}.${restart}`
    );
    await context.refresh();
  } else {
    const errors = result?.field_errors?.map((error) => error.message).join("; ");
    await vscode.window.showWarningMessage(
      `Could not expose globals: ${errors ?? result?.message ?? "edit the server config in the inspector first."}`
    );
  }
}

export async function addOfflineOpcuaConnection(
  context: OfflineCanvasActionContext,
  message: Record<string, unknown>
): Promise<void> {
  const connection = message.connection as Record<string, unknown> | undefined;
  if (!context.extensionContext || !context.projectDir || !connection) return;
  const points = Array.isArray(connection.points) ? connection.points.length : 0;
  const result = await offlineCommApply(
    context.extensionContext,
    context.projectDir,
    "opcua_client",
    { enabled: true, connections: [connection] },
    "add"
  );
  if (result?.applied) {
    await vscode.window.showInformationMessage(
      `Added OPC UA client connection with ${points} node(s).${
        result.lifecycle_effect === "restart_required" ? " Restart the runtime to read it." : ""
      }`
    );
    await context.refresh();
  } else {
    const errors = result?.field_errors?.map((error) => error.message).join("; ");
    await vscode.window.showWarningMessage(
      `Could not save the OPC UA client connection: ${errors ?? result?.message ?? "check the endpoint and try again."}`
    );
  }
}

export async function addOfflineEthercatChannels(
  context: OfflineCanvasActionContext,
  message: Record<string, unknown>
): Promise<void> {
  const paths = Array.isArray(message.paths)
    ? message.paths.filter((path): path is string => typeof path === "string" && path.trim().length > 0)
    : [];
  if (!context.extensionContext || !context.projectDir || paths.length === 0) return;
  const target = isRecord(message.target) ? message.target : {};
  const current = Object.keys(target).length > 0
    ? target
    : (findEndpointParams(context.topology, "ethercat") ?? {});
  if (Object.keys(current).length === 0) {
    await vscode.window.showWarningMessage("Configure EtherCAT modules first, then browse channels.");
    return;
  }
  const selectedChannels = Array.from(new Set(paths.map((path) => path.trim()))).sort();
  const result = await offlineCommApply(
    context.extensionContext,
    context.projectDir,
    "ethercat",
    { ...current, selected_channels: selectedChannels },
    "upsert"
  );
  if (result?.applied) {
    await vscode.window.showInformationMessage(
      `${countLabel(selectedChannels.length, "EtherCAT channel")} selected.${
        result.lifecycle_effect === "restart_required" ? " Restart to apply." : ""
      }`
    );
    await context.refresh();
  } else {
    const errors = result?.field_errors?.map((error) => error.message).join("; ");
    await vscode.window.showWarningMessage(
      `Could not save EtherCAT channels: ${errors ?? result?.message ?? "check the configured modules and try again."}`
    );
  }
}

function findEndpointParams(
  topology: FleetTopologyResponse | undefined,
  protocol: string
): Record<string, unknown> | undefined {
  for (const host of topology?.hosts ?? []) {
    const runtimes = [
      ...(host.runtimes ?? []),
      ...(host.containers ?? []).flatMap((container) => container.runtimes ?? []),
    ];
    for (const runtime of runtimes) {
      for (const endpoint of runtime.endpoints ?? []) {
        if (endpoint.protocol === protocol && isRecord(endpoint.params)) return endpoint.params;
      }
    }
  }
  return undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
