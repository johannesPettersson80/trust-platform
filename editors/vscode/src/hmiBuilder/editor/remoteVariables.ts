import * as vscode from "vscode";
import type { LiveSessionDeps } from "./liveSession";
import type { VariableNode } from "./messages";
const connections = new Map<string, LiveSessionDeps>();
export function registerRemoteVariables(root: vscode.Uri, deps: LiveSessionDeps | undefined): void {
  if (deps) connections.set(root.toString(), deps);
}
/** Read only the selected runtime's fresh publish list, scoped to this open project. */
export async function remoteVariableContext(
  project: vscode.Uri | string,
): Promise<{ catalogue: Record<string, unknown>; variables: VariableNode[] }> {
  const root = typeof project === "string" ? vscode.Uri.file(project) : project;
  if (!vscode.workspace.workspaceFolders?.some((folder) => folder.uri.toString() === root.toString()))
    return { catalogue: {}, variables: [] };
  try {
    const connection =
      connections.get(root.toString()) ??
      (await import("./liveOperatorRuntime")).runtimeLiveOperatorDeps(root.fsPath).session;
    const target = await connection.target();
    if (!target) return { catalogue: {}, variables: [] };
    let reply = await connection.request(target, "GET", "/api/hmi/remote", undefined);
    let body = reply.body as { catalogue?: Record<string, VariableNode>; pending?: boolean };
    const deadline = Date.now() + 2500;
    while (reply.status === 200 && body.pending && Date.now() < deadline) {
      await new Promise((resolve) => setTimeout(resolve, 250));
      const current = await connection.target();
      if (current?.endpoint !== target.endpoint || current?.url !== target.url) return { catalogue: {}, variables: [] };
      reply = await connection.request(target, "GET", "/api/hmi/remote", undefined);
      body = reply.body as typeof body;
    }
    if (reply.status !== 200 || !body?.catalogue) return { catalogue: {}, variables: [] };
    return {
      catalogue: body.catalogue,
      variables: Object.values(body.catalogue).map((node) => ({ ...node, name: node.path })),
    };
  } catch {
    return { catalogue: {}, variables: [] };
  }
}
