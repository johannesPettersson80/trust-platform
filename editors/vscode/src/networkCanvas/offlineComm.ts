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
} from "./adsBrowseContract";
import { adsConnectionNameForTarget } from "./adsDiscoveryPorts";

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

interface JsonCommandResult<T> {
  ok: boolean;
  value?: T;
  message?: string;
}

function runJsonCommand<T>(
  binary: string,
  args: string[],
  cwd?: string
): Promise<JsonCommandResult<T>> {
  return new Promise((resolve) => {
    execFile(
      binary,
      args,
      { cwd, timeout: 30_000, maxBuffer: 64 * 1024 * 1024 },
      (error, stdout, stderr) => {
        if (error) {
          resolve({
            ok: false,
            message:
              stderr.trim() ||
              stdout.trim() ||
              (error instanceof Error ? error.message : String(error)),
          });
          return;
        }
        try {
          resolve({ ok: true, value: JSON.parse(stdout) as T });
        } catch (parseError) {
          resolve({
            ok: false,
            message:
              parseError instanceof Error
                ? parseError.message
                : String(parseError),
          });
        }
      }
    );
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

export async function openGeneratedAdsDocuments(report: AdsImportSymbolsReport): Promise<void> {
  if (!report.generated_path || !path.isAbsolute(report.generated_path)) {
    return;
  }
  await openGeneratedDocument(report.generated_path);
}

/**
 * Opens the generated document at `file` so diagnostics refresh. A document that cannot be opened
 * is ignored: the write that produced it already succeeded.
 */
async function openGeneratedDocument(file: string): Promise<void> {
  try {
    await vscode.workspace.openTextDocument(vscode.Uri.file(file));
  } catch {
    // The write itself succeeded; opening the document only accelerates LSP/index refresh.
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
// carried in the route_plan so the canvas can show "Create route" without handling any credentials.
export interface RouteArtifact {
  kind?: string;
  label: string;
  filename?: string | null;
  content_type?: string;
  content: string;
}

export interface RoutePlan {
  route_name?: string;
  target?: {
    ip?: string;
    ams_net_id?: string;
    ams_port?: number;
  };
  local?: {
    chosen_ip?: string;
    ams_net_id?: string;
  };
  artifacts?: RouteArtifact[];
}

export interface BrowseSymbolsResponse {
  schema_version?: number;
  protocol: string;
  // ADS route status on a LIVE browse; `status:"missing"` → offer "Create route" (carries route_plan).
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

export interface OfflineAdsTagRemovalResult {
  applied: boolean;
  removed_count: number;
  restart_required: boolean;
  message: string;
}

/** The report that `trust-runtime ads remove-symbols` prints (specification 11 §6.12). */
export interface AdsTagRemovalReport {
  schema_version: number;
  applied: boolean;
  matched: boolean;
  removed_count: number;
  restart_required: boolean;
  files_written: string[];
  message: string;
}

/** The generated Structured Text that an import or a removal rewrites. */
const GENERATED_ADS_ST = "src/generated/ads_generated.st";

/** The `trust-runtime ads remove-symbols` arguments for `paths` of one ADS port. */
export function buildOfflineAdsRemoveArgs(
  projectDir: string,
  target: Record<string, unknown>,
  port: number,
  paths: readonly string[],
): string[] {
  const args = [
    "ads",
    "remove-symbols",
    "--project",
    projectDir,
    "--ams-port",
    String(port),
  ];
  const host = stringField(target, "host", "ip");
  if (host) {
    args.push("--host", host);
  }
  const targetNetId = stringField(target, "target_net_id", "ams_net_id");
  if (targetNetId) {
    args.push("--target-net-id", targetNetId);
  }
  for (const path of paths) {
    args.push("--path", path);
  }
  args.push("--json");
  return args;
}

/** The panel's result of that report: the Rust command's own message, count and restart need. */
export function adsTagRemovalResult(
  report: AdsTagRemovalReport,
): OfflineAdsTagRemovalResult {
  return {
    applied: report.applied,
    removed_count: report.removed_count,
    restart_required: report.restart_required,
    message: report.message,
  };
}

/**
 * Remove one imported ADS tag through the Rust `trust-runtime ads remove-symbols` command, which
 * rewrites `ads.toml`, regenerates the generated ST from every cached snapshot and writes both;
 * the extension host no longer edits an ADS file itself (design §4.3 B7; specification 25 §5.4).
 */
export async function offlineAdsRemoveTag(
  context: vscode.ExtensionContext,
  projectDir: string,
  target: Record<string, unknown>,
  port: number,
  symbolPath: string,
): Promise<OfflineAdsTagRemovalResult> {
  const result = await runJsonCommand<AdsTagRemovalReport>(
    runtimeBinary(context),
    buildOfflineAdsRemoveArgs(projectDir, target, port, [symbolPath]),
    projectDir,
  );
  if (!result.ok || !result.value) {
    return {
      applied: false,
      removed_count: 0,
      restart_required: false,
      message: result.message ?? "ADS tag removal failed.",
    };
  }
  const removal = adsTagRemovalResult(result.value);
  if (result.value.files_written.includes(GENERATED_ADS_ST)) {
    await openGeneratedDocument(path.join(projectDir, GENERATED_ADS_ST));
  }
  return removal;
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
  writable: boolean,
  connectionNameOverride?: string,
): Promise<OfflineAdsImportSymbolsResult> {
  const host = stringField(target, "host", "ip");
  if (!host) {
    return {
      applied: false,
      message: "ADS tag import needs a target host.",
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

  const connectionName = connectionNameOverride ??
    adsConnectionNameForTarget(target, "ads_import");
  const existingSnapshots = existingAdsSnapshotPaths(projectDir);
  const args = buildOfflineAdsImportArgs(
    projectDir,
    target,
    connectionName,
    normalizedSymbols,
    existingSnapshots,
  );

  const result = await runJsonCommand<AdsImportSymbolsReport>(
    runtimeBinary(context),
    args,
    projectDir
  );
  if (!result.ok || !result.value) {
    return {
      applied: false,
      message: result.message ?? "ADS tag import failed.",
    };
  }
  // The Rust import enables `[runtime.ads]` itself (specification 11 §6.12), so the extension host
  // no longer edits `runtime.toml`.
  await openGeneratedAdsDocuments(result.value);
  return {
    applied: true,
    lifecycle_effect: "restart_required",
    selected_count: result.value.selected_count,
    candidate_count: result.value.candidate_count,
    message: `Added ${result.value.selected_count} ADS tag${
      result.value.selected_count === 1 ? "" : "s"
    }. Restart the runtime to use the generated ST symbols.`,
    report: result.value,
  };
}

export function existingAdsSnapshotPaths(projectDir: string): string[] {
  const directory = path.join(projectDir, "ads", "snapshots");
  try {
    return fs
      .readdirSync(directory, { withFileTypes: true })
      .filter((entry) => entry.isFile() && entry.name.endsWith(".symbols.json"))
      .map((entry) => path.join(directory, entry.name))
      .sort();
  } catch {
    return [];
  }
}

export async function offlineBrowseSymbols(
  context: vscode.ExtensionContext,
  protocol: string,
  target: Record<string, unknown>,
  kind: "symbols" | "nodes" | "channels" = "symbols",
  connectionName?: string,
  projectDir?: string
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
    projectDir
  );
  if (result.ok) {
    return result.value;
  }
  if (protocol !== "ads") {
    return undefined;
  }
  const message = result.message ?? "ADS symbol browse failed.";
  return {
    protocol,
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
  scope?: { cidr?: string; host?: string; timeoutMs?: number }
): Promise<DiscoverResponse | undefined> {
  const args = ["comm", "discover", "--protocol", protocol, "--origin", origin, "--json"];
  if (scope?.cidr) {
    args.push("--cidr", scope.cidr);
  }
  if (scope?.host) {
    args.push("--host", scope.host);
  }
  if (scope?.timeoutMs) {
    args.push("--timeout-ms", String(scope.timeoutMs));
  }
  return runJson<DiscoverResponse>(runtimeBinary(context), args);
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
