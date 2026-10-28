import * as vscode from "vscode";

import { getBinaryPath } from "./binary";
import { getTrustConfiguration } from "./configuration";
import { projectRequest } from "./projectRequest";
import {
  resolveProjectControlEndpoint,
  type ProjectCommandRunner,
} from "./projectControlEndpoint";
import { getControlAuthToken } from "./runtimeAuth";

// Spec 11 §6.8.0a: a surface that talks to the workspace's runtime asks
// `trust-runtime ctl --project <dir> endpoint --json` for the endpoint and its token at each
// connection attempt, so it follows a runtime that restarted on another port with another token
// (the Windows default). An endpoint named in the `runtime.controlEndpoint` setting is used as it
// is, with the token stored for it. This module only chooses between the two and runs the command;
// trust-runtime resolves the endpoint.

export interface RuntimeConnection {
  readonly endpoint?: string;
  readonly authToken?: string;
  /** trust-runtime's reason when the workspace runtime has no endpoint now, such as not_running. */
  readonly problem?: { readonly code: string; readonly message: string };
}

export interface RuntimeConnectionDeps {
  /** The trust-runtime binary; the extension's own when omitted. */
  readonly bin?: string;
  /** Runs trust-runtime; a child process when omitted. */
  readonly run?: ProjectCommandRunner;
}

let runtimeBinary: (() => string) | undefined;

export function registerRuntimeConnection(context: vscode.ExtensionContext): void {
  runtimeBinary = () => getBinaryPath(context, "trust-runtime", "runtime.cli.path");
}

/** The runtime project of a workspace folder, as the language server finds it (`trust-lsp.projectRoot`): the folder
 * with runtime.toml, or its bundle folder. */
export async function workspaceProjectDir(folder?: vscode.WorkspaceFolder): Promise<string | undefined> {
  if (!folder || folder.uri.scheme !== "file") {
    return undefined;
  }
  const answer = await projectRequest<{ runtimeProject: string | null }>("trust-lsp.projectRoot", {
    runtimeFolder: folder.uri.fsPath,
  });
  return answer.ok ? answer.runtimeProject ?? undefined : undefined;
}

/** The endpoint and token for the workspace's runtime now. */
export async function resolveRuntimeConnection(
  resource?: vscode.Uri
): Promise<RuntimeConnection> {
  const configured = getTrustConfiguration(resource)
    .get<string>("runtime.controlEndpoint", "")
    .trim();
  const folder =
    (resource ? vscode.workspace.getWorkspaceFolder(resource) : undefined) ??
    vscode.workspace.workspaceFolders?.[0];
  return connectionFor(configured, await workspaceProjectDir(folder));
}

/** The connection for the endpoint named in the settings, if any, else for the project folder,
 * asked of trust-runtime on every call. */
export async function connectionFor(
  configured: string,
  projectDir: string | undefined,
  deps: RuntimeConnectionDeps = {}
): Promise<RuntimeConnection> {
  if (configured) {
    return { endpoint: configured, authToken: await getControlAuthToken(configured) };
  }
  const bin = deps.bin ?? runtimeBinary?.();
  if (!projectDir || !bin) {
    return {};
  }
  const resolved = await resolveProjectControlEndpoint(bin, projectDir, deps.run);
  if (!resolved.ok) {
    return { problem: { code: resolved.code, message: resolved.message } };
  }
  return {
    endpoint: resolved.value.endpoint,
    authToken:
      resolved.value.authToken ?? (await getControlAuthToken(resolved.value.endpoint)),
  };
}
