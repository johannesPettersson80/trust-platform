// Resolves Live operation from the same selected run target as reads and Deploy (§11).
import * as path from "path";
import * as vscode from "vscode";
import { sendRuntimeControlRequest } from "../../runtimeControlClient";
import { runtimeDeployDeps } from "./deployRuntime";
import { openLiveSocket, requestLive } from "./liveNetwork";
import type { LiveOperatorDeps } from "./liveOperator";
import type { LiveEndpoint } from "./liveSession";
import { remoteVariableContext } from "./remoteVariables";

/** The project's bundle as the editor check builds it (`trust-lsp.hmiBundle`, §16.4 item 2). */
async function projectBundle(root: string): Promise<{ ok?: boolean; version?: string }> {
  try {
    return (
      (await vscode.commands.executeCommand<{ ok?: boolean; version?: string }>("trust-lsp.hmiBundle", {
        remote: (await remoteVariableContext(root)).catalogue,
        root_uri: vscode.Uri.file(root).toString(),
      })) ?? { ok: false }
    );
  } catch {
    return { ok: false };
  }
}

/** The web server's address for the runtime at `controlEndpoint` and its `fleet.topology` answer, as
 * `trust-lsp.hmiDeployTarget` gives it; undefined when it reports none. */
export async function webServerAddress(controlEndpoint: string, topology: unknown): Promise<string | undefined> {
  const answer = await vscode.commands.executeCommand<{ ok?: boolean; url?: string | null } | undefined>(
    "trust-lsp.hmiDeployTarget",
    { controlEndpoint, topology },
  );
  return answer?.ok === true && typeof answer.url === "string" ? answer.url : undefined;
}

export function runtimeLiveOperatorDeps(root: string, allowSimulator = false): LiveOperatorDeps {
  let cached: { identity: string; target: LiveEndpoint } | undefined;
  return {
    dirty: () =>
      vscode.workspace.textDocuments.some(
        (document) =>
          document.isDirty &&
          document.uri.scheme === "file" &&
          document.uri.fsPath.startsWith(path.join(root, "hmi") + path.sep),
      ),
    bundle: () => projectBundle(root),
    runTarget: () => runtimeDeployDeps.target(),
    session: {
      async target() {
        const selected = await runtimeDeployDeps.target();
        if (!selected || (selected.simulator && !allowSimulator)) {
          cached = undefined;
          return undefined;
        }
        const identity = JSON.stringify([selected.endpoint, selected.token, selected.simulator]);
        if (cached?.identity === identity) return { ...cached.target, label: selected.label };
        const topology = await sendRuntimeControlRequest(
          selected.endpoint,
          selected.token,
          "fleet.topology",
          {},
          {
            timeoutMs: 5_000,
          },
        );
        const url = await webServerAddress(selected.endpoint, topology);
        if (!url) return undefined;
        const target = {
          url,
          simulator: selected.simulator,
          endpoint: selected.endpoint,
          token: selected.token,
          label: selected.label,
        };
        cached = { identity, target };
        return target;
      },
      request: requestLive,
      openSocket: openLiveSocket,
    },
  };
}
