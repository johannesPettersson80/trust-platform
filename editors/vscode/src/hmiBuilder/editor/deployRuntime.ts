import { remoteVariableContext } from "./remoteVariables";
// The real outside calls of Deploy from the editor (specification 39, §16.2 item 8): the language
// server's bundle check, the selected run target, its control requests, and a JSON POST to its web
// server with the administrator access.

import * as vscode from "vscode";

import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { sendRuntimeControlRequest } from "../../runtimeControlClient";
import type { DeployDeps } from "./deploy";
import { resolveHmiRunTarget } from "./runTarget";

/** How long the runtime may take to check and activate a bundle. */
const DEPLOY_TIMEOUT_MS = 30_000;

export const runtimeDeployDeps: DeployDeps = {
  async bundle(root) {
    try {
      return (
        (await vscode.commands.executeCommand<Awaited<ReturnType<DeployDeps["bundle"]>>>("trust-lsp.hmiBundle", {
          remote: (await remoteVariableContext(root)).catalogue,
          root_uri: vscode.Uri.file(root).toString(),
        })) ?? { ok: false, errors: [{ message: "The language server gave no answer." }] }
      );
    } catch {
      return { ok: false, errors: [{ message: "The truST language server is not running yet." }] };
    }
  },
  async target() {
    const target = await resolveHmiRunTarget(await runtimeLifecycleService.snapshot());
    return target && { ...target, token: await runtimeLifecycleService.controlAuthToken(target.endpoint) };
  },
  control: (endpoint, token, operation, params) =>
    sendRuntimeControlRequest(endpoint, token, operation, params, { timeoutMs: 5_000 }),
  async post(url, token, body) {
    const response = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
        ...(token ? { "X-Trust-Token": token } : {}),
      },
      body: JSON.stringify(body),
      signal: AbortSignal.timeout(DEPLOY_TIMEOUT_MS),
    });
    const answer: unknown = await response.json().catch(() => null);
    return { status: response.status, body: answer };
  },
  async address(controlEndpoint, topology) {
    const answer = await vscode.commands.executeCommand<{ ok?: boolean; url?: string | null } | undefined>(
      "trust-lsp.hmiDeployTarget",
      { controlEndpoint, topology },
    );
    return answer?.ok === true && typeof answer.url === "string" ? answer.url : undefined;
  },
};
