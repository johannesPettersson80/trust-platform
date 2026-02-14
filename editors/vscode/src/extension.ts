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
import { registerHmiPanel } from "./hmiPanel";
import { registerLanguageModelTools } from "./lm-tools";
import { augmentDiagnostic } from "./diagnostics";
import { defaultRuntimeControlEndpoint } from "./runtimeDefaults";
import { registerNewProjectCommand } from "./newProject";
import { registerPlcopenImportCommand } from "./plcopenImport";
import { registerPlcopenExportCommand } from "./plcopenExport";
import { registerStTestIntegration } from "./stTests";
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

const RUNTIME_ENDPOINT_SEEDED_KEY = "runtimeControlEndpointSeeded";

async function seedDefaultRuntimeControlEndpoint(
  context: vscode.ExtensionContext
): Promise<void> {
  const folders = vscode.workspace.workspaceFolders ?? [];
  if (folders.length === 0) {
    return;
  }
  const defaultEndpoint = defaultRuntimeControlEndpoint();
  await Promise.all(
    folders.map(async (folder) => {
      const seedKey = `${RUNTIME_ENDPOINT_SEEDED_KEY}:${folder.uri.toString()}`;
      if (context.workspaceState.get<boolean>(seedKey)) {
        return;
      }
      const config = vscode.workspace.getConfiguration("trust-lsp", folder.uri);
      const current = config.get<string>("runtime.controlEndpoint") ?? "";
      if (!current.trim()) {
        await config.update(
          "runtime.controlEndpoint",
          defaultEndpoint,
          vscode.ConfigurationTarget.WorkspaceFolder
        );
      }
      await context.workspaceState.update(seedKey, true);
    })
  );
}

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
  const testServerPath = (process.env.ST_LSP_TEST_SERVER ?? "").trim();
  if (testServerPath.length > 0) {
    return testServerPath;
  }
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

export async function activate(context: vscode.ExtensionContext) {
  registerDebugAdapter(context);
  registerIoPanel(context);
  registerHmiPanel(context);
  registerLanguageModelTools(context, { getClient: () => client });
  registerStTestIntegration(context);
  await seedDefaultRuntimeControlEndpoint(context);
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
  registerNewProjectCommand(context);
  registerPlcopenImportCommand(context);
  registerPlcopenExportCommand(context);
  registerNamespaceMoveCommand(context, client);
  registerNamespaceMoveCodeActions(context);
  registerNamespaceMoveContext(context);
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "trust-lsp.hmi.init",
      async (input?: { style?: string } | string) => {
        if (!client) {
          throw new Error("Language client is not available.");
        }
        const rawStyle =
          typeof input === "string"
            ? input
            : typeof input?.style === "string"
              ? input.style
              : "";
        const style = rawStyle.trim().toLowerCase();
        const args = style ? [{ style }] : [];
        return client.sendRequest("workspace/executeCommand", {
          command: "trust-lsp.hmiInit",
          arguments: args,
        });
      }
    )
  );

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

  context.subscriptions.push(
    vscode.workspace.onDidChangeWorkspaceFolders(async () => {
      await seedDefaultRuntimeControlEndpoint(context);
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
