import * as vscode from "vscode";

import {
  DEBUG_TYPE,
  debugChannel,
  ensureConfigurationEntryAuto,
  isConfigurationFile,
  runtimeSourceOptions,
  validateConfiguration,
} from "./configuration";
import {
  launchControlEndpointError,
  launchControlPreparationError,
  prepareLaunchControl,
} from "./launchControl";
import { redactDebugConfig, redactDebugText } from "./sessionLogging";
import { LIFECYCLE_START_ATTEMPT_FIELD } from "./startAttempt";

type DebugStartOptions = {
  readonly program?: string | vscode.Uri;
  readonly lifecycleAttemptId?: string;
  readonly workspaceFolder?: vscode.Uri;
};

type DebugStartArgument = string | vscode.Uri | DebugStartOptions | undefined;

/** Registers the simulator Start command and owns its complete pre-DAP path. */
export function registerDebugStartCommand(
  context: vscode.ExtensionContext
): vscode.Disposable {
  return vscode.commands.registerCommand(
    "trust-lsp.debug.start",
    (startArg?: DebugStartArgument) => executeDebugStart(context, startArg)
  );
}

async function executeDebugStart(
  context: vscode.ExtensionContext,
  startArg: DebugStartArgument
): Promise<boolean> {
  const startOptions = debugStartOptions(startArg);
  const lifecycleOwnedStart = Boolean(
    startOptions?.lifecycleAttemptId?.trim()
  );
  const programOverride = startOptions?.program ?? startArg;
  let programUri: vscode.Uri | undefined;
  let folder = startOptions?.workspaceFolder
    ? vscode.workspace.getWorkspaceFolder(startOptions.workspaceFolder) ??
      vscode.workspace.workspaceFolders?.find(
        (candidate) =>
          candidate.uri.toString() === startOptions.workspaceFolder?.toString()
      )
    : undefined;

  if (typeof programOverride === "string" && programOverride.trim()) {
    programUri = vscode.Uri.file(programOverride);
  } else if (programOverride instanceof vscode.Uri) {
    programUri = programOverride;
  }

  if (programUri) {
    if (!(await isConfigurationFile(programUri))) {
      vscode.window.showErrorMessage(
        "Debugging requires a CONFIGURATION entry file."
      );
      return false;
    }
  } else {
    programUri = await ensureConfigurationEntryAuto(folder);
    if (!programUri) {
      debugChannel().appendLine(
        "Simulator start stopped before launch: no CONFIGURATION entry could be selected."
      );
      return false;
    }
  }

  folder = vscode.workspace.getWorkspaceFolder(programUri) ?? folder;
  if (!folder) {
    folder = vscode.workspace.workspaceFolders?.[0];
  }

  const program = programUri.fsPath;
  const config: vscode.DebugConfiguration = {
    type: DEBUG_TYPE,
    request: "launch",
    name: "truST Simulator",
    program,
    internalConsoleOptions: "neverOpen",
    ...runtimeSourceOptions(programUri),
  };
  if (startOptions?.lifecycleAttemptId?.trim()) {
    config[LIFECYCLE_START_ATTEMPT_FIELD] =
      startOptions.lifecycleAttemptId.trim();
  }
  if (folder) {
    config.cwd = folder.uri.fsPath;
  }

  // Repair legacy Windows local control before cached diagnostics or source
  // validation can reject the first Start click. The public Start command and
  // the Run pane therefore observe the same secured runtime.toml; VS Code F5
  // reaches this migration through its configuration-provider path.
  const launchControl = prepareLaunchControl(
    config,
    folder,
    context.extensionMode === vscode.ExtensionMode.Test
  );
  if (launchControl.migratedRuntimeToml) {
    debugChannel().appendLine(
      "Secured Windows local runtime control authentication in runtime.toml."
    );
  }
  if (launchControl.failure) {
    throw launchControlPreparationError(launchControl.failure);
  }

  const diagnostics = vscode.languages.getDiagnostics(programUri);
  if (
    !lifecycleOwnedStart &&
    diagnostics.some(
      (diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Error
    )
  ) {
    const errors = diagnostics
      .filter(
        (diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Error
      )
      .map((diagnostic) => diagnostic.message)
      .join("; ");
    debugChannel().appendLine(
      `Simulator start stopped before launch: CONFIGURATION diagnostics: ${redactDebugText(errors)}`
    );
    vscode.window.showErrorMessage(
      "Configuration has errors. Fix them before starting a debug session."
    );
    return false;
  }
  if (!(await validateConfiguration(programUri))) {
    debugChannel().appendLine(
      `Simulator start stopped before launch: CONFIGURATION validation failed for ${programUri.fsPath}.`
    );
    return false;
  }

  const launchEndpointError = await launchControlEndpointError(
    config.controlEndpoint
  );
  if (launchEndpointError) {
    throw new Error(launchEndpointError);
  }

  debugChannel().appendLine(`Start debugging command: program=${program}`);
  const lifecycleOwnedDebugUi: vscode.DebugSessionOptions | undefined =
    lifecycleOwnedStart
      ? {
          // The truST sidebar owns this Simulator lifecycle. Direct F5 and
          // no-proof debug commands keep VS Code's ordinary debugger chrome.
          suppressDebugToolbar: true,
          suppressDebugStatusbar: true,
          suppressDebugView: true,
        }
      : undefined;
  const pendingTimer = setTimeout(() => {
    const active = vscode.debug.activeDebugSession;
    debugChannel().appendLine(
      `startDebugging still pending after 5s: active=${active?.name ?? "<none>"} type=${active?.type ?? "<none>"} config=${JSON.stringify(redactDebugConfig(config))}`
    );
  }, 5000);
  try {
    const started = await vscode.debug.startDebugging(
      folder,
      config,
      lifecycleOwnedDebugUi,
    );
    clearTimeout(pendingTimer);
    debugChannel().appendLine(
      `startDebugging result: ${started} folder=${folder?.name ?? "<none>"} config=${JSON.stringify(redactDebugConfig(config))}`
    );
    return started;
  } catch (error: unknown) {
    clearTimeout(pendingTimer);
    debugChannel().appendLine(
      `startDebugging error: ${redactDebugText(error instanceof Error ? error.message : String(error))} folder=${folder?.name ?? "<none>"} config=${JSON.stringify(redactDebugConfig(config))}`
    );
    throw error;
  }
}

function debugStartOptions(
  startArg: DebugStartArgument
): DebugStartOptions | undefined {
  return startArg &&
    typeof startArg === "object" &&
    !(startArg instanceof vscode.Uri) &&
    ("program" in startArg ||
      LIFECYCLE_START_ATTEMPT_FIELD in startArg ||
      "workspaceFolder" in startArg)
    ? startArg
    : undefined;
}
