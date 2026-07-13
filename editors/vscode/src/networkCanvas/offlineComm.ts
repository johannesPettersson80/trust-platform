import { execFile } from "child_process";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { getBinaryPath } from "../binary";
import type {
  CommApplyResponse,
  CommSchemaResponse,
} from "../communication/schemaForm";
import type { FleetTopologyResponse } from "./fleetTopology";
import {
  buildOfflineAdsImportArgs,
  buildOfflineBrowseSymbolsArgs,
  classifyAdsBrowseCommandFailure,
  listExistingAdsSnapshotPaths,
} from "./adsBrowseContract";
import { enableRuntimeAdsToml } from "./runtimeAdsToml";

// File-based comm config via the trust-runtime CLI — NO running runtime required. These shell
// out to `trust-runtime comm {schema,topology,apply}` so the canvas can show + edit settings
// on a stopped/offline project (config is just files on disk). Most legacy queries return
// undefined on command/JSON failure; ADS browse preserves a structured failure so an unavailable
// port is never misreported as an empty symbol table.

function runtimeBinary(context: vscode.ExtensionContext): string {
  return getBinaryPath(context, "trust-runtime", "runtime.cli.path");
}

function runJson<T>(
  binary: string,
  args: string[],
  cwd?: string
): Promise<T | undefined> {
  return new Promise((resolve) => {
    execFile(
      binary,
      args,
      { cwd, timeout: 15_000, maxBuffer: 32 * 1024 * 1024 },
      (error, stdout) => {
        if (error) {
          resolve(undefined);
          return;
        }
        try {
          resolve(JSON.parse(stdout) as T);
        } catch {
          resolve(undefined);
        }
      }
    );
  });
}

export interface JsonCommandResult<T> {
  ok: boolean;
  value?: T;
  message?: string;
}

export function runJsonCommand<T>(
  binary: string,
  args: string[],
  cwd?: string,
  cancellationToken?: vscode.CancellationToken
): Promise<JsonCommandResult<T>> {
  return new Promise((resolve) => {
    let settled = false;
    let cancellation: vscode.Disposable | undefined;
    const finish = (result: JsonCommandResult<T>) => {
      if (settled) {
        return;
      }
      settled = true;
      cancellation?.dispose();
      resolve(result);
    };
    const child = execFile(
      binary,
      args,
      { cwd, timeout: 30_000, maxBuffer: 64 * 1024 * 1024 },
      (error, stdout, stderr) => {
        if (error) {
          finish({
            ok: false,
            message:
              stderr.trim() ||
              stdout.trim() ||
              (error instanceof Error ? error.message : String(error)),
          });
          return;
        }
        try {
          finish({ ok: true, value: JSON.parse(stdout) as T });
        } catch (parseError) {
          finish({
            ok: false,
            message:
              parseError instanceof Error
                ? parseError.message
                : String(parseError),
          });
        }
      }
    );
    const cancel = () => {
      child.kill();
      finish({ ok: false, message: "Command cancelled." });
    };
    if (cancellationToken?.isCancellationRequested) {
      cancel();
    } else if (cancellationToken) {
      cancellation = cancellationToken.onCancellationRequested(cancel);
    }
  });
}

function stringField(
  value: Record<string, unknown>,
  ...keys: string[]
): string | undefined {
  for (const key of keys) {
    const raw = value[key];
    if (typeof raw === "string" && raw.trim().length > 0) {
      return raw.trim();
    }
  }
  return undefined;
}

function numberField(value: Record<string, unknown>, key: string): number | undefined {
  const raw = value[key];
  return typeof raw === "number" && Number.isFinite(raw) ? raw : undefined;
}

export function ensureAdsRuntimeEnabled(
  projectDir: string,
  configPath = "ads.toml"
): { ok: true; changed: boolean; runtimeTomlPath: string } | { ok: false; message: string } {
  const runtimeTomlPath = path.join(projectDir, "runtime.toml");
  if (!fs.existsSync(runtimeTomlPath)) {
    return {
      ok: false,
      message: "ADS variable import wrote the selected variables, but runtime.toml is missing so ADS cannot be enabled automatically.",
    };
  }

  const before = fs.readFileSync(runtimeTomlPath, "utf8");
  const after = enableRuntimeAdsToml(before, configPath);

  if (after !== before) {
    fs.writeFileSync(runtimeTomlPath, after, "utf8");
  }
  return { ok: true, changed: after !== before, runtimeTomlPath };
}

export async function openGeneratedAdsDocuments(report: AdsImportSymbolsReport): Promise<void> {
  if (!report.generated_path || !path.isAbsolute(report.generated_path)) {
    return;
  }
  try {
    await vscode.workspace.openTextDocument(vscode.Uri.file(report.generated_path));
  } catch {
    // The import itself succeeded; this only accelerates LSP/index refresh for the generated ST.
  }
}

// Static protocol catalog (no project, no server).
export async function offlineCommSchema(
  context: vscode.ExtensionContext
): Promise<CommSchemaResponse | undefined> {
  return runJson<CommSchemaResponse>(runtimeBinary(context), [
    "comm",
    "schema",
    "--json",
  ]);
}

// Config-derived topology (configured endpoints + non-secret params, status from config).
export async function offlineCommTopology(
  context: vscode.ExtensionContext,
  projectDir: string
): Promise<FleetTopologyResponse | undefined> {
  return runJson<FleetTopologyResponse>(
    runtimeBinary(context),
    ["comm", "topology", "--project", projectDir, "--json"],
    projectDir
  );
}

// Write a driver/service to the project's config files (io.toml / runtime.toml).
export async function offlineCommApply(
  context: vscode.ExtensionContext,
  projectDir: string,
  protocol: string,
  params: Record<string, unknown>,
  action: "add" | "upsert" | "remove" | "disable"
): Promise<CommApplyResponse | undefined> {
  return runJson<CommApplyResponse>(
    runtimeBinary(context),
    [
      "comm",
      "apply",
      "--project",
      projectDir,
      "--protocol",
      protocol,
      "--params",
      JSON.stringify(params),
      "--action",
      action,
      "--json",
    ],
    projectDir
  );
}

export interface FleetRuntimeAddResult {
  name: string;
  path: string;
  control_endpoint: string;
  web_port: number;
}

// §0.5 Browse/Connect: a device/endpoint found on the wire. `params` map onto the setup form.
export interface DiscoverCandidate {
  id: string;
  label: string;
  source: string; // scan | mdns | ads_broadcast | ethercat_bus | opcua_endpoint
  confidence: string; // confirmed | likely | port_reachable | unavailable
  protocol: string;
  originRuntimeId?: string;
  params: Record<string, unknown>;
  warnings?: string[];
}

export interface DiscoverResponse {
  schema_version?: number;
  protocol: string;
  candidates: DiscoverCandidate[];
  warnings?: string[];
}

// §0.5.3 `comm.browse_symbols` — look INSIDE a target: its tags/nodes/channels.
export interface SymbolNode {
  id: string;
  // Raw OPC-UA NodeId (e.g. "ns=2;i=1") — round-trips into comm.apply. The sanitized `id` is for
  // React keys only and is NOT reversible to the NodeId. Present for opcua_client browse leaves.
  node_id?: string;
  name: string;
  path: string;
  type?: string; // raw protocol type (OPC-UA: the DataType NodeId, e.g. "i=11")
  data_type?: string; // resolved/apply-ready type (OPC-UA: e.g. "double")
  size?: number;
  writable?: boolean;
  children?: SymbolNode[];
}

// §0.5.2 a ready-to-run AMS route setup artifact (PowerShell / StaticRoutes.xml / manual steps),
// carried in the route_plan so the canvas can show honest route-setup instructions without credentials.
export interface RouteArtifact {
  kind?: string;
  label: string;
  filename?: string | null;
  content_type?: string;
  content: string;
}

export interface RoutePlan {
  route_name?: string;
  artifacts?: RouteArtifact[];
}

export interface BrowseSymbolsResponse {
  schema_version?: number;
  protocol: string;
  kind?: string;
  // ADS route status on a LIVE browse; `status:"missing"` → offer Route setup (carries route_plan).
  route?: { status?: string; route_plan?: RoutePlan };
  // Structured protocol browse failure. OPC UA and ADS use protocol-specific codes so the canvas
  // can offer honest recovery instead of collapsing every failure into an empty tree.
  error?: { code: string; message: string };
  tree: SymbolNode[];
}

export interface AdsImportSymbolsReport {
  applied?: boolean;
  ads_toml_path: string;
  snapshot_path: string;
  generated_path: string;
  connection_name: string;
  candidate_count: number;
  selected_count: number;
  ads_toml_bytes: number;
  snapshot_bytes: number;
  generated_bytes: number;
  dry_run: boolean;
  lifecycle_effect?: string;
  message?: string;
}

export interface OfflineAdsImportSymbolsResult {
  applied: boolean;
  selected_count?: number;
  candidate_count?: number;
  lifecycle_effect?: "restart_required";
  message: string;
  report?: AdsImportSymbolsReport;
}

function adsSymbolName(selectionKey: string): string {
  return selectionKey.startsWith("ads:symbol:")
    ? selectionKey.slice("ads:symbol:".length)
    : selectionKey;
}

export async function offlineAdsImportSymbols(
  context: vscode.ExtensionContext,
  projectDir: string,
  target: Record<string, unknown>,
  symbols: string[],
  writable: boolean
): Promise<OfflineAdsImportSymbolsResult> {
  const host = stringField(target, "host", "ip");
  if (!host) {
    return {
      applied: false,
      message: "ADS variable import needs a target host.",
    };
  }
  if (writable) {
    return {
      applied: false,
      message:
        "Write-enabled ADS imports need a running runtime so truST can apply the explicit write acknowledgement.",
    };
  }
  const normalizedSymbols = symbols.map(adsSymbolName).filter(Boolean);
  if (normalizedSymbols.length === 0) {
    return {
      applied: false,
      message: "Select at least one ADS symbol to import.",
    };
  }

  const connectionName = stringField(target, "name") ?? "ads_import";
  let existingSnapshotPaths: string[];
  try {
    existingSnapshotPaths = listExistingAdsSnapshotPaths(
      projectDir,
      connectionName,
    );
  } catch (error) {
    return {
      applied: false,
      message: `ADS variable import could not read existing symbol snapshots: ${
        error instanceof Error ? error.message : String(error)
      }`,
    };
  }
  const args = buildOfflineAdsImportArgs(
    projectDir,
    target,
    connectionName,
    normalizedSymbols,
    existingSnapshotPaths,
  );

  const result = await runJsonCommand<AdsImportSymbolsReport>(
    runtimeBinary(context),
    args,
    projectDir
  );
  if (!result.ok || !result.value) {
    return {
      applied: false,
      message: result.message ?? "ADS variable import failed.",
    };
  }
  const runtimeConfig = ensureAdsRuntimeEnabled(projectDir);
  if (!runtimeConfig.ok) {
    return {
      applied: false,
      message: runtimeConfig.message,
      report: result.value,
    };
  }
  await openGeneratedAdsDocuments(result.value);
  return {
    applied: true,
    lifecycle_effect: "restart_required",
    selected_count: result.value.selected_count,
    candidate_count: result.value.candidate_count,
    message: `Added ${result.value.selected_count} ADS variable${
      result.value.selected_count === 1 ? "" : "s"
    }. Restart the runtime to use the generated ST symbols.`,
    report: result.value,
  };
}

export async function offlineBrowseSymbols(
  context: vscode.ExtensionContext,
  protocol: string,
  target: Record<string, unknown>,
  kind: "symbols" | "nodes" | "channels" = "symbols",
  connectionName?: string,
  projectDir?: string,
  cancellationToken?: vscode.CancellationToken
): Promise<BrowseSymbolsResponse | undefined> {
  const args = buildOfflineBrowseSymbolsArgs(
    protocol,
    target,
    kind,
    connectionName,
    projectDir
  );
  const result = await runJsonCommand<BrowseSymbolsResponse>(
    runtimeBinary(context),
    args,
    projectDir,
    cancellationToken
  );
  if (result.ok) {
    return result.value;
  }
  if (protocol !== "ads") {
    return undefined;
  }
  return adsBrowseFailureResponse(result.message);
}

/** Preserve CLI/transport diagnostics in the same versioned contract as a successful ADS browse. */
export function adsBrowseFailureResponse(messageValue?: string): BrowseSymbolsResponse {
  const message = messageValue ?? "ADS symbol browse failed.";
  return {
    schema_version: 1,
    protocol: "ads",
    kind: "symbols",
    tree: [],
    error: {
      code: classifyAdsBrowseCommandFailure(message),
      message,
    },
  };
}

export async function offlineCommDiscover(
  context: vscode.ExtensionContext,
  protocol: string,
  origin: string,
  scope?: {
    cidr?: string;
    host?: string;
    timeoutMs?: number;
    targetAmsNetId?: string;
    amsPort?: number;
  }
): Promise<DiscoverResponse> {
  const args = ["comm", "discover", "--protocol", protocol, "--origin", origin, "--json"];
  if (scope?.cidr) {
    args.push("--cidr", scope.cidr);
  }
  if (scope?.host) {
    args.push("--host", scope.host);
  }
  if (scope?.targetAmsNetId) {
    args.push("--target-net-id", scope.targetAmsNetId);
  }
  if (scope?.amsPort) {
    args.push("--ams-port", String(scope.amsPort));
  }
  if (scope?.timeoutMs) {
    args.push("--timeout-ms", String(scope.timeoutMs));
  }
  const result = await runJsonCommand<DiscoverResponse>(runtimeBinary(context), args);
  if (!result.ok || !result.value) {
    throw new Error(result.message ?? `${protocol} discovery failed.`);
  }
  return result.value;
}

// Scaffold a sibling runtime PROJECT under <fleetRoot> + register it in fleet.toml (offline, no
// running runtime). Returns undefined on failure (e.g. duplicate name, older binary).
export async function offlineFleetRuntimeAdd(
  context: vscode.ExtensionContext,
  fleetRoot: string,
  name: string,
  template: "simulate" | "empty"
): Promise<FleetRuntimeAddResult | undefined> {
  return runJson<FleetRuntimeAddResult>(runtimeBinary(context), [
    "fleet",
    "runtime",
    "add",
    "--fleet-root",
    fleetRoot,
    "--name",
    name,
    "--template",
    template,
    "--json",
  ]);
}
