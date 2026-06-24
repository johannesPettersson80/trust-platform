import { execFile } from "child_process";
import * as vscode from "vscode";

import { getBinaryPath } from "../binary";
import type {
  CommApplyResponse,
  CommSchemaResponse,
} from "../communication/schemaForm";
import type { FleetTopologyResponse } from "./fleetTopology";

// File-based comm config via the trust-runtime CLI — NO running runtime required. These shell
// out to `trust-runtime comm {schema,topology,apply}` so the canvas can show + edit settings
// on a stopped/offline project (config is just files on disk). Every call returns undefined on
// ANY failure (missing subcommand on an older binary, bad JSON, spawn error) so callers fall
// back to the live-control path and the UI never breaks before the CLIs ship.

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
  action: "add" | "upsert" | "remove"
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
  confidence: string; // observed | configured | manual
  protocol: string;
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
  artifacts?: RouteArtifact[];
}

export interface BrowseSymbolsResponse {
  schema_version?: number;
  protocol: string;
  // ADS route status on a LIVE browse; `status:"missing"` → offer "Create route" (carries route_plan).
  route?: { status?: string; route_plan?: RoutePlan };
  // Structured browse failure (opcua_client): code ∈ cert_untrusted | auth_required |
  // endpoint_unreachable | browse_denied | unsupported_security_profile. Drives the recovery action.
  error?: { code: string; message: string };
  tree: SymbolNode[];
}

export async function offlineBrowseSymbols(
  context: vscode.ExtensionContext,
  protocol: string,
  target: Record<string, unknown>,
  kind: "symbols" | "nodes" | "channels" = "symbols",
  connectionName?: string,
  projectDir?: string
): Promise<BrowseSymbolsResponse | undefined> {
  const args = ["comm", "browse-symbols", "--protocol", protocol, "--kind", kind, "--json"];
  // Local expose (truST's own globals) + EtherCAT channels read from project files offline.
  if (projectDir) {
    args.push("--project", projectDir);
  }
  // A remote target (ADS) carries connection params; local/project browses pass none.
  if (target && Object.keys(target).length > 0) {
    args.push("--target", JSON.stringify(target));
  }
  if (connectionName) {
    args.push("--connection-name", connectionName);
  }
  return runJson<BrowseSymbolsResponse>(runtimeBinary(context), args, projectDir);
}

// §0.5.3 `comm.discover` — find devices/endpoints. `origin` = where the scan runs (this_host =
// the dev machine via CLI; runtime = the runtime's network/hardware). Returns undefined on any
// failure (older binary without the verb, spawn error) so the UI degrades gracefully.
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
