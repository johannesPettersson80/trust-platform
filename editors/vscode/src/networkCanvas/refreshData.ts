import type * as vscode from "vscode";

import type { CommCapabilitiesResponse } from "../communication/capability";
import type { CommSchemaResponse } from "../communication/schemaForm";
import { fetchCommSchema } from "../communication/runtimeComm";
import { listManagedRuntimes } from "../localRuntime";
import type { ManagedRuntime } from "../localRuntimeModel";
import type { RuntimeTarget } from "../runtimeTarget";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import type { CanvasAnswer, CanvasGatheredReports, CanvasRuntimeReport } from "./canvasCommands";
import { fetchConnectorStatus } from "./connectorsStatus";
import { fetchFleetTopology, type FleetTopologyResponse } from "./fleetTopology";
import { offlineCommSchema, offlineCommTopology } from "./offlineComm";

export interface NetworkCanvasRefreshData {
  readonly schema?: CommSchemaResponse;
  readonly capabilities?: CommCapabilitiesResponse;
  /** What the runtimes and the project's configuration answered this round. */
  readonly gathered: CanvasGatheredReports;
  readonly runtimeSetupMessage?: string;
  readonly managed: ManagedRuntime[];
}

/**
 * Gathers one round of answers: the project's configuration, the runtime that serves the live data
 * and every other configured runtime. `trust-lsp.devicesTopology` merges them with the extension's
 * own facts each time the canvas draws (specification 25 §5.6). Nothing here merges, matches or
 * judges a report.
 */
export async function loadNetworkCanvasRefreshData(options: {
  readonly context?: vscode.ExtensionContext;
  readonly projectDir?: string;
  readonly runtime: RuntimeTarget;
  /** The endpoint of the extension's own simulator while it serves the live data. */
  readonly simulatorEndpoint?: string;
  /** Every other configured runtime target, probed. */
  readonly loadPeers: () => Promise<readonly RuntimeTarget[]>;
}): Promise<NetworkCanvasRefreshData> {
  const [offline, live, managed, peers] = await Promise.all([
    loadOfflineData(options.context, options.projectDir),
    loadLiveData(options.runtime),
    options.context ? listManagedRuntimes(options.context) : Promise.resolve([]),
    options.loadPeers().then((targets) => Promise.all(targets.map((target) => runtimeReport(target)))),
  ]);
  const schema = live.schema ?? offline.schema;
  return {
    schema,
    capabilities: live.capabilities,
    gathered: {
      project: offline.topology,
      primary: live.report,
      peers,
      simulator_endpoint: options.simulatorEndpoint,
    },
    runtimeSetupMessage: schema ? undefined : live.schemaError,
    managed,
  };
}

async function loadOfflineData(
  context: vscode.ExtensionContext | undefined,
  projectDir: string | undefined,
): Promise<{
  readonly schema?: CommSchemaResponse;
  readonly topology?: FleetTopologyResponse;
}> {
  if (!context) {
    return {};
  }
  const [schema, topology] = await Promise.all([
    offlineCommSchema(context),
    projectDir
      ? offlineCommTopology(context, projectDir)
      : Promise.resolve(undefined),
  ]);
  return { schema, topology };
}

async function loadLiveData(runtime: RuntimeTarget): Promise<{
  readonly schema?: CommSchemaResponse;
  readonly capabilities?: CommCapabilitiesResponse;
  readonly report?: CanvasRuntimeReport;
  readonly schemaError?: string;
}> {
  if (runtime.status !== "online_reachable" || !runtime.endpoint) {
    return {};
  }
  const [capabilities, schema, report] = await Promise.allSettled([
    sendRuntimeControlRequest<CommCapabilitiesResponse>(
      runtime.endpoint,
      runtime.authToken,
      "comm.capabilities",
      undefined,
      { timeoutMs: 2000 },
    ),
    fetchCommSchema(runtime),
    runtimeReport(runtime),
  ]);
  return {
    capabilities:
      capabilities.status === "fulfilled" ? capabilities.value : undefined,
    schema: schema.status === "fulfilled" ? schema.value : undefined,
    report: report.status === "fulfilled" ? report.value : undefined,
    schemaError:
      schema.status === "rejected" ? errorMessage(schema.reason) : undefined,
  };
}

/** What one target answered: its probe as `runTargets` reported it, and while it answers its topology and connector
 * reports. */
async function runtimeReport(target: RuntimeTarget): Promise<CanvasRuntimeReport> {
  const report = {
    endpoint: target.endpoint ?? "",
    label: target.label,
    status: target.status,
    auth_failure_kind: target.authFailureKind,
  };
  if (target.status !== "online_reachable") {
    return report;
  }
  const [topology, connectors] = await Promise.all([
    answerOf(fetchFleetTopology(target)),
    answerOf(fetchConnectorStatus(target)),
  ]);
  return { ...report, topology, connectors };
}

async function answerOf(request: Promise<unknown>): Promise<CanvasAnswer> {
  try {
    return { ok: (await request) ?? null };
  } catch (error) {
    return { error: errorMessage(error) };
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
