import { registerSimulatorSessions } from "./simulator";
import * as fs from "fs/promises";
import * as path from "path";
import { randomUUID } from "crypto";
import * as vscode from "vscode";
import type { LspClientProvider } from "../../lm-tools/shared";
import { getBinaryPath } from "../../binary";
import { HmiAgentTool, workspaceIdentity } from "./tools";
import { startAgentBridge, AgentBridge, BridgeTool } from "./bridge";

export const CONNECT_AGENT_COMMAND = "trust-lsp.hmiBuilder.connectAgent";
/** Explicitly creates a private connection for one workspace in this editor window. */
export function registerAgentConnection(context: vscode.ExtensionContext, getClient?: LspClientProvider): void {
  registerSimulatorSessions(context);
  const connections = new Map<string, { bridge: AgentBridge; connectionFile: string }>();
  const stop = async (id: string): Promise<void> => {
    const connection = connections.get(id);
    if (!connection) return;
    connections.delete(id);
    await connection.bridge.close();
    await fs.rm(connection.connectionFile, { force: true });
  };
  context.subscriptions.push({
    dispose: () => {
      for (const id of connections.keys()) void stop(id);
    },
  });
  context.subscriptions.push(
    vscode.workspace.onDidChangeWorkspaceFolders((event) => {
      for (const folder of event.removed) void stop(workspaceIdentity(folder.uri));
    }),
  );
  context.subscriptions.push(
    vscode.commands.registerCommand(
      CONNECT_AGENT_COMMAND,
      async (args?: { workspaceId?: string; reveal?: boolean }) => {
        const folders = vscode.workspace.workspaceFolders ?? [];
        let folder = args?.workspaceId
          ? folders.find((item) => workspaceIdentity(item.uri) === args.workspaceId)
          : folders.length === 1
            ? folders[0]
            : undefined;
        if (!folder && !args?.workspaceId && folders.length > 1) {
          folder = (
            await vscode.window.showQuickPick(
              folders.map((item) => ({ label: item.name, description: item.uri.toString(), folder: item })),
              { title: "Connect an agent to an HMI project" },
            )
          )?.folder;
        }
        if (!folder) throw new Error("Select an open workspace before connecting an agent.");
        if (folder.uri.scheme !== "file" || folder.uri.authority)
          throw new Error("This workspace URI is not supported by the local authoring bridge.");
        const workspaceId = workspaceIdentity(folder.uri);
        let connection = connections.get(workspaceId);
        if (!connection) {
          const manifest = context.extension.packageJSON as {
            contributes: { languageModelTools: Array<{ name: string; modelDescription: string; inputSchema: object }> };
          };
          const tools: BridgeTool[] = manifest.contributes.languageModelTools
            .filter((tool) => tool.name.startsWith("hmi_"))
            .map((tool) => ({
              name: tool.name,
              description: tool.modelDescription,
              inputSchema: tool.inputSchema,
              // The MCP server annotates each tool (specification 43 §3.1).
            }));
          const bridge = await startAgentBridge({
            workspaceId,
            tools,
            invoke: async (name, input, signal) => {
              const cancel = new vscode.CancellationTokenSource();
              const abort = (): void => cancel.cancel();
              signal.addEventListener("abort", abort, { once: true });
              if (signal.aborted) cancel.cancel();
              try {
                const tool = new HmiAgentTool(name, getClient, {
                  storageRoot: path.join(context.globalStorageUri.fsPath, "hmi-drafts"),
                  fontFile: context.asAbsolutePath("media/fonts/InterVariable.woff2"),
                });
                const result = await tool.invoke({ input }, cancel.token);
                const structuredContent = JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value) as {
                  ok: boolean;
                };
                return {
                  structuredContent,
                  isError: structuredContent.ok === false,
                  content: result.content.map((part) =>
                    part instanceof vscode.LanguageModelDataPart
                      ? { type: "image", mimeType: part.mimeType, data: Buffer.from(part.data).toString("base64") }
                      : { type: "text", text: (part as vscode.LanguageModelTextPart).value },
                  ),
                };
              } finally {
                signal.removeEventListener("abort", abort);
                cancel.dispose();
              }
            },
          });
          const directory = path.join(context.globalStorageUri.fsPath, "agent-connections");
          const connectionFile = path.join(directory, `${workspaceId}-${randomUUID()}.json`);
          try {
            await fs.mkdir(directory, { recursive: true, mode: 0o700 });
            await fs.writeFile(
              connectionFile,
              JSON.stringify({ url: bridge.url, capability: bridge.capability, workspaceId }),
              { mode: 0o600, flag: "wx" },
            );
          } catch (error) {
            await bridge.close();
            throw error;
          }
          connection = { bridge, connectionFile };
          connections.set(workspaceId, connection);
        }
        // The agent starts the truST language server's stdio MCP server, which owns the MCP rules (spec 43, §3.1).
        const command = getBinaryPath(context, "trust-lsp", "server.path");
        const configuration = { command, args: ["hmi-agent", connection.connectionFile] };
        if (args?.reveal !== false) {
          const toml = `[mcp_servers.trust_hmi]\ncommand = ${JSON.stringify(command)}\nargs = ${JSON.stringify(configuration.args)}\n`;
          const document = await vscode.workspace.openTextDocument({
            language: "markdown",
            content: `# Connect your agent to ${folder.name}\n\nThis connection reads the current project, including unsaved HMI edits.\nKeep this VS Code window open. Reconnect after restarting it.\n\n## Codex MCP configuration\n\nAdd this entry through your agent's MCP settings. This command does not modify those settings.\n\n\`\`\`toml\n${toml}\`\`\`\n\nThe connection file is private. Do not copy its contents into your project.\n\nTools cover project discovery, isolated drafts, validation, rendered previews and reviewed application. Authoring does not deploy or operate equipment.\n`,
          });
          await vscode.window.showTextDocument(document, { preview: true });
        }
        return { workspaceId, connectionFile: connection.connectionFile, configuration };
      },
    ),
  );
}
