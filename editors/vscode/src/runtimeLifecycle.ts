import * as vscode from "vscode";

import { affectsTrustConfiguration, getTrustConfiguration } from "./configuration";
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
import {
  isRuntimeControlAuthError,
  requestRuntimeStatus,
  runtimeControlAuthErrorKind,
} from "./runtimeControlClient";

const SESSION_WAIT_TIMEOUT_MS = 8000;
const SESSION_WAIT_POLL_MS = 100;
const SESSION_START_STABILITY_MS = 2500;
const DEBUG_START_COMMAND_TIMEOUT_MS = 6000;
const IO_NEXT_SCAN_TIMEOUT_MS = 1200;
const IO_NEXT_SCAN_POLL_MS = 60;

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
          affectsTrustConfiguration(event, "runtime.controlEndpoint") ||
          affectsTrustConfiguration(event, "runtime.controlEndpointEnabled") ||
          affectsTrustConfiguration(event, "runtime.inlineValuesEnabled") ||
          affectsTrustConfiguration(event, "runtime.mode")
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

  async requestIoState(options: { readonly persistFailure?: boolean; readonly afterScan?: number } = {}): Promise<RuntimeLifecycleResult> {
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
      await session.customRequest(
        "stIoState",
        options.afterScan === undefined ? undefined : { afterScan: options.afterScan }
      );
      return { ok: true, message: "I/O state requested." };
    } catch (err) {
      const failure = classifyRuntimeStartFailure(err);
      const ioFailure = {
        ...failure,
        message: `I/O state request failed: ${failure.message}`,
      };
      if (options.persistFailure) {
        this.failure = ioFailure;
        this.emitChanged();
      }
      return { ok: false, failure: ioFailure };
    }
  }

  async requestIoStateAfterScan(
    previousScan: number | undefined,
    options: { readonly timeoutMs?: number } = {}
  ): Promise<RuntimeLifecycleResult> {
    const deadline = Date.now() + (options.timeoutMs ?? IO_NEXT_SCAN_TIMEOUT_MS);
    let lastResult: RuntimeLifecycleResult = {
      ok: true,
      message: "I/O state requested.",
    };
    do {
      lastResult = await this.requestIoState({ afterScan: previousScan });
      if (!lastResult.ok) {
        return lastResult;
      }
      const nextScan = this.lastIoState.scan;
      if (
        previousScan === undefined ||
        nextScan === undefined ||
        nextScan > previousScan
      ) {
        return lastResult;
      }
      await delay(IO_NEXT_SCAN_POLL_MS);
    } while (Date.now() < deadline);
    return lastResult;
  }

  async setRuntimeMode(mode: unknown): Promise<void> {
    const normalized = mode === "online" ? "online" : "simulate";
    const target = this.runtimeConfigTarget();
    const config = getTrustConfiguration(target);
    await config.update("runtime.mode", normalized, this.runtimeConfigScope(target));
    this.emitChanged();
  }

  async startRuntime(targetLabel?: string): Promise<RuntimeLifecycleResult> {
    const status = await this.snapshot();
    if (status.status.runtimeMode === "online") {
      return this.startOnlineRuntime(status.status, targetLabel);
    }
    return this.startLocalSimulator();
  }

  // Connect (attach) to a configured remote runtime by its control endpoint. Points the runtime at
  // that endpoint, switches to online mode, then attaches. Honest: this is a "Connect", never a remote
  // "Start" — we attach to a runtime we don't own.
  async connectRemote(
    endpoint: string,
    targetLabel?: string
  ): Promise<RuntimeLifecycleResult> {
    const trimmed = endpoint.trim();
    if (!trimmed) {
      return {
        ok: false,
        failure: { kind: "failed_spawn", message: "Runtime endpoint not set." },
      };
    }
    const target = this.runtimeConfigTarget();
    const config = getTrustConfiguration(target);
    const scope = this.runtimeConfigScope(target);
    await config.update("runtime.controlEndpoint", trimmed, scope);
    await config.update("runtime.controlEndpointEnabled", true, scope);
    await config.update("runtime.mode", "online", scope);
    this.emitChanged();
    return this.startRuntime(targetLabel);
  }

  async startLocalSimulator(): Promise<RuntimeLifecycleResult> {
    this.starting = true;
    this.failure = undefined;
    this.emitChanged();
    try {
      await this.setRuntimeMode("simulate");
      const started = await withTimeout(
        vscode.commands.executeCommand<boolean>("trust-lsp.debug.start"),
        DEBUG_START_COMMAND_TIMEOUT_MS,
        "Start debugging timed out. Check the runtime port or target settings."
      );
      if (!started) {
        throw new Error("Start debugging did not launch a Simulator session.");
      }
      const session = await this.waitForStructuredTextSession(
        SESSION_WAIT_TIMEOUT_MS
      );
      if (!session) {
        throw new Error(
          "Timed out waiting for the Simulator debug session."
        );
      }
      const ioStateResult = await this.requestIoState({ persistFailure: true });
      if (!ioStateResult.ok) {
        this.starting = false;
        this.failure = ioStateResult.failure;
        this.emitChanged();
        return ioStateResult;
      }
      if (!(await this.waitForSessionStillPresent(SESSION_START_STABILITY_MS))) {
        this.starting = false;
        this.failure = {
          kind: "failed_spawn",
          message:
            "Simulator stopped during startup. Check the runtime port or target settings.",
        };
        this.emitChanged();
        return { ok: false, failure: this.failure };
      }
      this.starting = false;
      this.failure = undefined;
      this.emitChanged();
      return { ok: true, message: "Simulator running." };
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
        if (await this.waitForSessionGone(SESSION_WAIT_TIMEOUT_MS)) {
          return this.markStopped("Runtime stopped.");
        }
        return { ok: false, failure: classifyRuntimeStartFailure(err) };
      }
      if (await this.waitForSessionGone(SESSION_WAIT_TIMEOUT_MS)) {
        return this.markStopped("Runtime stopped.");
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
      const config = getTrustConfiguration(target);
      await config.update(
        "runtime.controlEndpointEnabled",
        false,
        this.runtimeConfigScope(target)
      );
      this.emitChanged();
      return { ok: true, message: "Runtime endpoint disabled." };
    }
    // Idempotent: nothing is running, so Stop is a no-op success (no warning).
    return this.markStopped("Runtime already stopped.");
  }

  private async startOnlineRuntime(
    status: RuntimeStatusPayload,
    targetLabel?: string
  ): Promise<RuntimeLifecycleResult> {
    const target = this.runtimeConfigTarget();
    const config = getTrustConfiguration(target);
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
          message: runtimeNotReachableMessage(status.endpoint),
          detail: status.endpoint,
        },
      };
    }

    // §0.6.8 — token from SecretStorage first (legacy setting fallback), never plaintext-only.
    const authToken = (await getControlAuthToken(status.endpoint)) ?? "";
    let runtimeInfo: unknown;
    try {
      runtimeInfo = await requestRuntimeStatus(status.endpoint, authToken || undefined, {
        timeoutMs: 1000,
      });
    } catch (err) {
      if (isRuntimeControlAuthError(err)) {
        const authKind = runtimeControlAuthErrorKind(err);
        return {
          ok: false,
          failure: {
            kind: "workspace_permission",
            message:
              authKind === "missing" || !authToken
                ? "No auth token provided — this runtime requires one."
                : "Auth token rejected — check it and try again.",
          },
        };
      }
      return {
        ok: false,
        failure: {
          kind: "stale_runtime",
          message: `Runtime status check failed: ${
            err instanceof Error ? err.message : String(err)
          }`,
        },
      };
    }
    if (runtimeDebugDisabled(runtimeInfo)) {
      return {
        ok: false,
        failure: {
          kind: "failed_spawn",
          message:
            "Remote debugging is disabled for this runtime. Open Devices & Connections or ask the runtime owner to enable debugging, then connect again.",
        },
      };
    }
    const runtimeOptions = runtimeSourceOptionsForTarget();
    const folder = vscode.workspace.workspaceFolders?.[0];
    const debugConfig: vscode.DebugConfiguration = {
      type: DEBUG_TYPE,
      request: "attach",
      name: remoteDebugSessionName(targetLabel, status.endpoint),
      endpoint: status.endpoint,
      authToken: authToken || undefined,
      targetLabel,
      internalConsoleOptions: "neverOpen",
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

  private async waitForSessionStillPresent(timeoutMs: number): Promise<boolean> {
    const startedAt = Date.now();
    while (Date.now() - startedAt < timeoutMs) {
      if (!this.getStructuredTextSession()) {
        return false;
      }
      await new Promise((resolve) => setTimeout(resolve, SESSION_WAIT_POLL_MS));
    }
    return !!this.getStructuredTextSession();
  }

  private trackStructuredTextSession(session: vscode.DebugSession): void {
    this.sessions.set(structuredTextSessionKey(session), session);
  }

  private untrackStructuredTextSession(session: vscode.DebugSession): void {
    this.sessions.delete(structuredTextSessionKey(session));
  }

  private markStopped(message: string): RuntimeLifecycleResult {
    this.lastIoState = EMPTY_IO_STATE;
    this.starting = false;
    this.failure = undefined;
    this.emitChanged();
    return { ok: true, message };
  }

  private emitChanged(): void {
    this.changeEmitter.fire();
  }
}

export const runtimeLifecycleService = new RuntimeLifecycleService();

export function registerRuntimeLifecycle(context: vscode.ExtensionContext): void {
  runtimeLifecycleService.register(context);
}

async function withTimeout<T>(
  promise: Thenable<T>,
  timeoutMs: number,
  timeoutMessage: string
): Promise<T> {
  let timer: NodeJS.Timeout | undefined;
  try {
    return await Promise.race([
      promise,
      new Promise<T>((_, reject) => {
        timer = setTimeout(() => reject(new Error(timeoutMessage)), timeoutMs);
      }),
    ]);
  } finally {
    if (timer) {
      clearTimeout(timer);
    }
  }
}

function structuredTextSessionKey(session: vscode.DebugSession): string {
  return session.id ?? session.name;
}

export function normalizeIoState(value: unknown): IoState {
  if (!isRecord(value)) {
    return EMPTY_IO_STATE;
  }
  const scan =
    typeof value.scan === "number" && Number.isFinite(value.scan)
      ? value.scan
      : undefined;
  return {
    scan,
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
        value: formatIoValue(rawValue),
        forced: entry.forced === true,
      };
      if (typeof entry.name === "string") {
        normalized.name = entry.name;
      }
      if (typeof entry.source === "string") {
        normalized.source = entry.source;
      }
      const valueType =
        typeof entry.valueType === "string"
          ? entry.valueType
          : typeof entry.value_type === "string"
            ? entry.value_type
            : typeof entry.type === "string"
              ? entry.type
              : undefined;
      if (valueType) {
        normalized.valueType = valueType.toUpperCase();
      }
      return normalized;
    })
    .filter((entry): entry is IoState["inputs"][number] => entry !== undefined);
}

function formatIoValue(rawValue: string): string {
  const trimmed = rawValue.trim();
  const boolMatch = /^Bool\((true|false)\)$/i.exec(trimmed);
  if (boolMatch) {
    return boolMatch[1].toLowerCase() === "true" ? "TRUE" : "FALSE";
  }
  const simpleValue = /^(?:S?Int|DInt|LInt|U?Int|UDInt|ULInt|Real|LReal|Byte|Word|DWord|LWord)\((.*)\)$/i.exec(trimmed);
  if (simpleValue) {
    return simpleValue[1];
  }
  return rawValue;
}

function runtimeDebugDisabled(value: unknown): boolean {
  if (!isRecord(value)) {
    return false;
  }
  if (value.debug_enabled === false) {
    return true;
  }
  const controlStatus = value.control_status;
  return isRecord(controlStatus) && controlStatus.debug_enabled === false;
}

function runtimeNotReachableMessage(endpoint: string): string {
  if (endpoint.trim().startsWith("unix://")) {
    return "Local runtime is stopped. Start it to connect.";
  }
  return `Runtime is not reachable at ${shortRuntimeEndpointLabel(endpoint)}.`;
}

function remoteDebugSessionName(
  targetLabel: string | undefined,
  endpoint: string
): string {
  const label = targetLabel?.trim() || shortRuntimeEndpointLabel(endpoint);
  return label ? `truST Remote (${label})` : "truST Remote";
}

function shortRuntimeEndpointLabel(endpoint: string): string {
  const text = endpoint.trim();
  if (!text) {
    return "the configured endpoint";
  }
  if (text.startsWith("tcp://")) {
    try {
      return new URL(text).host || "the configured endpoint";
    } catch {
      return "the configured endpoint";
    }
  }
  if (text.startsWith("unix://")) {
    return "the local control socket";
  }
  return text;
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
