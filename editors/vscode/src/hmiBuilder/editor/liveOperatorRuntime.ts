// Resolves Live operation from the same selected run target as reads and Deploy (§11).
import * as vscode from "vscode";
import { sendRuntimeControlRequest } from "../../runtimeControlClient";
import { runtimeDeployDeps } from "./deployRuntime";
import { openLiveSocket, requestLive } from "./liveNetwork";
import type {
  LiveGateFacts,
  LiveGateVerdict,
  LiveOperationEvent,
  LiveOperationVerdict,
  LiveOperatorDeps,
} from "./liveOperator";
import type { LiveBridge, LiveEndpoint } from "./liveSession";
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

/** Live mode's operating gate as the language server decides it (`trust-lsp.hmiLiveGate`, §11 item 6). */
export async function liveGate(facts: LiveGateFacts): Promise<LiveGateVerdict> {
  const answer = await vscode.commands.executeCommand<
    { ok?: boolean; verdict?: LiveGateVerdict; error?: string } | undefined
  >("trust-lsp.hmiLiveGate", facts);
  if (answer?.ok !== true || !answer.verdict) {
    throw new Error(answer?.error ?? "trust-lsp.hmiLiveGate gave no verdict");
  }
  return answer.verdict;
}

/** Whether Live mode's operating view ends, and why, as the language server decides it (`trust-lsp.hmiLiveOperation`,
 * §11 item 6). */
export async function liveOperation(event: LiveOperationEvent): Promise<LiveOperationVerdict> {
  const answer = await vscode.commands.executeCommand<
    { ok?: boolean; proceed?: boolean; end?: string | null; error?: string } | undefined
  >("trust-lsp.hmiLiveOperation", event);
  const end = answer?.end ?? null;
  // A verdict is well formed when it says whether to go on, its reason is null or a text, and it goes on or ends.
  const wellFormed =
    answer?.ok === true &&
    typeof answer.proceed === "boolean" &&
    (end === null || (typeof end === "string" && end !== "")) &&
    (answer.proceed || end !== null);
  if (!wellFormed) {
    throw new Error(answer?.error ?? "trust-lsp.hmiLiveOperation gave no verdict");
  }
  return { proceed: answer.proceed === true, end };
}

/** The language server's answer to a bridge event (`trust-lsp.hmiLiveBridge`, §11 item 6). */
async function bridgeAnswer(event: Record<string, unknown>): Promise<Record<string, unknown>> {
  const answer = await vscode.commands.executeCommand<Record<string, unknown> | undefined>(
    "trust-lsp.hmiLiveBridge",
    event,
  );
  if (answer?.ok !== true) {
    throw new Error(typeof answer?.error === "string" ? answer.error : "trust-lsp.hmiLiveBridge gave no verdict");
  }
  return answer;
}

/** What Live mode bridges to the run target's HMI web server and when it keeps the named session, as the language
 * server decides it from facts (`trust-lsp.hmiLiveBridge`, §11 item 6). A verdict of another shape fails closed. */
export const liveBridge: LiveBridge = {
  async request(facts) {
    const answer = await bridgeAnswer({ event: "request", ...facts });
    if (answer.bridged === true && typeof answer.reply === "boolean") return { bridged: true, reply: answer.reply };
    if (
      answer.bridged === false &&
      typeof answer.status === "number" &&
      typeof answer.message === "string" &&
      answer.message !== ""
    ) {
      return { bridged: false, status: answer.status, message: answer.message };
    }
    throw new Error("trust-lsp.hmiLiveBridge gave no verdict on the request");
  },
  async reply(facts) {
    const answer = await bridgeAnswer({ event: "reply", ...facts });
    if (answer.session === "keep" && typeof answer.cookie === "string" && answer.cookie !== "") {
      return { session: "keep", cookie: answer.cookie };
    }
    if (answer.session === "drop" || answer.session === "unchanged") return { session: answer.session };
    throw new Error("trust-lsp.hmiLiveBridge gave no verdict on the answer");
  },
  async stream(facts) {
    const answer = await bridgeAnswer({ event: "stream", ...facts });
    if (typeof answer.bridged !== "boolean") throw new Error("trust-lsp.hmiLiveBridge gave no verdict on the stream");
    return answer.bridged;
  },
};

export function runtimeLiveOperatorDeps(root: string, allowSimulator = false): LiveOperatorDeps {
  let cached: { identity: string; target: LiveEndpoint } | undefined;
  return {
    // The language server decides which of them are HMI changes (§11 item 6).
    unsaved: () => ({
      root,
      unsaved: vscode.workspace.textDocuments
        .filter((document) => document.isDirty && document.uri.scheme === "file")
        .map((document) => document.uri.fsPath),
    }),
    bundle: () => projectBundle(root),
    runTarget: () => runtimeDeployDeps.target(),
    liveGate,
    liveOperation,
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
      bridge: liveBridge,
    },
  };
}
