import * as vscode from "vscode";

import { sendRuntimeControlRequest } from "../runtimeControlClient";
import type { RuntimeTarget } from "../runtimeTarget";
import {
  OPEN_RUN_ACTION,
  adsImportFailurePrompt,
} from "./adsImportUx";
import { buildExposeApplyParams } from "./exposeConfig";
import { classifyAdsBrowseCommandFailure } from "./adsBrowseContract";
import type { FleetTopologyResponse } from "./fleetTopology";
import {
  ensureAdsRuntimeEnabled,
  offlineAdsImportSymbols,
  offlineBrowseSymbols,
  offlineCommApply,
  openGeneratedAdsDocuments,
  type AdsImportSymbolsReport,
  type BrowseSymbolsResponse,
} from "./offlineComm";

export interface ProtocolActionDependencies {
  readonly panel: () => vscode.WebviewPanel | undefined;
  readonly extensionContext: () => vscode.ExtensionContext | undefined;
  readonly topology: () => FleetTopologyResponse | undefined;
  readonly runtimeTarget: () => RuntimeTarget | undefined;
  readonly runtimeTargetForOrigin: (
    originId: string,
    leaseId: string | undefined,
    browseSessionId: string | undefined
  ) => RuntimeTarget | undefined;
  readonly refresh: () => Promise<void>;
}

/** Owns protocol-specific browse/import mutations outside the panel lifecycle shell. */
export class NetworkCanvasProtocolActions {
  constructor(private readonly dependencies: ProtocolActionDependencies) {}

  async browseSymbols(message: Record<string, unknown>): Promise<void> {
    const panel = this.dependencies.panel();
    const context = this.dependencies.extensionContext();
    if (!panel || !context) {
      return;
    }
    const browseSessionId =
      typeof message.browseSessionId === "string" &&
      message.browseSessionId.length > 0
        ? message.browseSessionId
        : undefined;
    const browseRequestId =
      typeof message.browseRequestId === "number" &&
      Number.isSafeInteger(message.browseRequestId) &&
      message.browseRequestId >= 0
        ? message.browseRequestId
        : undefined;
    if (!browseSessionId || browseRequestId === undefined) {
      return;
    }
    const protocol =
      typeof message.protocol === "string" ? message.protocol : "ads";
    const target = isRecord(message.target) ? message.target : {};
    const commandTarget = withoutBrowseUiMetadata(target);
    const kind =
      message.kind === "channels" || message.kind === "nodes"
        ? message.kind
        : "symbols";
    const connectionName =
      typeof commandTarget.name === "string" ? commandTarget.name : undefined;
    const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    const discoveryOriginId =
      typeof target.discovery_origin_runtime_id === "string"
        ? target.discovery_origin_runtime_id
        : undefined;
    const discoveryOriginLeaseId =
      typeof target.discovery_origin_lease_id === "string"
        ? target.discovery_origin_lease_id
        : undefined;
    const runtime = discoveryOriginId
      ? this.dependencies.runtimeTargetForOrigin(
          discoveryOriginId,
          discoveryOriginLeaseId,
          browseSessionId
        )
      : undefined;
    const viaRuntime =
      Boolean(discoveryOriginId) &&
      runtime?.status === "online_reachable" &&
      Boolean(runtime.endpoint);
    let result: BrowseSymbolsResponse | undefined;
    if (discoveryOriginId && !viaRuntime) {
      result = {
        protocol,
        tree: [],
        error: {
          code: "discovery_origin_unreachable",
          message:
            "The selected discovery runtime is no longer reachable. Reconnect it and discover ADS devices again.",
        },
      };
    } else if (viaRuntime && runtime?.endpoint) {
      try {
        result = await sendRuntimeControlRequest<BrowseSymbolsResponse>(
          runtime.endpoint,
          runtime.authToken,
          "comm.browse_symbols",
          {
            protocol,
            target: commandTarget,
            kind,
            connection_name: connectionName,
          },
          { timeoutMs: 20_000 }
        );
      } catch (error) {
        const detail = error instanceof Error ? error.message : String(error);
        result = {
          protocol,
          tree: [],
          error: {
            code: classifyAdsBrowseCommandFailure(detail),
            message: detail,
          },
        };
      }
    } else {
      result = await offlineBrowseSymbols(
        context,
        protocol,
        commandTarget,
        kind,
        connectionName,
        projectDir
      );
    }
    if (this.dependencies.panel() !== panel || !panel.visible) {
      return;
    }
    void panel.webview.postMessage({
      type: "symbolTree",
      browseSessionId,
      browseRequestId,
      tree: result?.tree ?? [],
      routeMissing: result?.route?.status === "missing",
      routePlan: result?.route?.route_plan,
      error: result?.error,
    });
  }

  async addExpose(message: Record<string, unknown>): Promise<void> {
    const protocol =
      typeof message.protocol === "string" ? message.protocol : "";
    const paths = stringPaths(message.paths);
    const allowWrites = Boolean(message.writable);
    const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    const context = this.dependencies.extensionContext();
    if (!context || !projectDir || !protocol || paths.length === 0) {
      return;
    }
    const current = this.findEndpointParams(protocol);
    if (!current) {
      await vscode.window.showWarningMessage(
        `Configure the ${protocolDisplayName(protocol)} first, then choose globals to expose.`
      );
      return;
    }
    const { names, params } = buildExposeApplyParams(
      current,
      paths,
      allowWrites
    );
    const result = await offlineCommApply(
      context,
      projectDir,
      protocol,
      params,
      "upsert"
    );
    if (result?.applied) {
      const restart =
        result.lifecycle_effect === "restart_required" ? " Restart to apply." : "";
      await vscode.window.showInformationMessage(
        `${protocolDisplayName(protocol)}: exposed ${countLabel(names.length, "global")}.${restart}`
      );
      await this.dependencies.refresh();
    } else {
      const errors = result?.field_errors
        ?.map((error) => error.message)
        .join("; ");
      await vscode.window.showWarningMessage(
        `Could not expose globals: ${
          errors ??
          result?.message ??
          "edit the server config in the inspector first."
        }`
      );
    }
  }

  async addOpcuaConnection(message: Record<string, unknown>): Promise<void> {
    const connection = isRecord(message.connection)
      ? message.connection
      : undefined;
    const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    const context = this.dependencies.extensionContext();
    if (!context || !projectDir || !connection) {
      return;
    }
    const points = Array.isArray(connection.points)
      ? connection.points.length
      : 0;
    const result = await offlineCommApply(
      context,
      projectDir,
      "opcua_client",
      { enabled: true, connections: [connection] },
      "add"
    );
    if (result?.applied) {
      await vscode.window.showInformationMessage(
        `Added OPC UA client connection with ${points} node(s).${
          result.lifecycle_effect === "restart_required"
            ? " Restart the runtime to read it."
            : ""
        }`
      );
      await this.dependencies.refresh();
    } else {
      const errors = result?.field_errors
        ?.map((error) => error.message)
        .join("; ");
      await vscode.window.showWarningMessage(
        `Could not save the OPC UA client connection: ${
          errors ?? result?.message ?? "check the endpoint and try again."
        }`
      );
    }
  }

  async addEthercatChannels(message: Record<string, unknown>): Promise<void> {
    const paths = stringPaths(message.paths, true);
    const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    const context = this.dependencies.extensionContext();
    if (!context || !projectDir || paths.length === 0) {
      return;
    }
    const target = isRecord(message.target) ? message.target : {};
    const current =
      Object.keys(target).length > 0
        ? target
        : (this.findEndpointParams("ethercat") ?? {});
    if (Object.keys(current).length === 0) {
      await vscode.window.showWarningMessage(
        "Configure EtherCAT modules first, then browse channels."
      );
      return;
    }
    const selectedChannels = Array.from(
      new Set(paths.map((path) => path.trim()))
    ).sort();
    const result = await offlineCommApply(
      context,
      projectDir,
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
      await this.dependencies.refresh();
    } else {
      const errors = result?.field_errors
        ?.map((error) => error.message)
        .join("; ");
      await vscode.window.showWarningMessage(
        `Could not save EtherCAT channels: ${
          errors ??
          result?.message ??
          "check the configured modules and try again."
        }`
      );
    }
  }

  async addTags(message: Record<string, unknown>): Promise<void> {
    const paths = stringPaths(message.paths);
    if (paths.length === 0) {
      return;
    }
    const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    if (!projectDir) {
      await vscode.window.showWarningMessage(
        "Add variables needs an open project so truST can write ads.toml and generated ST."
      );
      return;
    }
    const source = isRecord(message.target) ? message.target : {};
    const discoveryOriginId =
      typeof source.discovery_origin_runtime_id === "string"
        ? source.discovery_origin_runtime_id
        : undefined;
    const commandMessage: Record<string, unknown> = {
      ...message,
      target: withoutBrowseUiMetadata(source),
    };
    if (discoveryOriginId) {
      await vscode.window.showWarningMessage(
        "Remote discovery is read-only in this release. Browse variables through the selected runtime, then add them from a project running on that same computer."
      );
      return;
    }
    const runtime = this.dependencies.runtimeTarget();
    if (
      !runtime ||
      runtime.status !== "online_reachable" ||
      !runtime.endpoint
    ) {
      await this.addTagsOffline(commandMessage, projectDir, paths);
      return;
    }
    await this.addTagsLive(commandMessage, projectDir, paths, runtime);
  }

  private async addTagsOffline(
    message: Record<string, unknown>,
    projectDir: string,
    paths: string[]
  ): Promise<void> {
    const protocol =
      typeof message.protocol === "string" ? message.protocol : "";
    const context = this.dependencies.extensionContext();
    if (protocol !== "ads" || !context) {
      await vscode.window.showWarningMessage(
        "Add variables needs a reachable runtime — it writes ads.toml + the generated ST through the runtime's ADS import pipeline."
      );
      return;
    }
    const source = isRecord(message.target) ? message.target : {};
    const report = await offlineAdsImportSymbols(
      context,
      projectDir,
      source,
      paths,
      Boolean(message.writable)
    );
    if (report.applied) {
      await vscode.window.showInformationMessage(
        `Added ${countLabel(
          report.selected_count ?? paths.length,
          "ADS variable",
        )}. Restart the Simulator, then view the imported variables in Live Values → ADS.`,
      );
      await this.dependencies.refresh();
      return;
    }
    const prompt = adsImportFailurePrompt(report.message);
    console.error(`[truST ADS import] ${report.message}`);
    const selected = await vscode.window.showWarningMessage(
      prompt.message,
      { modal: prompt.modal, detail: prompt.detail },
      ...prompt.actions
    );
    if (selected === OPEN_RUN_ACTION) {
      await vscode.commands.executeCommand("trust.home.focus");
    }
  }

  private async addTagsLive(
    message: Record<string, unknown>,
    projectDir: string,
    paths: string[],
    runtime: RuntimeTarget
  ): Promise<void> {
    const endpoint = runtime.endpoint;
    if (!endpoint) {
      return;
    }
    const source = isRecord(message.target) ? message.target : {};
    const target: Record<string, unknown> = {
      name: typeof source.name === "string" ? source.name : undefined,
      ip: typeof source.host === "string" ? source.host : source.ip,
      ams_net_id:
        typeof source.target_net_id === "string"
          ? source.target_net_id
          : source.ams_net_id,
      ams_port: typeof source.ams_port === "number" ? source.ams_port : 851,
      tc_version: source.tc_version,
    };
    const connectionName =
      typeof source.name === "string" && source.name.trim().length > 0
        ? source.name
        : "ads_import";
    try {
      const report = await sendRuntimeControlRequest<AdsImportSymbolsReport>(
        endpoint,
        runtime.authToken,
        "ads.import_symbols.apply",
        {
          connection_name: connectionName,
          symbols: paths,
          target,
          write_acknowledged: Boolean(message.writable),
        },
        { timeoutMs: 20_000 }
      );
      if (!report?.applied) {
        const raw = report?.message ?? "The runtime rejected the import.";
        console.error(`[truST ADS import] ${raw}`);
        const prompt = adsImportFailurePrompt(raw);
        await vscode.window.showWarningMessage(
          prompt.message,
          { modal: prompt.modal, detail: prompt.detail },
          ...prompt.actions
        );
        return;
      }
      const runtimeConfig = ensureAdsRuntimeEnabled(projectDir);
      if (!runtimeConfig.ok) {
        await vscode.window.showWarningMessage(
          `Added ADS variables, but ADS runtime was not enabled automatically: ${runtimeConfig.message}`
        );
        await this.dependencies.refresh();
        return;
      }
      await openGeneratedAdsDocuments(report);
      await vscode.window.showInformationMessage(
        `Added ${countLabel(
          report.selected_count ?? paths.length,
          "ADS variable"
        )}. Restart the Simulator, then view the imported variables in Live Values → ADS.`
      );
      await this.dependencies.refresh();
    } catch (error) {
      const raw = error instanceof Error ? error.message : String(error);
      console.error(`[truST ADS import] ${raw}`);
      const prompt = adsImportFailurePrompt(raw);
      await vscode.window.showWarningMessage(
        prompt.message,
        { modal: prompt.modal, detail: prompt.detail },
        ...prompt.actions
      );
    }
  }

  private findEndpointParams(
    protocol: string
  ): Record<string, unknown> | undefined {
    for (const host of this.dependencies.topology()?.hosts ?? []) {
      const runtimes = [
        ...(host.runtimes ?? []),
        ...(host.containers ?? []).flatMap(
          (container) => container.runtimes ?? []
        ),
      ];
      for (const runtime of runtimes) {
        for (const endpoint of runtime.endpoints ?? []) {
          if (endpoint.protocol === protocol && isRecord(endpoint.params)) {
            return endpoint.params;
          }
        }
      }
    }
    return undefined;
  }
}

function protocolDisplayName(protocol: string): string {
  switch (protocol) {
    case "ads":
      return "Read from ADS";
    case "ads_server":
      return "Share over ADS";
    case "opcua":
      return "OPC UA server";
    case "opcua_client":
      return "OPC UA client";
    case "modbus_tcp":
      return "Modbus TCP";
    default:
      return protocol.replace(/_/g, " ");
  }
}

function countLabel(
  count: number,
  singular: string,
  plural = `${singular}s`
): string {
  return `${count} ${count === 1 ? singular : plural}`;
}

function stringPaths(value: unknown, nonEmpty = false): string[] {
  return Array.isArray(value)
    ? value.filter(
        (path): path is string =>
          typeof path === "string" && (!nonEmpty || path.trim().length > 0)
      )
    : [];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function withoutBrowseUiMetadata(
  target: Record<string, unknown>
): Record<string, unknown> {
  const clean = { ...target };
  delete clean.ads_port_confirmed;
  delete clean.discovery_origin_runtime_id;
  delete clean.discovery_origin_lease_id;
  return clean;
}
