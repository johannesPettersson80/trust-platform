import * as vscode from "vscode";

import type { RuntimeTarget } from "../runtimeTarget";
import type { FleetTopologyResponse } from "./fleetTopology";
import {
  presentCanvasReply,
  requestCanvasAct,
  type CanvasLiveRuntime,
  type CanvasReply,
} from "./canvasCommands";
import { offlineBrowseSymbols } from "./offlineComm";

export interface ProtocolActionDependencies {
  readonly panel: () => vscode.WebviewPanel | undefined;
  readonly extensionContext: () => vscode.ExtensionContext | undefined;
  readonly topology: () => FleetTopologyResponse | undefined;
  readonly runtimeTarget: () => RuntimeTarget | undefined;
  readonly refresh: () => Promise<void>;
  readonly startRuntime: () => Promise<void>;
}

/** The notice action that starts the project's runtime. */
const START_RUNTIME = "start_runtime";

/**
 * The browse panes' actions. Each one forwards the pane's choice to `trust-lsp.devicesCanvasAct`,
 * which plans the connection, imports or removes the tags, and writes the project's files or asks
 * the runtime that serves the live data (specification 25 §5.4 and §5.8); the canvas shows the
 * server's answer.
 */
export class NetworkCanvasProtocolActions {
  constructor(private readonly dependencies: ProtocolActionDependencies) {}

  async addAdsDevice(message: Record<string, unknown>): Promise<void> {
    await this.perform("ads_add_device", {
      target: record(message.target),
      label: typeof message.label === "string" ? message.label : "",
    });
  }

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
    const target = record(message.target);
    const kind =
      message.kind === "channels" || message.kind === "nodes"
        ? message.kind
        : "symbols";
    const connectionName =
      typeof target.name === "string" ? target.name : undefined;
    const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    const result = await offlineBrowseSymbols(
      context,
      protocol,
      target,
      kind,
      connectionName,
      projectDir
    );
    // The recovery that a failed browse offers is the server's (specification 25 §5.4 and §11.2).
    const errorView = result?.error
      ? (await requestCanvasAct("browse_error", { protocol, error: result.error })).result
      : undefined;
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
      errorView,
    });
  }

  async addExpose(message: Record<string, unknown>): Promise<void> {
    await this.perform("expose", {
      protocol: typeof message.protocol === "string" ? message.protocol : "",
      topology: this.dependencies.topology() ?? {},
      paths: Array.isArray(message.paths) ? message.paths : [],
      writable: Boolean(message.writable),
    });
  }

  async addOpcuaConnection(message: Record<string, unknown>): Promise<void> {
    await this.perform("opcua_add", {
      target: record(message.target),
      label: typeof message.label === "string" ? message.label : "",
      nodes: Array.isArray(message.nodes) ? message.nodes : [],
      writable: Boolean(message.writable),
    });
  }

  async addEthercatChannels(message: Record<string, unknown>): Promise<void> {
    await this.perform("ethercat_channels", {
      target: record(message.target),
      topology: this.dependencies.topology() ?? {},
      paths: Array.isArray(message.paths) ? message.paths : [],
    });
  }

  async addTags(message: Record<string, unknown>): Promise<void> {
    await this.perform(
      "ads_tags_import",
      {
        protocol: typeof message.protocol === "string" ? message.protocol : "",
        target: record(message.target),
        paths: Array.isArray(message.paths) ? message.paths : [],
        writable: Boolean(message.writable),
      },
      true
    );
  }

  async addAdsTagsBatch(message: Record<string, unknown>): Promise<void> {
    await this.tagsBatch(message, "ads_tags_add", {
      target: record(message.target),
      selections: Array.isArray(message.selections) ? message.selections : [],
      writable: Boolean(message.writable),
      changed_path: typeof message.changedPath === "string" ? message.changedPath : undefined,
    });
  }

  async removeAdsTag(message: Record<string, unknown>): Promise<void> {
    await this.tagsBatch(message, "ads_tag_remove", {
      target: record(message.target),
      selection: record(message.selection),
    });
  }

  /** The ADS port that the text typed into a browse pane names, or why it names none. */
  async checkAdsPort(message: Record<string, unknown>): Promise<void> {
    const panel = this.dependencies.panel();
    const text = typeof message.text === "string" ? message.text : "";
    if (!panel || typeof message.requestId !== "number") {
      return;
    }
    const reply = await requestCanvasAct("ads_port", { text });
    const result = record(reply.result);
    if (this.dependencies.panel() === panel && panel.visible) {
      void panel.webview.postMessage({
        type: "adsPortChecked",
        requestId: message.requestId,
        text,
        port: typeof result.port === "number" ? result.port : undefined,
        error:
          typeof result.error === "string"
            ? result.error
            : reply.notice?.text,
      });
    }
  }

  /** A tag browser's add or removal, whose result goes back to the pane that asked for it. */
  private async tagsBatch(
    message: Record<string, unknown>,
    op: string,
    args: Record<string, unknown>
  ): Promise<void> {
    const panel = this.dependencies.panel();
    const browseSessionId =
      typeof message.browseSessionId === "string"
        ? message.browseSessionId
        : undefined;
    const adsImportRequestId =
      typeof message.adsImportRequestId === "number" &&
      Number.isSafeInteger(message.adsImportRequestId)
        ? message.adsImportRequestId
        : undefined;
    if (!panel || !browseSessionId || adsImportRequestId === undefined) {
      return;
    }
    const reply = await requestCanvasAct(op, args, this.context());
    await presentCanvasReply(reply);
    if (reply.refresh) {
      await this.dependencies.refresh();
    }
    if (reply.result !== undefined && this.dependencies.panel() === panel && panel.visible) {
      void panel.webview.postMessage({
        type: "adsTagsResult",
        browseSessionId,
        adsImportRequestId,
        result: reply.result,
      });
    }
  }

  /** One action: the server's answer is shown, and its notice may offer to start the runtime. */
  private async perform(
    op: string,
    args: Record<string, unknown>,
    live = false
  ): Promise<CanvasReply> {
    const reply = await requestCanvasAct(op, args, this.context(live));
    const chosen = await presentCanvasReply(reply);
    if (reply.refresh) {
      await this.dependencies.refresh();
    }
    if (chosen === START_RUNTIME) {
      await this.dependencies.startRuntime();
    }
    return reply;
  }

  /** The open project, and the runtime that serves the live data while it answers. */
  private context(live = true): { project?: string; runtime?: CanvasLiveRuntime } {
    const runtime = this.dependencies.runtimeTarget();
    return {
      project: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
      runtime:
        live && runtime?.status === "online_reachable" && runtime.endpoint
          ? { endpoint: runtime.endpoint, token: runtime.authToken }
          : undefined,
    };
  }
}

function record(value: unknown): Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}
