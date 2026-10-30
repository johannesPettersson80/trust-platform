import * as vscode from "vscode";

import { affectsTrustConfiguration, getTrustConfiguration } from "./configuration";
import { debugChannel, DEBUG_TYPE } from "./debug/configuration";
import { runtimeSourceOptionsForTarget } from "./runtimeSourceOptions";
import { getControlAuthToken } from "./runtimeAuth";
import { resolveProjectConnection, resolveRuntimeConnection } from "./runtimeConnection";
import {
  classifyRuntimeStartFailure,
  type NetworkCanvasRuntimeFailure as RuntimeStartFailure,
  type NetworkCanvasRuntimeFailureKind as RuntimeStartFailureKind,
} from "./networkCanvas/runtimeFailures";
import { runtimeStatusPayload } from "./io-panel/status";
import type { IoState, RuntimeStatusPayload } from "./io-panel/types";
import { projectRequest } from "./projectRequest";
import { toModelSnapshot } from "./trustHomeModel";

const SESSION_WAIT_TIMEOUT_MS = 8000;
const SESSION_WAIT_POLL_MS = 100;
const SESSION_START_STABILITY_MS = 2500;
const SIMULATOR_LAUNCH_TIMEOUT_MS = 15_000;
const IO_STATE_REQUEST_TIMEOUT_MS = 5_000;
const IO_NEXT_SCAN_TIMEOUT_MS = 1200;
const IO_NEXT_SCAN_POLL_MS = 60;
// While the runtime settings are online and no session is attached, the status is read again at this interval, so that
// a runtime started or stopped outside VS Code shows in the side panel and the status bar (specification 25, section
// 5.1). An attached session reports its own end.
const ONLINE_WATCH_INTERVAL_MS = 2000;

export type { RuntimeStartFailure, RuntimeStartFailureKind };

export type RuntimeLifecycleResult =
  | { readonly ok: true; readonly message: string }
  | { readonly ok: false; readonly failure: RuntimeStartFailure };

// The runtime a Connect attempt talks to: its endpoint and token, or why it has none, and whether it is the
// workspace project's own runtime, which trust-runtime finds at each attempt (spec 11 §6.8.0a).
type OnlineConnection = {
  readonly endpoint: string;
  readonly authToken?: string;
  readonly problem?: string;
  readonly followsProject: boolean;
};

// The language server's answer to a Connect (`trust-lsp.runTargets`, action `connect`).
type ConnectAnswer =
  | { readonly outcome: "attach"; readonly sessionName: string }
  | { readonly outcome: "debugDisabled"; readonly failure: RuntimeStartFailure }
  | { readonly outcome: "failed"; readonly failure: RuntimeStartFailure };

export type RuntimeLifecycleSnapshot = {
  readonly sessionId?: string;
  readonly status: RuntimeStatusPayload;
  readonly ioState: IoState;
  readonly starting: boolean;
  readonly failure?: RuntimeStartFailure;
};

export function runtimeSessionControlAuthToken(
  session: Pick<vscode.DebugSession, "configuration"> | undefined,
  endpoint: string
): string | undefined {
  const configuration = session?.configuration;
  const sessionEndpoint = configuration?.request === "attach"
    ? configuration.endpoint
    : configuration?.controlEndpoint;
  const token = configuration?.request === "attach"
    ? configuration.authToken
    : configuration?.controlAuthToken;
  return typeof sessionEndpoint === "string" && sessionEndpoint.trim() === endpoint.trim() &&
    typeof token === "string" && token.trim()
    ? token.trim()
    : undefined;
}

const EMPTY_IO_STATE: IoState = { inputs: [], outputs: [], memory: [], ads: [] };

class RuntimeLifecycleService {
  private readonly sessions = new Map<string, vscode.DebugSession>();
  private readonly changeEmitter = new vscode.EventEmitter<void>();
  private readonly ioStateChangeEmitter = new vscode.EventEmitter<IoState>();
  private registered = false;
  private readonly ioStates = new Map<string, { state: IoState; highWaterScan?: number }>();
  private get lastIoState(): IoState {
    const session = this.getStructuredTextSession();
    return session ? this.ioStates.get(structuredTextSessionKey(session))?.state ?? EMPTY_IO_STATE : EMPTY_IO_STATE;
  }
  private starting = false;
  private failure: RuntimeStartFailure | undefined;

  readonly onDidChange = this.changeEmitter.event;
  readonly onDidIoStateChange = this.ioStateChangeEmitter.event;

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
        const key = structuredTextSessionKey(event.session);
        if (!this.sessions.has(key)) return;
        const state = normalizeIoState(event.body);
        const previous = this.ioStates.get(key);
        if (state.scan !== undefined && previous?.highWaterScan !== undefined && state.scan < previous.highWaterScan) return;
        this.ioStates.set(key, { state, highWaterScan: state.scan ?? previous?.highWaterScan });
        const active = this.getStructuredTextSession();
        if (active && structuredTextSessionKey(active) === key) this.emitIoStateChanged();
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
          this.ioStates.clear();
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

    this.watchOnline(context);

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

  // Reads the status again while online without a session and tells the surfaces when what the language server reported
  // of the runtime changed: its endpoint, whether it answers, and the project's runtime.
  private watchOnline(context: vscode.ExtensionContext): void {
    let reported = "";
    let reading = false;
    const timer = setInterval(() => {
      const online = getTrustConfiguration(this.runtimeConfigTarget()).get<string>("runtime.mode", "simulate") === "online";
      if (!online || this.getStructuredTextSession()) {
        reported = "";
        return;
      }
      if (reading) return;
      reading = true;
      void this.snapshot()
        .then(({ status }) => {
          const now = JSON.stringify([status.endpoint, status.endpointReachable, status.projectEndpoint, status.projectReachable]);
          if (reported && now !== reported) this.emitChanged();
          reported = now;
        })
        .catch(() => undefined)
        .finally(() => {
          reading = false;
        });
    }, ONLINE_WATCH_INTERVAL_MS);
    context.subscriptions.push({ dispose: () => clearInterval(timer) });
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
    const session = this.getStructuredTextSession();
    return {
      sessionId: session?.id,
      status: await runtimeStatusPayload({
        runtimeConfigTarget: () => this.runtimeConfigTarget(),
        getStructuredTextSession: () => session,
      }),
      ioState: this.lastIoState,
      starting: this.starting,
      failure: this.failure,
    };
  }

  async controlAuthToken(endpoint: string): Promise<string | undefined> {
    return runtimeSessionControlAuthToken(this.getStructuredTextSession(), endpoint) ??
      await getControlAuthToken(endpoint);
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
      await withTimeout(
        session.customRequest(
          "stIoState",
          options.afterScan === undefined ? undefined : { afterScan: options.afterScan }
        ),
        IO_STATE_REQUEST_TIMEOUT_MS,
        "I/O state request timed out."
      );
      return { ok: true, message: "I/O state requested." };
    } catch (err) {
      const failure = await classifyRuntimeStartFailure(err);
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
      // Spec 11 §6.8.0a — the endpoint and token are resolved again for this connection attempt, so a
      // workspace runtime that restarted on another port is found. An endpoint named in the settings is used as
      // it is, with the token stored for it (§0.6.8, legacy setting fallback, never plaintext-only).
      const target = this.runtimeConfigTarget();
      const named = getTrustConfiguration(target).get<string>("runtime.controlEndpoint", "").trim();
      const connection = await resolveRuntimeConnection(target);
      return this.startOnlineRuntime(
        {
          endpoint:
            connection.endpoint ?? (status.status.endpointConfigured ? status.status.endpoint : ""),
          authToken: connection.authToken,
          problem: connection.problem?.message,
          followsProject: !named,
        },
        targetLabel
      );
    }
    return this.startLocalSimulator();
  }

  // Connect (attach) to a runtime by its control endpoint. Honest: this is a "Connect", never a remote
  // "Start" — we attach to a runtime we don't own. The runtime is asked first; the runtime settings point
  // at it only after a successful Connect (see startOnlineRuntime).
  async connectRemote(
    endpoint: string,
    targetLabel?: string
  ): Promise<RuntimeLifecycleResult> {
    const trimmed = endpoint.trim();
    // §0.6.8 — the token stored for that endpoint (legacy setting fallback), never plaintext-only. A blank endpoint
    // is the language server's to refuse.
    return this.startOnlineRuntime(
      { endpoint: trimmed, authToken: await getControlAuthToken(trimmed), followsProject: false },
      targetLabel
    );
  }

  // Connect to the workspace project's own runtime (specification 25, section 5.1): its endpoint and token are what
  // trust-runtime reports for the project at this attempt, as Start in online mode asks them, so a runtime that
  // restarted on another port is found, and a successful Connect leaves `runtime.controlEndpoint` empty.
  async connectProjectRuntime(targetLabel?: string): Promise<RuntimeLifecycleResult> {
    const connection = await resolveProjectConnection(this.runtimeConfigTarget());
    return this.startOnlineRuntime(
      {
        endpoint: connection.endpoint ?? "",
        authToken: connection.authToken,
        problem: connection.problem?.message,
        followsProject: true,
      },
      targetLabel
    );
  }

  // The reason an online target has no session, when the lifecycle reported it for `endpoint`: the
  // runtime answered Connect but keeps debugging disabled.
  debugDisabledFailure(endpoint: string): RuntimeStartFailure | undefined {
    const failure = this.failure;
    return failure?.kind === "debug_disabled" && failure.endpoint === endpoint.trim()
      ? failure
      : undefined;
  }

  async startLocalSimulator(): Promise<RuntimeLifecycleResult> {
    this.starting = true;
    this.failure = undefined;
    this.emitChanged();
    try {
      await this.setRuntimeMode("simulate");
      const session = await this.waitForStructuredTextSessionOrCommand(
        vscode.commands.executeCommand<boolean>("trust-lsp.debug.start"),
        SIMULATOR_LAUNCH_TIMEOUT_MS
      );
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
      this.failure = await classifyRuntimeStartFailure(err);
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
        return { ok: false, failure: await classifyRuntimeStartFailure(err) };
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

    // What Stop does without a session is the language server's (`trust-lsp.runTargets`, action `stop`, specification
    // 25 §5.1): a runtime connected without a session is left by turning the control endpoint off; with none, Stop is
    // a success that changes nothing.
    const answer = await projectRequest<{ settings: "disableEndpoint" | "keep"; message: string }>(
      "trust-lsp.runTargets",
      { action: "stop", snapshot: toModelSnapshot(await this.snapshot()) }
    );
    if (!answer.ok) {
      return { ok: false, failure: { kind: "stale_runtime", message: answer.error } };
    }
    if (answer.settings === "disableEndpoint") {
      const target = this.runtimeConfigTarget();
      const config = getTrustConfiguration(target);
      await config.update(
        "runtime.controlEndpointEnabled",
        false,
        this.runtimeConfigScope(target)
      );
      this.emitChanged();
      return { ok: true, message: answer.message };
    }
    return this.markStopped(answer.message);
  }

  // Connect changes nothing until it succeeds (specification 25, section 5.1): the runtime must be
  // reachable and answer `status` with the endpoint's stored token, and then either open an attach
  // session or report debugging disabled (it is then the target without Live Values). Only a successful
  // Connect points the runtime settings at it; a failed one leaves the previous target, settings and
  // status bar as they were, and its failure names the endpoint so the runtime's node can show it.
  private async startOnlineRuntime(
    connection: OnlineConnection,
    targetLabel?: string
  ): Promise<RuntimeLifecycleResult> {
    // The probe, the `status` request with the endpoint's token and the reading of its answer are the language
    // server's (`trust-lsp.runTargets`, action `connect`, specification 25 §5.1 and §6.4).
    const answer = await projectRequest<ConnectAnswer>("trust-lsp.runTargets", {
      action: "connect",
      endpoint: connection.endpoint,
      authToken: connection.authToken,
      problem: connection.problem,
      targetLabel,
    });
    if (!answer.ok) {
      return { ok: false, failure: { kind: "stale_runtime", message: answer.error } };
    }
    if (answer.outcome === "failed") {
      return { ok: false, failure: answer.failure };
    }
    const endpoint = connection.endpoint.trim();
    if (answer.outcome === "debugDisabled") {
      // The runtime answered, so it is the target; Live Values says why it has no session.
      this.failure = answer.failure;
      await this.useConnectedRuntime(endpoint, connection.followsProject);
      return { ok: true, message: answer.failure.message };
    }
    const authToken = connection.authToken ?? "";
    const runtimeOptions = runtimeSourceOptionsForTarget();
    const folder = vscode.workspace.workspaceFolders?.[0];
    const debugConfig: vscode.DebugConfiguration = {
      type: DEBUG_TYPE,
      request: "attach",
      name: answer.sessionName,
      endpoint,
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
    } catch (err) {
      return {
        ok: false,
        failure: { ...(await classifyRuntimeStartFailure(err)), endpoint },
      };
    }
    await this.useConnectedRuntime(endpoint, connection.followsProject);
    return { ok: true, message: "Attached to runtime." };
  }

  // The workspace project's runtime is asked of trust-runtime at each connection attempt (spec 11 §6.8.0a), so a
  // successful Connect to it leaves `runtime.controlEndpoint` empty and the next attempt follows a restart on
  // another port. Any other runtime is named there (specification 25, section 5.1).
  private async useConnectedRuntime(endpoint: string, followsProject: boolean): Promise<void> {
    if (!followsProject) {
      await this.useOnlineEndpoint(endpoint);
      return;
    }
    const target = this.runtimeConfigTarget();
    const config = getTrustConfiguration(target);
    const scope = this.runtimeConfigScope(target);
    if (!config.get<boolean>("runtime.controlEndpointEnabled", true)) {
      await config.update("runtime.controlEndpointEnabled", true, scope);
    }
    if (config.get<string>("runtime.mode", "simulate") !== "online") {
      await config.update("runtime.mode", "online", scope);
    }
    this.emitChanged();
  }

  // A successful Connect, or selecting a runtime as the run target, points the runtime settings at it: its
  // endpoint, in use, in online mode. An empty endpoint follows the workspace project's own runtime, whose endpoint each
  // connection asks of trust-runtime (spec 11 §6.8.0a). Only values that differ are written.
  async useOnlineEndpoint(endpoint: string): Promise<void> {
    const target = this.runtimeConfigTarget();
    const config = getTrustConfiguration(target);
    const scope = this.runtimeConfigScope(target);
    if (config.get<string>("runtime.controlEndpoint", "").trim() !== endpoint) {
      await config.update("runtime.controlEndpoint", endpoint, scope);
    }
    if (!config.get<boolean>("runtime.controlEndpointEnabled", true)) {
      await config.update("runtime.controlEndpointEnabled", true, scope);
    }
    if (config.get<string>("runtime.mode", "simulate") !== "online") {
      await config.update("runtime.mode", "online", scope);
    }
    this.emitChanged();
  }

  private async waitForStructuredTextSessionOrCommand(
    command: Thenable<boolean>,
    timeoutMs: number
  ): Promise<vscode.DebugSession> {
    type CommandOutcome =
      | { readonly status: "pending" }
      | { readonly status: "resolved"; readonly started: boolean }
      | { readonly status: "rejected"; readonly error: unknown };
    const commandOutcome: { current: CommandOutcome } = {
      current: { status: "pending" },
    };
    void Promise.resolve(command).then(
      (started) => {
        commandOutcome.current = { status: "resolved", started };
      },
      (error: unknown) => {
        commandOutcome.current = { status: "rejected", error };
      }
    );

    const startedAt = Date.now();
    while (Date.now() - startedAt < timeoutMs) {
      const session = this.getStructuredTextSession();
      if (session) {
        return session;
      }
      const outcome = commandOutcome.current;
      if (outcome.status === "rejected") {
        throw outcome.error;
      }
      if (outcome.status === "resolved" && !outcome.started) {
        throw new Error("Start debugging did not launch a Simulator session.");
      }
      await new Promise((resolve) => setTimeout(resolve, SESSION_WAIT_POLL_MS));
    }
    const session = this.getStructuredTextSession();
    if (session) {
      return session;
    }
    const outcome = commandOutcome.current;
    if (outcome.status === "rejected") {
      throw outcome.error;
    }
    if (outcome.status === "resolved" && !outcome.started) {
      throw new Error("Start debugging did not launch a Simulator session.");
    }
    throw new Error("Timed out waiting for the Simulator debug session.");
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
    this.ioStates.delete(structuredTextSessionKey(session));
  }

  // A failure belongs to the target it was reported for; selecting another target forgets it.
  clearFailure(): void {
    if (this.failure) {
      this.failure = undefined;
      this.emitChanged();
    }
  }

  private markStopped(message: string): RuntimeLifecycleResult {
    if (!this.getStructuredTextSession()) this.ioStates.clear();
    this.starting = false;
    this.failure = undefined;
    this.emitChanged();
    return { ok: true, message };
  }

  private emitChanged(): void {
    this.changeEmitter.fire();
  }

  private emitIoStateChanged(): void {
    this.ioStateChangeEmitter.fire(this.lastIoState);
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

/** The I/O state that the debug adapter's `stIoState` event carries. The adapter writes it as Live Values shows it: values in
 * IEC text, types in capitals, the ADS tags apart (trust-debug `adapter/io_values.rs`, specification 25 §4.3); this keeps
 * only the entries of the event's shape. */
export function normalizeIoState(value: unknown): IoState {
  if (!isRecord(value)) {
    return EMPTY_IO_STATE;
  }
  const entries = (list: unknown): IoState["inputs"] =>
    Array.isArray(list) ? list.filter(isIoEntry) : [];
  return {
    scan: typeof value.scan === "number" && Number.isFinite(value.scan) ? value.scan : undefined,
    inputs: entries(value.inputs),
    outputs: entries(value.outputs),
    memory: entries(value.memory),
    ads: entries(value.ads),
  };
}

function isIoEntry(entry: unknown): entry is IoState["inputs"][number] {
  return isRecord(entry) && typeof entry.address === "string" && typeof entry.value === "string";
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
