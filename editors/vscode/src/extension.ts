import { affectsTrustConfiguration, getTrustConfiguration } from "./configuration";
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
import { registerNetworkCanvasPanel } from "./networkCanvas/networkCanvasPanel";
import { registerRuntimeLifecycle } from "./runtimeLifecycle";
import { registerRuntimeControls } from "./runtimeControls";
import { registerTrustHome } from "./trustHomeView";
import { registerLanguageModelTools } from "./lm-tools";
import { augmentDiagnostic } from "./diagnostics";
import { focusPendingMain, registerNewProjectCommand } from "./newProject";
import { registerExamples } from "./examples";
import { registerCheckProgram } from "./checkProgram";
import { registerLibraryCodeActions } from "./libraryCodeActions";
import { registerLibraries } from "./libraries";
import { initSelectedRuntimeStore } from "./selectedRuntime";
import { registerRuntimeAuth } from "./runtimeAuth";
import { registerLocalRuntime } from "./localRuntime";
import { registerOpcuaTrust } from "./opcuaTrust";
import { registerNewStatechartCommand } from "./statechart/newStatechart";
import { registerImportStatechartCommand } from "./statechart/importStatechart";
import { registerNewBlocklyCommand } from "./blockly/newBlockly";
import { registerImportBlocklyCommand } from "./blockly/importBlockly";
import { registerNewLadderCommand } from "./ladder/newLadder";
import { registerImportLadderCommand } from "./ladder/importLadder";
import { newSfcCommand } from "./sfc/newSfc";
import { registerImportSfcCommand } from "./sfc/importSfc";
import { registerPlcopenImportCommand } from "./plcopenImport";
import { registerPlcopenExportCommand } from "./plcopenExport";
import { registerStTestIntegration } from "./stTests";
import {
  registerNamespaceMoveCommand,
  registerNamespaceMoveCodeActions,
  registerNamespaceMoveContext,
} from "./namespaceMove";
import { StateChartEditorProvider } from "./statechart/stateChartEditor";
import { BlocklyEditorProvider } from "./blockly/blocklyEditor";
import { LadderEditorProvider } from "./ladder/ladderEditor";
import { SfcEditorProvider } from "./sfc/sfcEditor";
import { registerVisualCompanionSync } from "./visual/companionSt";
import { registerVisualCustomEditorAutoOpen } from "./visual/autoOpenCustomEditors";
import { createProductRuntimeIdentityApi } from "./productRuntimeIdentity";

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
  const config = getTrustConfiguration();
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
              "trust.languageServer.executablePath"
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
          "truST LSP could not find the language server binary. In dev mode, build it and copy into editors/vscode/bin, or set trust.languageServer.executablePath."
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
  context.subscriptions.push(StateChartEditorProvider.register(context));
  context.subscriptions.push(BlocklyEditorProvider.register(context));
  context.subscriptions.push(LadderEditorProvider.register(context));
  context.subscriptions.push(SfcEditorProvider.register(context));

  registerDebugAdapter(context);
  registerRuntimeLifecycle(context);
  initSelectedRuntimeStore(context);
  registerRuntimeAuth(context);
  registerLocalRuntime(context);
  registerOpcuaTrust(context);
  registerRuntimeControls(context);
  registerTrustHome(context);
  registerIoPanel(context);
  registerHmiPanel(context);
  registerNetworkCanvasPanel(context);
  try {
    registerLanguageModelTools(context, { getClient: () => client });
  } catch (error) {
    console.error("Failed to register language model tools:", error);
  }
  context.subscriptions.push(registerVisualCustomEditorAutoOpen());
  context.subscriptions.push(registerVisualCompanionSync());
  registerStTestIntegration(context);
  registerLibraryCodeActions(context);
  const config = getTrustConfiguration();
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
  registerExamples(context);
  registerCheckProgram(context);
  registerLibraries(context, { getClient: () => client });
  // If a project was just scaffolded (which reloaded the window), focus its Main.st now.
  void focusPendingMain(context);
  registerNewStatechartCommand(context);
  registerImportStatechartCommand(context);
  registerNewBlocklyCommand(context);
  registerImportBlocklyCommand(context);
  registerNewLadderCommand(context);
  registerImportLadderCommand(context);
  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.sfc.new", newSfcCommand)
  );
  registerImportSfcCommand(context);
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
      if (affectsTrustConfiguration(event)) {
        sendServerConfig(client);
      }
      if (affectsTrustConfiguration(event, "trace.server")) {
        const updated = getTrustConfiguration().get<string>("trace.server");
        if (client) {
          void client.setTrace(traceFromConfig(updated));
        }
      }
      if (affectsTrustConfiguration(event, "server.path")) {
        vscode.window.showInformationMessage(
          "Language server path changed. Reload VS Code to restart the truST language server."
        );
      }
      if (affectsTrustConfiguration(event, "diagnostics.showIecReferences")) {
        showIecDiagnosticRefs = readIecDiagnosticsSetting(
          getTrustConfiguration()
        );
      }
    })
  );
  return createProductRuntimeIdentityApi(context, command);
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
