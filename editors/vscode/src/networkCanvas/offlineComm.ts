import { execFile } from "child_process";
import * as vscode from "vscode";

import { getBinaryPath } from "../binary";
import type { CommSchemaResponse } from "../communication/schemaForm";
import type { FleetTopologyResponse } from "./fleetTopology";

// The trust-runtime CLI as the canvas's transport while no runtime runs: `comm schema`,
// `comm topology`, `comm discover`, `comm browse-symbols` and `fleet runtime add` answer from
// this computer and the project's files. A query that fails answers undefined, except a browse,
// whose failure the browse pane shows. Every write of the project's files and every rule is
// `trust-lsp.devicesCanvasAct`'s (canvasCommands.ts).

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
  // can offer honest recovery instead of collapsing every failure into an empty tree; a command
  // that did not answer carries only what it printed.
  error?: { code?: string; message?: string };
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
  const args = [
    "comm",
    "browse-symbols",
    "--protocol",
    protocol,
    "--kind",
    kind,
    "--json",
  ];
  if (projectDir) {
    args.push("--project", projectDir);
  }
  if (Object.keys(target).length > 0) {
    args.push("--target", JSON.stringify(target));
  }
  if (connectionName) {
    args.push("--connection-name", connectionName);
  }
  const result = await runJsonCommand<BrowseSymbolsResponse>(
    runtimeBinary(context),
    args,
    projectDir
  );
  if (result.ok) {
    return result.value;
  }
  // The browse answers its own protocol errors with an empty tree (specification 11); a command
  // that did not answer at all is shown with what it printed.
  return { protocol, tree: [], error: { message: result.message } };
}

export async function offlineCommDiscover(
  context: vscode.ExtensionContext,
  protocol: string,
  origin: string,
  scope?: { cidr?: string; host?: string; timeoutMs?: number; adsPorts?: readonly unknown[] }
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
  for (const port of scope?.adsPorts ?? []) {
    args.push("--ads-port", String(port));
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
