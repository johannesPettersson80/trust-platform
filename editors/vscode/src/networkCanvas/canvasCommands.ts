// The canvas's two requests to the truST language server (specification 25 §5.15):
// `trust-lsp.devicesTopology` merges what the runtimes answered into the topology and the fleet view
// the canvas draws, and `trust-lsp.devicesCanvasAct` performs one action of the canvas. Every rule is
// the server's (`crates/trust-devices/src/canvas`); this module forwards each request through the
// command the language client registers, and shows or applies the server's answer.

import * as vscode from "vscode";

import type { FleetTopologyResponse } from "./fleetTopology";
import type { CanvasFleet } from "./fleetModel";

export const DEVICES_TOPOLOGY_COMMAND = "trust-lsp.devicesTopology";
export const DEVICES_CANVAS_ACT_COMMAND = "trust-lsp.devicesCanvasAct";

/** Shown when the language server does not answer, for example while it starts. */
const SERVER_NOT_RUNNING = "The truST language server is not running yet.";

/** One request's answer as the canvas gathered it: its result, or why it failed. */
export type CanvasAnswer = { readonly ok: unknown } | { readonly error: string };

/** What one runtime target answered this round. */
export interface CanvasRuntimeReport {
  readonly endpoint: string;
  readonly label: string;
  readonly reach: "reachable" | "unreachable" | "auth_missing" | "auth_rejected" | "missing_endpoint";
  readonly topology?: CanvasAnswer;
  readonly connectors?: CanvasAnswer;
}

/** What the canvas gathered from the runtimes this round. */
export interface CanvasGatheredReports {
  readonly project?: unknown;
  readonly primary?: CanvasRuntimeReport;
  readonly peers: readonly CanvasRuntimeReport[];
  readonly simulator_endpoint?: string;
}

/** One round of the canvas's answers and the extension's own facts, which the server merges. */
export interface CanvasTopologyRequest extends CanvasGatheredReports {
  readonly run_mode: string;
  readonly search_query: string;
  readonly discovered: readonly unknown[];
  /** The local simulator as the extension's lifecycle reports it. */
  readonly simulator?: {
    readonly running: boolean;
    readonly runtime_state: string;
    readonly starting: boolean;
    readonly io_values: boolean;
  };
  /** The last failure of a start or a Connect. */
  readonly failure?: {
    readonly kind: string;
    readonly message: string;
    readonly detail?: string;
    readonly endpoint?: string;
  };
  readonly schema?: unknown;
  readonly capabilities?: unknown;
  readonly managed?: ReadonlyArray<{
    readonly name: string;
    readonly control_endpoint: string;
    readonly state: string;
  }>;
  readonly session?: { readonly attached_endpoint?: string; readonly run_target?: string };
  readonly setup?: {
    readonly stage: string;
    readonly active_protocol: string;
    readonly device_requested: boolean;
    readonly apply_result?: unknown;
  };
}

/** The tags already imported from a device, per ADS port. */
export interface AdsPortSelection {
  readonly port: number;
  readonly paths: readonly string[];
}

/** What the merged topology says about one result of the last search. */
export interface DiscoveredAdsFacts {
  readonly id: string;
  readonly on_canvas: boolean;
  readonly imported_ads_symbols: readonly AdsPortSelection[];
  /** Why the result cannot be added now. */
  readonly disabled_reason?: string;
  /** The runtime a found device is added to; none when the runtime that found it is gone. */
  readonly draft_runtime_id?: string;
  /** The host the canvas adds for a found runtime. */
  readonly host?: { readonly endpoint: string; readonly label: string };
}

/** The server's merged topology and fleet view. */
export interface CanvasTopologyAnswer {
  readonly local?: FleetTopologyResponse;
  readonly fleet?: CanvasFleet;
  readonly topology_error?: string;
  readonly discovered: readonly DiscoveredAdsFacts[];
}

/** What the canvas tells the user after an action. */
export interface CanvasNotice {
  readonly tone: "info" | "warning";
  readonly text: string;
  readonly detail?: string;
  readonly modal: boolean;
  readonly actions?: ReadonlyArray<{ readonly id: string; readonly label: string }>;
}

/** The server's reply to one action of the canvas. */
export interface CanvasReply {
  readonly applied: boolean;
  readonly notice?: CanvasNotice;
  readonly refresh: boolean;
  readonly open_documents?: readonly string[];
  readonly result?: unknown;
}

/** The runtime that serves the canvas's live data, while it answers. */
export interface CanvasLiveRuntime {
  readonly endpoint: string;
  readonly token?: string;
}

/** Merges one round of answers; an answer the server could not give is the banner's error. */
export async function requestCanvasTopology(
  request: CanvasTopologyRequest,
): Promise<CanvasTopologyAnswer> {
  const answer = await executeServerCommand(DEVICES_TOPOLOGY_COMMAND, request);
  if (!isRecord(answer) || isRecord(answer.error) || answer.error !== undefined) {
    return { topology_error: serverError(answer), discovered: [] };
  }
  return {
    local: isRecord(answer.local) ? (answer.local as unknown as FleetTopologyResponse) : undefined,
    fleet: isRecord(answer.fleet) ? (answer.fleet as unknown as CanvasFleet) : undefined,
    topology_error: typeof answer.topology_error === "string" ? answer.topology_error : undefined,
    discovered: Array.isArray(answer.discovered)
      ? (answer.discovered as DiscoveredAdsFacts[])
      : [],
  };
}

/** Performs one action of the canvas; a reply the server could not give warns why. */
export async function requestCanvasAct(
  op: string,
  args: Record<string, unknown>,
  context: { readonly project?: string; readonly runtime?: CanvasLiveRuntime } = {},
): Promise<CanvasReply> {
  const answer = await executeServerCommand(DEVICES_CANVAS_ACT_COMMAND, {
    op,
    args,
    ...(context.project ? { project: context.project } : {}),
    ...(context.runtime ? { runtime: context.runtime } : {}),
  });
  if (!isRecord(answer) || answer.error !== undefined || typeof answer.applied !== "boolean") {
    return {
      applied: false,
      refresh: false,
      notice: { tone: "warning", text: serverError(answer), modal: false },
    };
  }
  return answer as unknown as CanvasReply;
}

/**
 * Shows a reply's notice and opens the generated documents it names, so that their diagnostics
 * refresh (specification 25 §5.4). Answers the ID of the notice action the user chose.
 */
export async function presentCanvasReply(reply: CanvasReply): Promise<string | undefined> {
  for (const file of reply.open_documents ?? []) {
    try {
      await vscode.workspace.openTextDocument(vscode.Uri.file(file));
    } catch {
      // The write that produced the document already succeeded; opening it only refreshes faster.
    }
  }
  const notice = reply.notice;
  if (!notice) {
    return undefined;
  }
  const labels = (notice.actions ?? []).map((action) => action.label);
  const options = { modal: notice.modal, detail: notice.detail };
  const chosen =
    notice.tone === "warning"
      ? await vscode.window.showWarningMessage(notice.text, options, ...labels)
      : await vscode.window.showInformationMessage(notice.text, options, ...labels);
  return (notice.actions ?? []).find((action) => action.label === chosen)?.id;
}

async function executeServerCommand(command: string, argument: unknown): Promise<unknown> {
  try {
    return await vscode.commands.executeCommand<unknown>(command, argument);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    // VS Code refuses a command that no client registered: the server has not started or stopped.
    return { error: { message: message.includes("not found") ? SERVER_NOT_RUNNING : message } };
  }
}

function serverError(answer: unknown): string {
  const error = isRecord(answer) ? answer.error : undefined;
  return isRecord(error) && typeof error.message === "string" && error.message.length > 0
    ? error.message
    : SERVER_NOT_RUNNING;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
