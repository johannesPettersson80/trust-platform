import * as fs from "fs";
import * as net from "net";
import * as path from "path";
import * as vscode from "vscode";

import { getBinaryPath } from "./binary";
import { localSimControl } from "./simControl";
import {
  __testCreateDefaultConfigurationAuto,
  __testEnsureConfigurationEntryAuto,
  captureStructuredTextEditor,
  clearSessionProgram,
  DEBUG_TYPE,
  debugChannel,
  ensureConfigurationEntry,
  ensureConfigurationEntryAuto,
  initializeDebugConfigurationState,
  isConfigurationFile,
  loadRuntimeControlConfig,
  markSessionProgram,
  maybeReloadForEditor,
  preferredStructuredTextUri,
  runtimeSourceOptions,
  selectWorkspaceFolderPathForMode,
  validateConfiguration,
} from "./debug/configuration";
import { diagnosticsGateReason, validityLine } from "./compileGate";
import { parseControlEndpoint } from "./runtimeControl";

const LAUNCH_WARN_DELAY_MS = 1500;
const HOT_RELOAD_REQUEST_TIMEOUT_MS = 30000;
const IO_BUSY_RETRY_DELAYS_MS = [75, 150, 250, 400, 600];
const DEBUG_STOP_WAIT_TIMEOUT_MS = 8000;
const DEBUG_STOP_UI_SETTLE_MS = 1000;
const TEST_CONTROL_ENDPOINT_OVERRIDE_ENV = "TRUST_UX_DEBUG_CONTROL_ENDPOINT";
const TEST_CONTROL_AUTH_TOKEN_ENV = "TRUST_UX_DEBUG_CONTROL_AUTH_TOKEN";

export type DebugReloadEvent = {
  readonly ok: boolean;
  readonly message?: string;
  readonly gated?: boolean;
};

const debugReloadEmitter = new vscode.EventEmitter<DebugReloadEvent>();
export const onDidDebugReload = debugReloadEmitter.event;

// The debug output channel is user-visible; never log the injected per-session control token.
function redactDebugConfig(config: vscode.DebugConfiguration): Record<string, unknown> {
  if (config.controlAuthToken === undefined) {
    return config as Record<string, unknown>;
  }
  return { ...config, controlAuthToken: "***" };
}

function applyLaunchControlEndpoint(
  config: vscode.DebugConfiguration,
  folder: vscode.WorkspaceFolder | undefined,
  allowTestControlEndpointOverride: boolean
): void {
  if (config.request !== "launch" || config.controlEndpoint) {
    return;
  }
  const testEndpoint = allowTestControlEndpointOverride
    ? (process.env[TEST_CONTROL_ENDPOINT_OVERRIDE_ENV] ?? "").trim()
    : "";
  if (testEndpoint) {
    config.controlEndpoint = testEndpoint;
    const testToken = (process.env[TEST_CONTROL_AUTH_TOKEN_ENV] ?? "").trim();
    if (testToken) {
      config.controlAuthToken = testToken;
    }
    return;
  }
  const sim = localSimControl(folder?.uri.fsPath);
  if (sim) {
    config.controlEndpoint = sim.endpoint;
    config.controlAuthToken = sim.authToken;
  }
}

async function launchControlEndpointError(
  endpoint: unknown
): Promise<string | undefined> {
  if (typeof endpoint !== "string") {
    return undefined;
  }
  const parsed = parseControlEndpoint(endpoint);
  if (!parsed || parsed.kind !== "tcp") {
    return undefined;
  }
  if (await canConnectToTcpEndpoint(parsed.host, parsed.port)) {
    return "The runtime port is already in use.";
  }
  return undefined;
}

function canConnectToTcpEndpoint(host: string, port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = net.createConnection({ host, port });
    let finished = false;
    const finish = (value: boolean) => {
      if (finished) {
        return;
      }
      finished = true;
      socket.removeAllListeners();
      socket.destroy();
      resolve(value);
    };
    socket.once("connect", () => finish(true));
    socket.once("error", () => finish(false));
    socket.setTimeout(300, () => finish(false));
  });
}

function withTimeout<T>(
  operation: () => Thenable<T>,
  timeoutMs: number,
  message: string
): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  return new Promise<T>((resolve, reject) => {
    timer = setTimeout(() => reject(new Error(message)), timeoutMs);
    void Promise.resolve().then(operation).then(
      (value) => {
        if (timer) {
          clearTimeout(timer);
        }
        resolve(value);
      },
      (error) => {
        if (timer) {
          clearTimeout(timer);
        }
        reject(error);
      }
    );
  });
}

function summarizeReloadCommandMessage(message: string): string {
  const trimmed = message.trim();
  const sourceErrorCount = trimmed
    .split(/\r?\n/)
    .filter((line) => /\.(st|pou):/i.test(line)).length;
  if (sourceErrorCount > 0) {
    return `Compile failed — ${sourceErrorCount} error${sourceErrorCount === 1 ? "" : "s"}. Open Problems, then try again.`;
  }
  const firstLine =
    trimmed.split(/\r?\n/).find((line) => line.trim())?.trim() ?? "";
  if (!firstLine) {
    return "Reload did not report a reason.";
  }
  if (firstLine.length <= 160) {
    return firstLine;
  }
  return `${firstLine.slice(0, 157).trimEnd()}...`;
}

// Same, for a DAP message (the launch request carries the config under `arguments`).
function redactDapMessage(value: unknown): unknown {
  if (!value || typeof value !== "object") {
    return value;
  }
  const message = value as { arguments?: Record<string, unknown> };
  const args = message.arguments;
  if (args && typeof args === "object" && "controlAuthToken" in args) {
    return {
      ...(value as Record<string, unknown>),
      arguments: { ...args, controlAuthToken: "***" },
    };
  }
  return value;
}

type LaunchFallbackState = {
  seenLaunch: boolean;
  fallbackTimer?: NodeJS.Timeout;
};

const launchFallbackState = new Map<string, LaunchFallbackState>();
const structuredTextSessions = new Map<string, vscode.DebugSession>();

type IoCommandArgs = {
  address?: string;
  value?: string;
};

export type IoDebugAction = "write" | "force" | "release";

export interface IoDebugRequestSession {
  customRequest(command: string, args?: unknown): Thenable<unknown>;
}

type ExpressionCommandArgs = {
  expression?: string;
  value?: string;
};

function structuredTextSessionKey(session: vscode.DebugSession): string {
  return session.id ?? session.name;
}

function structuredTextSession(): vscode.DebugSession | undefined {
  const active = vscode.debug.activeDebugSession;
  if (active && active.type === DEBUG_TYPE) {
    return active;
  }
  for (const session of structuredTextSessions.values()) {
    return session;
  }
  return undefined;
}

function sameDebugSession(
  left: vscode.DebugSession | undefined,
  right: vscode.DebugSession
): boolean {
  if (!left) {
    return false;
  }
  if (left.id && right.id) {
    return left.id === right.id;
  }
  return left.name === right.name && left.type === right.type;
}

async function waitForStructuredTextSessionTerminated(
  session: vscode.DebugSession,
  timeoutMs = DEBUG_STOP_WAIT_TIMEOUT_MS
): Promise<boolean> {
  return new Promise((resolve) => {
    const finish = (ok: boolean) => {
      clearTimeout(timer);
      disposable.dispose();
      resolve(ok);
    };
    const timer = setTimeout(() => finish(false), timeoutMs);
    const disposable = vscode.debug.onDidTerminateDebugSession((terminated) => {
      if (sameDebugSession(terminated, session)) {
        finish(true);
      }
    });
  });
}

function normalizeIoCommandArgs(args: unknown[]): IoCommandArgs {
  const first = args[0];
  if (first && typeof first === "object") {
    const typed = first as { address?: unknown; value?: unknown };
    return {
      address:
        typeof typed.address === "string" ? typed.address.trim() : undefined,
      value: typeof typed.value === "string" ? typed.value : undefined,
    };
  }
  return {
    address: typeof first === "string" ? first.trim() : undefined,
    value: typeof args[1] === "string" ? args[1] : undefined,
  };
}

function normalizeExpressionCommandArgs(args: unknown[]): ExpressionCommandArgs {
  const first = args[0];
  if (first && typeof first === "object") {
    const typed = first as {
      expression?: unknown;
      address?: unknown;
      value?: unknown;
    };
    const expression =
      typeof typed.expression === "string"
        ? typed.expression.trim()
        : typeof typed.address === "string"
          ? typed.address.trim()
          : undefined;
    return {
      expression,
      value: typeof typed.value === "string" ? typed.value : undefined,
    };
  }
  return {
    expression: typeof first === "string" ? first.trim() : undefined,
    value: typeof args[1] === "string" ? args[1] : undefined,
  };
}

export async function __testSendIoDebugRequest(
  session: IoDebugRequestSession,
  action: IoDebugAction,
  address: string,
  value?: string
): Promise<void> {
  await sendIoDebugRequest(session, action, address, value);
}

async function sendIoDebugRequest(
  session: IoDebugRequestSession,
  action: IoDebugAction,
  address: string,
  value?: string
): Promise<void> {
  switch (action) {
    case "write":
      await sendIoCustomRequest(session, "stIoWrite", {
        address,
        value: value ?? "FALSE",
      });
      return;
    case "force":
      await sendIoCustomRequest(session, "stIoForce", {
        address,
        value: value ?? "FALSE",
      });
      return;
    case "release":
      await sendIoCustomRequest(session, "stIoRelease", { address });
      return;
  }
}

async function sendIoCustomRequest(
  session: IoDebugRequestSession,
  command: string,
  args: Record<string, unknown>
): Promise<void> {
  let lastError: unknown;
  for (let attempt = 0; attempt <= IO_BUSY_RETRY_DELAYS_MS.length; attempt += 1) {
    try {
      await session.customRequest(command, args);
      return;
    } catch (error) {
      lastError = error;
      if (!isRuntimeBusyError(error) || attempt >= IO_BUSY_RETRY_DELAYS_MS.length) {
        throw error;
      }
      await sleep(IO_BUSY_RETRY_DELAYS_MS[attempt]);
    }
  }
  throw lastError;
}

function isRuntimeBusyError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return /\bruntime busy\b/i.test(message);
}

async function sleep(ms: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

function resolveAdapterCommand(
  config: vscode.WorkspaceConfiguration,
  context: vscode.ExtensionContext
): string {
  return getBinaryPath(context, "trust-debug", "debug.adapter.path");
}

async function ensureAdapterCommand(
  config: vscode.WorkspaceConfiguration,
  context: vscode.ExtensionContext
): Promise<string | undefined> {
  const command = resolveAdapterCommand(config, context);

  if (path.isAbsolute(command) && !fs.existsSync(command)) {
    void vscode.window
      .showErrorMessage(
        `Structured Text debug adapter not found at '${command}'. ` +
          `Install the extension from the Marketplace or set trust-lsp.debug.adapter.path.`,
        "Open Settings"
      )
      .then((choice) => {
        if (choice === "Open Settings") {
          void vscode.commands.executeCommand(
            "workbench.action.openSettings",
            "trust-lsp.debug.adapter.path"
          );
        }
      });
    return undefined;
  }

  return command;
}

function adapterEnv(
  config: vscode.WorkspaceConfiguration
): Record<string, string> {
  const overrides =
    config.get<Record<string, string>>("debug.adapter.env") ?? {};
  return {
    ...(process.env as Record<string, string>),
    ...overrides,
  };
}

class StructuredTextDebugAdapterFactory
  implements vscode.DebugAdapterDescriptorFactory, vscode.Disposable
{
  constructor(private readonly context: vscode.ExtensionContext) {}

  dispose(): void {
    // No resources to dispose yet.
  }

  createDebugAdapterDescriptor(
    _session: vscode.DebugSession
  ): vscode.ProviderResult<vscode.DebugAdapterDescriptor> {
    const config = vscode.workspace.getConfiguration("trust-lsp");
    debugChannel().appendLine("createDebugAdapterDescriptor called");
    return ensureAdapterCommand(config, this.context).then((command) => {
      if (!command) {
        debugChannel().appendLine(
          "No debug adapter command resolved; aborting session."
        );
        return undefined;
      }
      debugChannel().appendLine(`Launching adapter: ${command}`);
      const args = config.get<string[]>("debug.adapter.args") ?? [];
      const options: vscode.DebugAdapterExecutableOptions = {
        env: adapterEnv(config),
      };
      return new vscode.DebugAdapterExecutable(command, args, options);
    });
  }
}

class StructuredTextDebugConfigurationProvider
  implements vscode.DebugConfigurationProvider
{
  constructor(private readonly allowTestControlEndpointOverride: boolean) {}

  provideDebugConfigurations(
    _folder: vscode.WorkspaceFolder | undefined,
    _token?: vscode.CancellationToken
  ): vscode.ProviderResult<vscode.DebugConfiguration[]> {
    return [
      {
        type: DEBUG_TYPE,
        request: "launch",
        name: "truST Simulator",
        program: "${file}",
        internalConsoleOptions: "neverOpen",
      },
    ];
  }

  async resolveDebugConfiguration(
    folder: vscode.WorkspaceFolder | undefined,
    config: vscode.DebugConfiguration
  ): Promise<vscode.DebugConfiguration | null | undefined> {
    if (!config.type && !config.request && !config.name) {
      config.type = DEBUG_TYPE;
      config.request = "launch";
      config.name = "truST Simulator";
    }

    if (!config.type) {
      config.type = DEBUG_TYPE;
    }
    if (!config.request) {
      config.request = "launch";
    }
    if (!config.name) {
      config.name = "truST Simulator";
    }

    if (config.request === "attach") {
      if (!config.endpoint) {
        const controlConfig = loadRuntimeControlConfig(folder);
        if (!controlConfig?.endpoint) {
          vscode.window.showErrorMessage(
            "Attach requires runtime.control.endpoint in runtime.toml."
          );
          return null;
        }
        config.endpoint = controlConfig.endpoint;
        if (controlConfig.authToken && !config.authToken) {
          config.authToken = controlConfig.authToken;
        }
      }
      const runtimeOptions = runtimeSourceOptions();
      Object.assign(config, runtimeOptions);
    } else {
      if (!config.program) {
        const configUri = await ensureConfigurationEntryAuto();
        if (!configUri) {
          return null;
        }
        config.program = configUri.fsPath;
      } else {
        const programUri = vscode.Uri.file(config.program);
        if (!(await isConfigurationFile(programUri))) {
          const configUri = await ensureConfigurationEntryAuto();
          if (!configUri) {
            return null;
          }
          config.program = configUri.fsPath;
        }
      }
    }

    if (!config.cwd && folder) {
      config.cwd = folder.uri.fsPath;
    }

    // Pin the launch session's control server to a known per-workspace endpoint + a random
    // per-session token (the adapter already starts a control server on launch). This lets the
    // Network Canvas reach the live simulator for comm.schema/apply + fleet.topology with write
    // access, and avoids two workspaces colliding on one default socket. Explicit user configs win.
    //
    // Test-mode evidence runners can override the endpoint to force a real TCP bind conflict for
    // ERR-04. This is intentionally not a user setting: production/dev users keep the normal
    // per-workspace Unix socket.
    applyLaunchControlEndpoint(
      config,
      folder,
      this.allowTestControlEndpointOverride
    );

    debugChannel().appendLine(
      `Resolved debug config: type=${config.type} request=${config.request} program=${config.program ?? "<none>"} cwd=${config.cwd ?? "<none>"}`
    );

    return config;
  }

  resolveDebugConfigurationWithSubstitutedVariables(
    _folder: vscode.WorkspaceFolder | undefined,
    config: vscode.DebugConfiguration
  ): vscode.DebugConfiguration | null | undefined {
    debugChannel().appendLine(
      `Resolved debug config (substituted): type=${config.type} request=${config.request} program=${config.program ?? "<none>"} cwd=${config.cwd ?? "<none>"}`
    );
    return config;
  }
}

class StructuredTextDebugAdapterTrackerFactory
  implements vscode.DebugAdapterTrackerFactory
{
  createDebugAdapterTracker(
    session: vscode.DebugSession
  ): vscode.ProviderResult<vscode.DebugAdapterTracker> {
    if (session.type !== DEBUG_TYPE) {
      return undefined;
    }
    const interestingCommands = new Set([
      "initialize",
      "launch",
      "configurationDone",
      "setBreakpoints",
      "threads",
      "stackTrace",
      "scopes",
      "variables",
      "continue",
      "pause",
      "disconnect",
    ]);
    const interestingEvents = new Set([
      "initialized",
      "stopped",
      "continued",
      "terminated",
      "exited",
      "output",
    ]);
    const sessionId = session.id ?? session.name;
    const state: LaunchFallbackState = {
      seenLaunch: false,
    };
    launchFallbackState.set(sessionId, state);
    const channel = debugChannel();
    const formatMessage = (value: unknown): string => {
      try {
        return JSON.stringify(redactDapMessage(value));
      } catch (err) {
        return String(err);
      }
    };
    channel.appendLine(`Debug adapter tracker attached: ${session.name}`);
    return {
      onWillReceiveMessage: (message) => {
        channel.appendLine(`[DAP <-] ${formatMessage(message)}`);
        const command = (message as { command?: string }).command;
        if (command && interestingCommands.has(command)) {
          channel.appendLine(`[DAP <-] ${command}`);
        }
        if (command === "launch") {
          state.seenLaunch = true;
        }
      },
      onDidSendMessage: (message) => {
        channel.appendLine(`[DAP ->] ${formatMessage(message)}`);
        const event = (message as { event?: string }).event;
        if (event && interestingEvents.has(event)) {
          channel.appendLine(`[DAP ->] event ${event}`);
        }
        if (event === "initialized" && !state.fallbackTimer) {
          state.fallbackTimer = setTimeout(() => {
            const current = launchFallbackState.get(sessionId);
            if (!current || current.seenLaunch) {
              return;
            }
            channel.appendLine(
              "[DAP] launch not seen after initialized; waiting for VS Code"
            );
          }, LAUNCH_WARN_DELAY_MS);
        }
      },
      onError: (error) => {
        channel.appendLine(`[DAP] error: ${error}`);
      },
      onExit: (code, signal) => {
        channel.appendLine(
          `[DAP] exit: code=${code ?? "<none>"} signal=${signal ?? "<none>"}`
        );
        const current = launchFallbackState.get(sessionId);
        if (current?.fallbackTimer) {
          clearTimeout(current.fallbackTimer);
        }
        launchFallbackState.delete(sessionId);
      },
    };
  }
}

export function registerDebugAdapter(
  context: vscode.ExtensionContext
): void {
  initializeDebugConfigurationState(context.workspaceState);
  captureStructuredTextEditor(vscode.window.activeTextEditor);

  const factory = new StructuredTextDebugAdapterFactory(context);
  context.subscriptions.push(
    vscode.debug.registerDebugAdapterDescriptorFactory(DEBUG_TYPE, factory)
  );
  context.subscriptions.push(factory);
  debugChannel().appendLine("Structured Text debug adapter factory registered.");

  const provider = new StructuredTextDebugConfigurationProvider(
    context.extensionMode === vscode.ExtensionMode.Test
  );
  context.subscriptions.push(
    vscode.debug.registerDebugConfigurationProvider(DEBUG_TYPE, provider)
  );
  context.subscriptions.push(
    vscode.debug.registerDebugConfigurationProvider(
      DEBUG_TYPE,
      provider,
      vscode.DebugConfigurationProviderTriggerKind.Dynamic
    )
  );

  const trackerFactory = new StructuredTextDebugAdapterTrackerFactory();
  context.subscriptions.push(
    vscode.debug.registerDebugAdapterTrackerFactory(DEBUG_TYPE, trackerFactory)
  );

  const stringifySession = (session: vscode.DebugSession): string => {
    try {
      return JSON.stringify(session.configuration);
    } catch (err) {
      return String(err);
    }
  };

  context.subscriptions.push(
    vscode.debug.onDidStartDebugSession((session) => {
      debugChannel().appendLine(
        `Debug session started: ${session.name} type=${session.type} config=${stringifySession(session)}`
      );
      if (session.type === DEBUG_TYPE) {
        structuredTextSessions.set(structuredTextSessionKey(session), session);
        markSessionProgram(session);
      }
    })
  );

  context.subscriptions.push(
    vscode.debug.onDidTerminateDebugSession((session) => {
      debugChannel().appendLine(
        `Debug session terminated: ${session.name} type=${session.type} config=${stringifySession(session)}`
      );
      if (session.type === DEBUG_TYPE) {
        const sessionId = session.id ?? session.name;
        const current = launchFallbackState.get(sessionId);
        if (current?.fallbackTimer) {
          clearTimeout(current.fallbackTimer);
        }
        structuredTextSessions.delete(structuredTextSessionKey(session));
        launchFallbackState.delete(sessionId);
        clearSessionProgram(session);
      }
    })
  );

  context.subscriptions.push(
    vscode.debug.onDidChangeActiveDebugSession((session) => {
      if (session) {
        debugChannel().appendLine(
          `Debug session active: ${session.name} type=${session.type}`
        );
      } else {
        debugChannel().appendLine("Debug session active: <none>");
      }
    })
  );

  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      captureStructuredTextEditor(editor);
      void maybeReloadForEditor(editor);
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "trust-lsp.debug.start",
      async (programOverride?: string | vscode.Uri) => {
        let programUri: vscode.Uri | undefined;
        let folder: vscode.WorkspaceFolder | undefined;

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
          programUri = await ensureConfigurationEntryAuto();
          if (!programUri) {
            return false;
          }
        }

        folder = vscode.workspace.getWorkspaceFolder(programUri);
        if (!folder) {
          folder = vscode.workspace.workspaceFolders?.[0];
        }

        const diagnostics = vscode.languages.getDiagnostics(programUri);
        if (
          diagnostics.some(
            (diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Error
          )
        ) {
          vscode.window.showErrorMessage(
            "Configuration has errors. Fix them before starting a debug session."
          );
          return false;
        }
        if (!(await validateConfiguration(programUri))) {
          return false;
        }

        const program = programUri.fsPath;
        debugChannel().appendLine(`Start debugging command: program=${program}`);

        const runtimeOptions = runtimeSourceOptions(programUri);
        const config: vscode.DebugConfiguration = {
          type: DEBUG_TYPE,
          request: "launch",
          name: "truST Simulator",
          program,
          internalConsoleOptions: "neverOpen",
          ...runtimeOptions,
        };

        if (folder) {
          config.cwd = folder.uri.fsPath;
        }
        applyLaunchControlEndpoint(
          config,
          folder,
          context.extensionMode === vscode.ExtensionMode.Test
        );
        const launchEndpointError = await launchControlEndpointError(
          config.controlEndpoint
        );
        if (launchEndpointError) {
          throw new Error(launchEndpointError);
        }

        const pendingTimer = setTimeout(() => {
          const active = vscode.debug.activeDebugSession;
          debugChannel().appendLine(
            `startDebugging still pending after 5s: active=${active?.name ?? "<none>"} type=${active?.type ?? "<none>"} config=${JSON.stringify(redactDebugConfig(config))}`
          );
        }, 5000);
        try {
          const started = await vscode.debug.startDebugging(folder, config);
          clearTimeout(pendingTimer);
          debugChannel().appendLine(
            `startDebugging result: ${started} folder=${folder?.name ?? "<none>"} config=${JSON.stringify(redactDebugConfig(config))}`
          );
          return started;
        } catch (err: unknown) {
          clearTimeout(pendingTimer);
          debugChannel().appendLine(
            `startDebugging error: ${err instanceof Error ? err.message : String(err)} folder=${folder?.name ?? "<none>"} config=${JSON.stringify(redactDebugConfig(config))}`
          );
          throw err;
        }
      }
    )
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.debug.attach", async () => {
      const folder = vscode.workspace.workspaceFolders?.[0];
      const controlConfig = loadRuntimeControlConfig(folder);
      if (!controlConfig?.endpoint) {
        vscode.window.showErrorMessage(
          "Attach requires runtime.control.endpoint in runtime.toml."
        );
        return false;
      }
      const runtimeOptions = runtimeSourceOptions();
      const config: vscode.DebugConfiguration = {
        type: DEBUG_TYPE,
        request: "attach",
        name: "Attach Structured Text",
        endpoint: controlConfig.endpoint,
        authToken: controlConfig.authToken,
        internalConsoleOptions: "neverOpen",
        ...runtimeOptions,
      };
      if (folder) {
        config.cwd = folder.uri.fsPath;
      }
      return vscode.debug.startDebugging(folder, config);
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.debug.stop", async () => {
      const session = structuredTextSession();
      if (!session) {
        return false;
      }
      const terminated = waitForStructuredTextSessionTerminated(session);
      try {
        await vscode.debug.stopDebugging(session);
      } catch (err) {
        if (await terminated) {
          await sleep(DEBUG_STOP_UI_SETTLE_MS);
          return true;
        }
        throw err;
      }
      const stopped = await terminated;
      if (stopped) {
        await sleep(DEBUG_STOP_UI_SETTLE_MS);
      }
      return stopped;
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "trust-lsp.debug.io.write",
      async (...args: unknown[]) => {
        const { address, value } = normalizeIoCommandArgs(args);
        if (!address) {
          throw new Error("Missing I/O address.");
        }
        const session = structuredTextSession();
        if (!session) {
          throw new Error("No active Structured Text debug session.");
        }
        await sendIoDebugRequest(session, "write", address, value);
      }
    )
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "trust-lsp.debug.io.force",
      async (...args: unknown[]) => {
        const { address, value } = normalizeIoCommandArgs(args);
        if (!address) {
          throw new Error("Missing I/O address.");
        }
        const session = structuredTextSession();
        if (!session) {
          throw new Error("No active Structured Text debug session.");
        }
        await sendIoDebugRequest(session, "force", address, value);
      }
    )
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "trust-lsp.debug.io.release",
      async (...args: unknown[]) => {
        const { address } = normalizeIoCommandArgs(args);
        if (!address) {
          throw new Error("Missing I/O address.");
        }
        const session = structuredTextSession();
        if (!session) {
          throw new Error("No active Structured Text debug session.");
        }
        await sendIoDebugRequest(session, "release", address);
      }
    )
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "trust-lsp.debug.expr.write",
      async (...args: unknown[]) => {
        const { expression, value } = normalizeExpressionCommandArgs(args);
        if (!expression) {
          throw new Error("Missing expression.");
        }
        const session = structuredTextSession();
        if (!session) {
          throw new Error("No active Structured Text debug session.");
        }
        await session.customRequest("setExpression", {
          expression,
          value: value ?? "FALSE",
        });
      }
    )
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "trust-lsp.debug.expr.force",
      async (...args: unknown[]) => {
        const { expression, value } = normalizeExpressionCommandArgs(args);
        if (!expression) {
          throw new Error("Missing expression.");
        }
        const session = structuredTextSession();
        if (!session) {
          throw new Error("No active Structured Text debug session.");
        }
        await session.customRequest("setExpression", {
          expression,
          value: `force: ${value ?? "FALSE"}`,
        });
      }
    )
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "trust-lsp.debug.expr.release",
      async (...args: unknown[]) => {
        const { expression } = normalizeExpressionCommandArgs(args);
        if (!expression) {
          throw new Error("Missing expression.");
        }
        const session = structuredTextSession();
        if (!session) {
          throw new Error("No active Structured Text debug session.");
        }
        await session.customRequest("setExpression", {
          expression,
          value: "release",
        });
      }
    )
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "trust-lsp.debug.ensureConfiguration",
      async () => {
        await ensureConfigurationEntry();
      }
    )
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.debug.reload", async () => {
      const session = vscode.debug.activeDebugSession;
      if (!session || session.type !== DEBUG_TYPE) {
        const message = "No active Structured Text debug session to reload.";
        vscode.window.showErrorMessage(
          message
        );
        return { ok: false, message };
      }
      const gateReason = diagnosticsGateReason(validityLine(), "update");
      if (gateReason) {
        vscode.window.showWarningMessage(gateReason);
        debugReloadEmitter.fire({ ok: false, message: gateReason, gated: true });
        return { ok: false, message: gateReason, gated: true };
      }

      const config = session.configuration ?? {};
      const program =
        typeof config.program === "string" ? config.program : undefined;
      const preferred = preferredStructuredTextUri();
      const activeFile = preferred?.fsPath;

      try {
        const target =
          program && program.trim().length > 0
            ? vscode.Uri.file(program)
            : preferred;
        const runtimeOptions = runtimeSourceOptions(target);
        await withTimeout(
          () => session.customRequest("stReload", {
            program: program ?? activeFile,
            ...runtimeOptions,
          }),
          HOT_RELOAD_REQUEST_TIMEOUT_MS,
          "Update running simulation timed out. The simulator may still be busy; try again or restart it."
        );
        debugReloadEmitter.fire({ ok: true });
        return { ok: true };
      } catch (err) {
        const rawMessage = err instanceof Error ? err.message : String(err);
        const message = summarizeReloadCommandMessage(rawMessage);
        vscode.window.showErrorMessage(`Hot reload failed: ${message}`);
        debugReloadEmitter.fire({ ok: false, message });
        return { ok: false, message };
      }
    })
  );
}

export {
  __testCreateDefaultConfigurationAuto,
  __testEnsureConfigurationEntryAuto,
  selectWorkspaceFolderPathForMode,
};
