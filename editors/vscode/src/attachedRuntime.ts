import * as vscode from "vscode";
import { projectRequest } from "./projectRequest";

// The runtime that a debug session attached to, named to the language server for the inline values of the session's
// workspace folder (specification 25 §6.8, `trust-lsp.attachedRuntime`). The extension names the folder and the
// endpoint, which are facts; the language server finds the token of the folder's own runtime itself, so no token
// crosses (specification 14 §7.1.2).

const DEBUG_TYPE = "structured-text";

/** What the language server is told of `session`: its workspace folder and the endpoint it attached to, for an attach
 * session of truST; none for any other session. */
export function attachedRuntimeOf(
  session: Pick<vscode.DebugSession, "type" | "configuration" | "workspaceFolder">
): { readonly folder: string | null; readonly endpoint: string } | undefined {
  const configuration = session.configuration;
  if (session.type !== DEBUG_TYPE || configuration?.request !== "attach" || typeof configuration.endpoint !== "string") {
    return undefined;
  }
  return { folder: session.workspaceFolder?.uri.fsPath ?? null, endpoint: configuration.endpoint };
}

/** Names each attach session's endpoint to the language server when the session starts, and its folder with
 * `endpoint: null` when it ends. */
export function registerAttachedRuntimeForwarding(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.debug.onDidStartDebugSession((session) => {
      const attached = attachedRuntimeOf(session);
      if (attached) {
        void projectRequest("trust-lsp.attachedRuntime", attached);
      }
    }),
    vscode.debug.onDidTerminateDebugSession((session) => {
      const attached = attachedRuntimeOf(session);
      if (attached) {
        void projectRequest("trust-lsp.attachedRuntime", { folder: attached.folder, endpoint: null });
      }
    })
  );
}
