// Resolves Live operation from the same selected run target as reads and Deploy (§11).
import * as path from "path";
import * as vscode from "vscode";
import { ownWebServer, webAddress } from "./deploy";
import { runtimeDeployDeps } from "./deployRuntime";
import { openLiveSocket, requestLive } from "./liveNetwork";
import type { LiveOperatorDeps } from "./liveOperator";
import type { LiveEndpoint } from "./liveSession";

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
    bundle: () => runtimeDeployDeps.bundle(root),
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
        const topology = await runtimeDeployDeps.control(selected.endpoint, selected.token, "fleet.topology", {});
        const web = ownWebServer(topology);
        const url = web && webAddress(selected.endpoint, web.listen, web.tls);
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
