import * as vscode from "vscode";

import { debugChannel, DEBUG_TYPE } from "./debug/configuration";
import { runtimeSourceOptionsForTarget } from "./runtimeSourceOptions";
import { getControlAuthToken } from "./runtimeAuth";
import {
  classifyRuntimeStartFailure,
  type NetworkCanvasRuntimeFailure as RuntimeStartFailure,
  type NetworkCanvasRuntimeFailureKind as RuntimeStartFailureKind,
} from "./networkCanvas/runtimeFailures";
import {
  probeEndpointReachable,
  runtimeStatusPayload,
} from "./io-panel/status";
import type { IoState, RuntimeStatusPayload } from "./io-panel/types";

const SESSION_WAIT_TIMEOUT_MS = 8000;
const SESSION_WAIT_POLL_MS = 100;

export type { RuntimeStartFailure, RuntimeStartFailureKind };

export type RuntimeLifecycleResult =
  | { readonly ok: true; readonly message: string }
  | { readonly ok: false; readonly failure: RuntimeStartFailure };

export type RuntimeLifecycleSnapshot = {
  readonly status: RuntimeStatusPayload;
  readonly ioState: IoState;
  readonly starting: boolean;
  readonly failure?: RuntimeStartFailure;
};

const EMPTY_IO_STATE: IoState = { inputs: [], outputs: [], memory: [] };

class RuntimeLifecycleService {
  private readonly sessions = new Map<string, vscode.DebugSession>();
  private readonly changeEmitter = new vscode.EventEmitter<void>();
  private registered = false;
  private lastIoState: IoState = EMPTY_IO_STATE;
  private starting = false;
  private failure: RuntimeStartFailure | undefined;

  readonly onDidChange = this.changeEmitter.event;

  register(context: vscode.ExtensionContext): void {
    if (this.registered) {
      return;
    }
    this.registered = true;

    const activeSession = vscode.debug.activeDebugSession;
    if (activeSession && activeSession.type === DEBUG_TYPE) {
      this.trackStructuredTextSession(activeSession);
    }

    context.subscriptions.push(
      vscode.debug.onDidReceiveDebugSessionCustomEvent((event) => {
        if (event.event !== "stIoState" || event.session.type !== DEBUG_TYPE) {
          return;
        }
        this.lastIoState = normalizeIoState(event.body);
        this.emitChanged();
      })
    );

    context.subscriptions.push(
      vscode.debug.onDidStartDebugSession((session) => {
        if (session.type !== DEBUG_TYPE) {
          return;
        }
        this.trackStructuredTextSession(session);
        this.failure = undefined;
        this.starting = false;
        void this.requestIoState();
        this.emitChanged();
      })
    );

    context.subscriptions.push(
      vscode.debug.onDidTerminateDebugSession((session) => {
        if (session.type !== DEBUG_TYPE) {
          return;
        }
        this.untrackStructuredTextSession(session);
        if (!this.getStructuredTextSession()) {
          this.lastIoState = EMPTY_IO_STATE;
        }
        this.starting = false;
        this.emitChanged();
      })
    );

    context.subscriptions.push(
      vscode.debug.onDidChangeActiveDebugSession((session) => {
        if (session && session.type === DEBUG_TYPE) {
          this.trackStructuredTextSession(session);
          void this.requestIoState();
        }
        this.emitChanged();
      })
    );

    context.subscriptions.push(
      vscode.workspace.onDidChangeConfiguration((event) => {
        if (
          event.affectsConfiguration("trust-lsp.runtime.controlEndpoint") ||
          event.affectsConfiguration("trust-lsp.runtime.controlEndpointEnabled") ||
          event.affectsConfiguration("trust-lsp.runtime.inlineValuesEnabled") ||
          event.affectsConfiguration("trust-lsp.runtime.mode")
        ) {
          this.emitChanged();
        }
      })
    );
  }

  getStructuredTextSession(): vscode.DebugSession | undefined {
    const active = vscode.debug.activeDebugSession;
    if (active && active.type === DEBUG_TYPE) {
      return active;
    }
    for (const session of this.sessions.values()) {
      return session;
    }
    return undefined;
  }

  runtimeConfigTarget(): vscode.Uri | undefined {
    const activeSession = this.getStructuredTextSession();
    if (activeSession?.workspaceFolder) {
      return activeSession.workspaceFolder.uri;
    }
    const editor = vscode.window.activeTextEditor;
    if (editor) {
      const folder = vscode.workspace.getWorkspaceFolder(editor.document.uri);
      if (folder) {
        return folder.uri;
      }
    }
    return vscode.workspace.workspaceFolders?.[0]?.uri;
  }

  runtimeConfigScope(
    target: vscode.Uri | undefined
  ): vscode.ConfigurationTarget {
    return target
      ? vscode.ConfigurationTarget.WorkspaceFolder
      : vscode.ConfigurationTarget.Workspace;
  }

  async snapshot(): Promise<RuntimeLifecycleSnapshot> {
    return {
      status: await runtimeStatusPayload({
        runtimeConfigTarget: () => this.runtimeConfigTarget(),
        getStructuredTextSession: () => this.getStructuredTextSession(),
      }),
      ioState: this.lastIoState,
      starting: this.starting,
      failure: this.failure,
    };
  }

  async requestIoState(): Promise<RuntimeLifecycleResult> {
    const session = this.getStructuredTextSession();
    if (!session) {
      return {
        ok: false,
        failure: {
          kind: "stale_runtime",
          message: "No active Structured Text debug session.",
        },
      };
    }
    try {
      await session.customRequest("stIoState");
      return { ok: true, message: "I/O state requested." };
    } catch (err) {
      const failure = classifyRuntimeStartFailure(err);
      this.failure = {
        ...failure,
        message: `I/O state request failed: ${failure.message}`,
      };
      this.emitChanged();
      return { ok: false, failure: this.failure };
    }
  }

  async setRuntimeMode(mode: unknown): Promise<void> {
    const normalized = mode === "online" ? "online" : "simulate";
    const target = this.runtimeConfigTarget();
    const config = vscode.workspace.getConfiguration("trust-lsp", target);
    await config.update("runtime.mode", normalized, this.runtimeConfigScope(target));
    this.emitChanged();
  }

  async startRuntime(): Promise<RuntimeLifecycleResult> {
    const status = await this.snapshot();
    if (status.status.runtimeMode === "online") {
      return this.startOnlineRuntime(status.status);
    }
    return this.startLocalSimulator();
  }

  // Connect (attach) to a configured remote runtime by its control endpoint. Points the runtime at
  // that endpoint, switches to online mode, then attaches. Honest: this is a "Connect", never a remote
  // "Start" — we attach to a runtime we don't own.
  async connectRemote(endpoint: string): Promise<RuntimeLifecycleResult> {
    const trimmed = endpoint.trim();
    if (!trimmed) {
      return {
        ok: false,
        failure: { kind: "failed_spawn", message: "Runtime endpoint not set." },
      };
    }
    const target = this.runtimeConfigTarget();
    const config = vscode.workspace.getConfiguration("trust-lsp", target);
    const scope = this.runtimeConfigScope(target);
    await config.update("runtime.controlEndpoint", trimmed, scope);
    await config.update("runtime.controlEndpointEnabled", true, scope);
    await config.update("runtime.mode", "online", scope);
    this.emitChanged();
    return this.startRuntime();
  }

  async startLocalSimulator(): Promise<RuntimeLifecycleResult> {
    this.starting = true;
    this.failure = undefined;
    this.emitChanged();
    try {
      await this.setRuntimeMode("simulate");
      const started = await vscode.commands.executeCommand<boolean>(
        "trust-lsp.debug.start"
      );
      if (!started) {
        throw new Error("Start debugging did not launch a local simulator session.");
      }
      const session = await this.waitForStructuredTextSession(
        SESSION_WAIT_TIMEOUT_MS
      );
      if (!session) {
        throw new Error(
          "Timed out waiting for the local simulator debug session."
        );
      }
      await this.requestIoState();
      this.starting = false;
      this.failure = undefined;
      this.emitChanged();
      return { ok: true, message: "Local simulator running." };
    } catch (err) {
      this.starting = false;
      this.failure = classifyRuntimeStartFailure(err);
      debugChannel().appendLine(
        `Simulator start failed: ${this.failure.message}`
      );
      this.emitChanged();
      return { ok: false, failure: this.failure };
    }
  }

  async stopRuntime(): Promise<RuntimeLifecycleResult> {
    const activeSession = this.getStructuredTextSession();
    if (activeSession) {
      // `trust-lsp.debug.stop` calls vscode.debug.stopDebugging(), which resolves to `void`,
      // NOT `true`, on a successful stop. Success is therefore verified by the session actually
      // going away — never by the command's return value. Stop is idempotent: a session that has
      // already disappeared after Stop is treated as SUCCESS, never as a stale-session warning.
      try {
        await vscode.commands.executeCommand("trust-lsp.debug.stop");
      } catch (err) {
        if (!this.getStructuredTextSession()) {
          return { ok: true, message: "Runtime stopped." };
        }
        return { ok: false, failure: classifyRuntimeStartFailure(err) };
      }
      if (await this.waitForSessionGone(SESSION_WAIT_TIMEOUT_MS)) {
        return { ok: true, message: "Runtime stopped." };
      }
      return {
        ok: false,
        failure: {
          kind: "stale_runtime",
          message: "Runtime did not stop. Check the Structured Text debug session.",
        },
      };
    }

    const snapshot = await this.snapshot();
    if (snapshot.status.runtimeState === "connected") {
      const target = this.runtimeConfigTarget();
      const config = vscode.workspace.getConfiguration("trust-lsp", target);
      await config.update(
        "runtime.controlEndpointEnabled",
        false,
        this.runtimeConfigScope(target)
      );
      this.emitChanged();
      return { ok: true, message: "Runtime endpoint disabled." };
    }
    // Idempotent: nothing is running, so Stop is a no-op success (no warning).
    return { ok: true, message: "Runtime already stopped." };
  }

  private async startOnlineRuntime(
    status: RuntimeStatusPayload
  ): Promise<RuntimeLifecycleResult> {
    const target = this.runtimeConfigTarget();
    const config = vscode.workspace.getConfiguration("trust-lsp", target);
    if (!status.endpointConfigured) {
      return {
        ok: false,
        failure: {
          kind: "failed_spawn",
          message: "Runtime endpoint not set.",
        },
      };
    }

    if (!status.endpointEnabled) {
      await config.update(
        "runtime.controlEndpointEnabled",
        true,
        this.runtimeConfigScope(target)
      );
    }

    const reachable = await probeEndpointReachable(status.endpoint);
    if (!reachable) {
      return {
        ok: false,
        failure: {
          kind: "stale_runtime",
          message: `Runtime not reachable: ${status.endpoint}`,
        },
      };
    }

    // §0.6.8 — token from SecretStorage first (legacy setting fallback), never plaintext-only.
    const authToken = (await getControlAuthToken(status.endpoint)) ?? "";
    const runtimeOptions = runtimeSourceOptionsForTarget();
    const folder = vscode.workspace.workspaceFolders?.[0];
    const debugConfig: vscode.DebugConfiguration = {
      type: DEBUG_TYPE,
      request: "attach",
      name: "Attach Structured Text",
      endpoint: status.endpoint,
      authToken: authToken || undefined,
      ...runtimeOptions,
    };
    if (folder) {
      debugConfig.cwd = folder.uri.fsPath;
    }
    try {
      const started = await vscode.debug.startDebugging(folder, debugConfig);
      if (!started) {
        throw new Error("Attach failed to start.");
      }
      return { ok: true, message: "Attached to runtime." };
    } catch (err) {
      return { ok: false, failure: classifyRuntimeStartFailure(err) };
    }
  }

  private async waitForStructuredTextSession(
    timeoutMs: number
  ): Promise<vscode.DebugSession | undefined> {
    const startedAt = Date.now();
    while (Date.now() - startedAt < timeoutMs) {
      const session = this.getStructuredTextSession();
      if (session) {
        return session;
      }
      await new Promise((resolve) => setTimeout(resolve, SESSION_WAIT_POLL_MS));
    }
    return this.getStructuredTextSession();
  }

  // Returns true once no Structured Text debug session remains (the terminate event has landed and
  // cleared our tracking map). Used by stopRuntime to verify a stop honestly instead of trusting the
  // command's void return value.
  private async waitForSessionGone(timeoutMs: number): Promise<boolean> {
    const startedAt = Date.now();
    while (Date.now() - startedAt < timeoutMs) {
      if (!this.getStructuredTextSession()) {
        return true;
      }
      await new Promise((resolve) => setTimeout(resolve, SESSION_WAIT_POLL_MS));
    }
    return !this.getStructuredTextSession();
  }

  private trackStructuredTextSession(session: vscode.DebugSession): void {
    this.sessions.set(structuredTextSessionKey(session), session);
  }

  private untrackStructuredTextSession(session: vscode.DebugSession): void {
    this.sessions.delete(structuredTextSessionKey(session));
  }

  private emitChanged(): void {
    this.changeEmitter.fire();
  }
}

export const runtimeLifecycleService = new RuntimeLifecycleService();

export function registerRuntimeLifecycle(context: vscode.ExtensionContext): void {
  runtimeLifecycleService.register(context);
}

function structuredTextSessionKey(session: vscode.DebugSession): string {
  return session.id ?? session.name;
}

function normalizeIoState(value: unknown): IoState {
  if (!isRecord(value)) {
    return EMPTY_IO_STATE;
  }
  return {
    inputs: normalizeIoEntries(value.inputs),
    outputs: normalizeIoEntries(value.outputs),
    memory: normalizeIoEntries(value.memory),
  };
}

function normalizeIoEntries(value: unknown): IoState["inputs"] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .map((entry): IoState["inputs"][number] | undefined => {
      if (!isRecord(entry)) {
        return undefined;
      }
      const address = typeof entry.address === "string" ? entry.address : "";
      const rawValue = typeof entry.value === "string" ? entry.value : "";
      if (!address || rawValue.length === 0) {
        return undefined;
      }
      const normalized: IoState["inputs"][number] = {
        address,
        value: rawValue,
        forced: entry.forced === true,
      };
      if (typeof entry.name === "string") {
        normalized.name = entry.name;
      }
      return normalized;
    })
    .filter((entry): entry is IoState["inputs"][number] => entry !== undefined);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
