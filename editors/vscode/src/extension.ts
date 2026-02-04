import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  Trace,
} from "vscode-languageclient/node";
import { registerDebugAdapter } from "./debug";
import { getBinaryPath } from "./binary";
import { registerIoPanel } from "./ioPanel";
import { registerLanguageModelTools } from "./lm-tools";
import { augmentDiagnostic } from "./diagnostics";
import {
  registerNamespaceMoveCommand,
  registerNamespaceMoveCodeActions,
  registerNamespaceMoveContext,
} from "./namespaceMove";

let client: LanguageClient | undefined;
let showIecDiagnosticRefs = true;
let startAttempts = 0;
let startRetryTimer: NodeJS.Timeout | undefined;
let notifiedStartFailure = false;

const MAX_START_ATTEMPTS = 3;
const START_RETRY_DELAY_MS = 3000;

function sendServerConfig(target: LanguageClient | undefined): void {
  if (!target) {
    return;
  }
  const config = vscode.workspace.getConfiguration("trust-lsp");
  void target.sendNotification("workspace/didChangeConfiguration", {
    settings: { "trust-lsp": config },
  });
}


function resolveServerCommand(context: vscode.ExtensionContext): string {
  return getBinaryPath(context, "trust-lsp", "server.path");
}

function traceFromConfig(value?: string): Trace {
  switch (value) {
    case "messages":
      return Trace.Messages;
    case "verbose":
      return Trace.Verbose;
    default:
      return Trace.Off;
  }
}

function readIecDiagnosticsSetting(config: vscode.WorkspaceConfiguration): boolean {
  return config.get<boolean>("diagnostics.showIecReferences", true);
}

function startClientWithRetry(
  context: vscode.ExtensionContext,
  config: vscode.WorkspaceConfiguration
): void {
  if (!client) {
    return;
  }
  if (startRetryTimer) {
    clearTimeout(startRetryTimer);
    startRetryTimer = undefined;
  }

  const attempt = startAttempts + 1;
  startAttempts = attempt;
  const startPromise = client.start();
  const trace = traceFromConfig(config.get<string>("trace.server"));

  void startPromise.then(() => {
    startAttempts = 0;
    notifiedStartFailure = false;
    if (client) {
      sendServerConfig(client);
      void client.setTrace(trace);
    }
  });

  void startPromise.catch((error) => {
    const message =
      error instanceof Error ? error.message : String(error ?? "");
    const isNotFound = message.includes("ENOENT") || message.includes("not found");
    if (!notifiedStartFailure) {
      notifiedStartFailure = true;
      const title = isNotFound
        ? "truST LSP failed to start: trust-lsp binary not found."
        : "truST LSP failed to start. Check the Output panel for details.";
      const actions = ["Show Output", "Retry Now"];
      if (isNotFound) {
        actions.unshift("Open Settings");
      }
      void vscode.window
        .showErrorMessage(title, ...actions)
        .then((selection) => {
          if (!selection) {
            return;
          }
          if (selection === "Open Settings") {
            void vscode.commands.executeCommand(
              "workbench.action.openSettings",
              "trust-lsp.server.path"
            );
            return;
          }
          if (selection === "Show Output") {
            client?.outputChannel?.show(true);
            return;
          }
          if (selection === "Retry Now") {
            startAttempts = 0;
            startClientWithRetry(context, config);
          }
        });
      if (isNotFound) {
        client?.outputChannel?.appendLine(
          "truST LSP could not find the trust-lsp binary. In dev mode, build it and copy into editors/vscode/bin, or set trust-lsp.server.path."
        );
      }
    }

    if (startAttempts < MAX_START_ATTEMPTS) {
      const delayMs = START_RETRY_DELAY_MS * startAttempts;
      startRetryTimer = setTimeout(() => {
        startClientWithRetry(context, config);
      }, delayMs);
    }
  });
}

export function activate(context: vscode.ExtensionContext) {
  registerDebugAdapter(context);
  registerIoPanel(context);
  registerLanguageModelTools(context, { getClient: () => client });
  const config = vscode.workspace.getConfiguration("trust-lsp");
  showIecDiagnosticRefs = readIecDiagnosticsSetting(config);
  const command = resolveServerCommand(context);

  const serverOptions: ServerOptions = {
    command,
    args: [],
    options: {
      env: process.env,
    },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "structured-text" }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher(
        "**/*.{st,ST,pou,POU}"
      ),
    },
    middleware: {
      handleDiagnostics(uri, diagnostics, next) {
        next(
          uri,
          diagnostics.map((diagnostic) =>
            augmentDiagnostic(diagnostic, showIecDiagnosticRefs)
          )
        );
      },
    },
  };

  client = new LanguageClient(
    "trust-lsp",
    "Structured Text Language Server",
    serverOptions,
    clientOptions
  );

  context.subscriptions.push(client);
  registerNamespaceMoveCommand(context, client);
  registerNamespaceMoveCodeActions(context);
  registerNamespaceMoveContext(context);

  startClientWithRetry(context, config);

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("trust-lsp")) {
        sendServerConfig(client);
      }
      if (event.affectsConfiguration("trust-lsp.trace.server")) {
        const updated = vscode.workspace
          .getConfiguration("trust-lsp")
          .get<string>("trace.server");
        if (client) {
          void client.setTrace(traceFromConfig(updated));
        }
      }
      if (event.affectsConfiguration("trust-lsp.server.path")) {
        vscode.window.showInformationMessage(
          "trust-lsp.server.path changed. Reload VS Code to restart the language server."
        );
      }
      if (event.affectsConfiguration("trust-lsp.diagnostics.showIecReferences")) {
        showIecDiagnosticRefs = readIecDiagnosticsSetting(
          vscode.workspace.getConfiguration("trust-lsp")
        );
      }
    })
  );
}

export async function deactivate(): Promise<void> {
  if (!client) {
    return;
  }
  if (startRetryTimer) {
    clearTimeout(startRetryTimer);
    startRetryTimer = undefined;
  }
  startAttempts = 0;
  notifiedStartFailure = false;
  const current = client;
  client = undefined;
  await current.stop();
}