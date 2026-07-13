import * as net from "net";
import * as vscode from "vscode";

import { parseControlEndpoint } from "../runtimeControl";
import { localSimControl } from "../simControl";
import {
  migrateWindowsRuntimeControlProject,
  type RuntimeControlProjectMigrationFailure,
} from "../windowsRuntimeControlMigration";

const TEST_CONTROL_ENDPOINT_OVERRIDE_ENV = "TRUST_UX_DEBUG_CONTROL_ENDPOINT";
const TEST_CONTROL_AUTH_TOKEN_ENV = "TRUST_UX_DEBUG_CONTROL_AUTH_TOKEN";

export interface LaunchControlPreparation {
  readonly migratedRuntimeToml: boolean;
  readonly failure?: RuntimeControlProjectMigrationFailure;
}

export function prepareLaunchControl(
  config: vscode.DebugConfiguration,
  folder: vscode.WorkspaceFolder | undefined,
  allowTestControlEndpointOverride: boolean,
  platform: NodeJS.Platform = process.platform
): LaunchControlPreparation {
  const migrationRoot = resolveLaunchMigrationRoot(config, folder);
  const migration =
    config.request === "launch"
      ? migrateWindowsRuntimeControlProject(migrationRoot, platform)
      : { changed: false };
  const launchFailure = applyLaunchControlEndpoint(
    config,
    folder,
    allowTestControlEndpointOverride
  );
  return {
    migratedRuntimeToml: migration.changed,
    ...(migration.failure || launchFailure
      ? { failure: migration.failure ?? launchFailure }
      : {}),
  };
}

export function launchControlPreparationError(
  failure: RuntimeControlProjectMigrationFailure
): Error & { readonly runtimeFailure: RuntimeControlProjectMigrationFailure } {
  return Object.assign(new Error(failure.message), {
    name: "RuntimeControlConfigurationError",
    runtimeFailure: failure,
  });
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
): RuntimeControlProjectMigrationFailure | undefined {
  if (config.request !== "launch") {
    return undefined;
  }
  const testEndpoint = allowTestControlEndpointOverride
    ? (process.env[TEST_CONTROL_ENDPOINT_OVERRIDE_ENV] ?? "").trim()
    : "";
  if (!normalizedString(config.controlEndpoint) && testEndpoint) {
    config.controlEndpoint = testEndpoint;
    const testToken = (process.env[TEST_CONTROL_AUTH_TOKEN_ENV] ?? "").trim();
    if (testToken) {
      config.controlAuthToken = testToken;
    }
  }
  const sim = localSimControl(folder?.uri.fsPath);
  if (!normalizedString(config.controlEndpoint) && sim) {
    config.controlEndpoint = sim.endpoint;
    config.controlAuthToken = sim.authToken;
    return undefined;
  }
  if (normalizedString(config.controlAuthToken)) {
    return undefined;
  }
  const endpoint = normalizedString(config.controlEndpoint);
  if (endpoint && localLaunchControlEndpoint(endpoint) && sim) {
    // Preserve an explicitly chosen same-computer endpoint, but never create a
    // TCP debug control server without a credential.
    config.controlAuthToken = sim.authToken;
    return undefined;
  }
  return {
    kind: "configuration",
    code: "runtime_control_auth_requires_manual_configuration",
    message:
      "The Simulator control endpoint requires authentication. Configure a strong control token before starting the Simulator.",
  };
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

function normalizedString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function localLaunchControlEndpoint(endpoint: string): boolean {
  const parsed = parseControlEndpoint(endpoint);
  if (!parsed || parsed.kind === "unix") {
    return parsed?.kind === "unix";
  }
  const host = parsed.host.toLowerCase();
  if (host === "localhost" || host === "::1") {
    return true;
  }
  const octets = host.split(".");
  return (
    octets.length === 4 &&
    octets[0] === "127" &&
    octets.every((part) => /^\d{1,3}$/.test(part) && Number(part) <= 255)
  );
}
