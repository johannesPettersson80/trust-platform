import * as net from "net";
import * as vscode from "vscode";

import { parseControlEndpoint } from "../runtimeControl";
import { localSimControl } from "../simControl";
import { migrateWindowsRuntimeControlProject } from "../windowsRuntimeControlMigration";

const TEST_CONTROL_ENDPOINT_OVERRIDE_ENV = "TRUST_UX_DEBUG_CONTROL_ENDPOINT";
const TEST_CONTROL_AUTH_TOKEN_ENV = "TRUST_UX_DEBUG_CONTROL_AUTH_TOKEN";

export interface LaunchControlPreparation {
  readonly migratedRuntimeToml: boolean;
}

export function prepareLaunchControl(
  config: vscode.DebugConfiguration,
  folder: vscode.WorkspaceFolder | undefined,
  allowTestControlEndpointOverride: boolean
): LaunchControlPreparation {
  const migrationRoot = resolveLaunchMigrationRoot(config, folder);
  const migratedRuntimeToml =
    config.request === "launch" &&
    migrateWindowsRuntimeControlProject(migrationRoot).changed;
  applyLaunchControlEndpoint(
    config,
    folder,
    allowTestControlEndpointOverride
  );
  return { migratedRuntimeToml };
}

export function resolveLaunchMigrationRoot(
  config: vscode.DebugConfiguration,
  folder: vscode.WorkspaceFolder | undefined
): string | undefined {
  return (
    concreteDebugPath(config.runtimeRoot) ??
    folder?.uri.fsPath ??
    concreteDebugPath(config.cwd)
  );
}

export function applyLaunchControlEndpoint(
  config: vscode.DebugConfiguration,
  folder: vscode.WorkspaceFolder | undefined,
  allowTestControlEndpointOverride: boolean
): void {
  if (config.request !== "launch" || config.controlEndpoint) {
    return;
  }
  const testEndpoint = allowTestControlEndpointOverride
    ? (process.env[TEST_CONTROL_ENDPOINT_OVERRIDE_ENV] ?? "").trim()
    : "";
  if (testEndpoint) {
    config.controlEndpoint = testEndpoint;
    const testToken = (process.env[TEST_CONTROL_AUTH_TOKEN_ENV] ?? "").trim();
    if (testToken) {
      config.controlAuthToken = testToken;
    }
    return;
  }
  const sim = localSimControl(folder?.uri.fsPath);
  if (sim) {
    config.controlEndpoint = sim.endpoint;
    config.controlAuthToken = sim.authToken;
  }
}

export async function launchControlEndpointError(
  endpoint: unknown
): Promise<string | undefined> {
  if (typeof endpoint !== "string") {
    return undefined;
  }
  const parsed = parseControlEndpoint(endpoint);
  if (!parsed || parsed.kind !== "tcp") {
    return undefined;
  }
  if (await canConnectToTcpEndpoint(parsed.host, parsed.port)) {
    return "The runtime port is already in use.";
  }
  return undefined;
}

function canConnectToTcpEndpoint(host: string, port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = net.createConnection({ host, port });
    let finished = false;
    const finish = (value: boolean) => {
      if (finished) {
        return;
      }
      finished = true;
      socket.removeAllListeners();
      socket.destroy();
      resolve(value);
    };
    socket.once("connect", () => finish(true));
    socket.once("error", () => finish(false));
    socket.setTimeout(300, () => finish(false));
  });
}

function concreteDebugPath(value: unknown): string | undefined {
  return typeof value === "string" &&
    value.trim().length > 0 &&
    !value.includes("${")
    ? value
    : undefined;
}
