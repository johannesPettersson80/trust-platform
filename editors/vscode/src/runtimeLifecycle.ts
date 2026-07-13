import * as vscode from "vscode";

import { getTrustConfiguration } from "./configuration";
import { debugChannel, DEBUG_TYPE } from "./debug/configuration";
import { runtimeStatusPayload } from "./io-panel/status";
import {
  findLifecycleSessionForAttempt,
  lifecycleStartAttemptId,
  RuntimeLifecycleAttemptRegistry,
} from "./debug/startAttempt";
import {
  debugSessionAcceptancePath,
  selectLifecycleDebugSession,
} from "./debug/sessionSelection";
import {
  runtimeLifecyclePhase,
  type LifecyclePhase,
} from "./lifecycleEntryFailure";
import {
  DEBUG_START_COMMAND_TIMEOUT_MS,
  normalizeIoState,
  runtimeFailureScopeForSession,
  runtimeOperationConflict,
  runtimeOperationChangesPhase,
  runtimeTargetForSession,
  SESSION_START_STABILITY_MS,
  SESSION_WAIT_POLL_MS,
  SESSION_WAIT_TIMEOUT_MS,
  structuredTextSessionKey,
  withTimeout,
  type RuntimeLifecycleChange,
  type RuntimeLifecycleFailureScope,
  type RuntimeLifecycleOperationKind,
  type RuntimeLifecycleOperationState,
  type RuntimeLifecycleResult,
  type RuntimeLifecycleSnapshot,
  type RuntimeLifecycleTarget,
  type RuntimeStartFailure,
} from "./runtimeLifecycleModel";
import {
  RuntimeLifecycleLiveValues,
  type RuntimeIoStateRequestOptions,
} from "./runtimeLifecycleLiveValues";
import { RuntimeConfigTargetTracker } from "./runtimeConfigTargetTracker";
import { startOnlineRuntimeConnection } from "./runtimeOnlineConnection";
import type { LocalSimulatorPreparationResult } from "./localSimulatorPreparation";
import {
  coordinateLocalSimulatorStart,
  type LocalSimulatorProjectValidator,
} from "./localSimulatorStartCoordinator";
import {
  runOwnedDebugTransition,
  type OwnedDebugTransitionOutcome,
} from "./runtimeDebugTransition";
import {
  terminateRejectedSession,
  waitForLifecycleSession as waitForLifecycleSessionResult,
  waitForSessionPresence,
  waitForSessionStable,
} from "./runtimeSessionReadiness";
import { registerRuntimeLifecycleEvents } from "./runtimeLifecycleEvents";
import { runRuntimeStopOperation } from "./runtimeStopOperation";
import { selectRuntimeSessionTarget } from "./runtimeSessionAuthority";
import {
  startLocalSimulatorDebugSession,
  type LocalSimulatorDebugStart,
} from "./localSimulatorDebugStart";

export {
  isStructuralRuntimeLifecycleChange,
  normalizeIoState,
  runtimeOperationChangesPhase,
  withTimeout,
  type RuntimeLifecycleChange,
  type RuntimeLifecycleFailureScope,
  type RuntimeLifecycleOperationKind,
  type RuntimeLifecycleOperationState,
  type RuntimeLifecycleResult,
  type RuntimeLifecycleSnapshot,
  type RuntimeLifecycleTarget,
  type RuntimeStartFailure,
  type RuntimeStartFailureKind,
} from "./runtimeLifecycleModel";

type LocalRuntimeDebugStop = (
  session: vscode.DebugSession,
) => Thenable<unknown>;
type RuntimeSessionTargetSelection = (
  session: vscode.DebugSession,
) => Thenable<void>;

export type LocalSimulatorStartResult = LocalSimulatorPreparationResult;
export type RuntimeExclusiveOperationResult<T> =
  | { readonly acquired: true; readonly value: T }
  | { readonly acquired: false; readonly reason: string };

export class RuntimeLifecycleService {
  private readonly sessions = new Map<string, vscode.DebugSession>();
  private readonly acceptedSessions = new Set<string>();
  private readonly rejectedSessions = new Set<string>();
  private readonly operations = new RuntimeLifecycleAttemptRegistry();
  private readonly runtimeConfigTargets = new RuntimeConfigTargetTracker();
  private readonly changeEmitter =
    new vscode.EventEmitter<RuntimeLifecycleChange>();
  private registered = false;
  private readonly liveValues: RuntimeLifecycleLiveValues;
  private starting = false;
  private failure: RuntimeStartFailure | undefined;
  private failureScope: RuntimeLifecycleFailureScope | undefined;
  private externalTransitionTarget: RuntimeLifecycleTarget | undefined;

  constructor(
    private readonly executeLocalSimulatorDebugStart: LocalSimulatorDebugStart =
      startLocalSimulatorDebugSession,
    private readonly executeRuntimeDebugStop: LocalRuntimeDebugStop = (
      session,
    ) => vscode.debug.stopDebugging(session),
    private readonly selectAcceptedSessionTarget: RuntimeSessionTargetSelection =
      selectRuntimeSessionTarget,
    private readonly sessionStabilityMs = SESSION_START_STABILITY_MS,
  ) {
    this.liveValues = new RuntimeLifecycleLiveValues({
      acceptedSession: () => this.acceptedLifecycleSession(),
      isAcceptedAndTracked: (session) => {
        const key = structuredTextSessionKey(session);
        return (
          this.acceptedSessions.has(key) && this.sessions.get(key) === session
        );
      },
      persistFailure: (failure, session) => {
        this.failure = failure;
        this.failureScope = runtimeFailureScopeForSession(session);
        this.emitChanged();
      },
      emitIoChange: () => this.emitChanged("io"),
    });
  }

  readonly onDidChange = this.changeEmitter.event;

  register(context: vscode.ExtensionContext): void {
    if (this.registered) {
      return;
    }
    this.registered = true;
    this.runtimeConfigTargets.capture(vscode.window.activeTextEditor);
    registerRuntimeLifecycleEvents(context, {
      sessions: this.sessions,
      acceptedSessions: this.acceptedSessions,
      rejectedSessions: this.rejectedSessions,
      operations: this.operations,
      starting: () => this.starting,
      setStarting: (value) => {
        this.starting = value;
      },
      setTransitionTarget: (value) => {
        this.externalTransitionTarget = value;
      },
      clearFailure: () => {
        this.failure = undefined;
        this.failureScope = undefined;
      },
      setIoState: (value) => {
        this.liveValues.setIoState(value);
      },
      setAdsState: (value) => {
        this.liveValues.setAdsState(value);
      },
      getSession: () => this.getStructuredTextSession(),
      requestIoState: (session) => void this.requestIoState({ session }),
      requestAdsState: () => void this.requestAdsState(),
      acceptExternal: (session) => void this.acceptExternalSession(session),
      terminateUnaccepted: (session) =>
        void this.terminateUnacceptedSession(session),
      captureEditor: (editor) => this.runtimeConfigTargets.capture(editor),
      emit: (kind) => this.emitChanged(kind),
    });
  }

  getStructuredTextSession(): vscode.DebugSession | undefined {
    const active = vscode.debug.activeDebugSession;
    const trackedActive =
      active?.type === DEBUG_TYPE &&
      this.sessions.has(structuredTextSessionKey(active))
        ? active
        : undefined;
    return selectLifecycleDebugSession(
      trackedActive,
      this.sessions.values(),
      structuredTextSessionKey,
      (session) => this.acceptedSessions.has(structuredTextSessionKey(session)),
      (session) => this.sessionDisposition(session) === "rejected",
    );
  }

  /** The sole session authorized for user-visible runtime operations. */
  acceptedDebugSession(): vscode.DebugSession | undefined {
    return this.acceptedLifecycleSession();
  }

  phase(): LifecyclePhase {
    const session = this.getStructuredTextSession();
    return runtimeLifecyclePhase(
      this.starting,
      session?.configuration.request,
      !!session && this.acceptedSessions.has(structuredTextSessionKey(session)),
    );
  }

  operationState(): RuntimeLifecycleOperationState | undefined {
    return this.operations.current();
  }

  transitionTarget(): RuntimeLifecycleTarget | undefined {
    if (!this.starting) {
      return undefined;
    }
    return this.operations.current()?.target ?? this.externalTransitionTarget;
  }

  activeTarget(): RuntimeLifecycleTarget | undefined {
    const session = this.acceptedLifecycleSession();
    return session ? runtimeTargetForSession(session) : undefined;
  }

  async runExclusiveOperation<T>(
    kind: RuntimeLifecycleOperationKind,
    target: RuntimeLifecycleTarget,
    operation: (operationId: string) => Thenable<T>,
  ): Promise<RuntimeExclusiveOperationResult<T>> {
    if (this.starting || this.operations.active()) {
      return {
        acquired: false,
        reason:
          "A runtime operation is already in progress. Wait for it to finish.",
      };
    }
    const operationId = this.operations.begin(kind, target);
    const changesPhase = runtimeOperationChangesPhase(kind);
    if (changesPhase) {
      this.starting = true;
      this.failure = undefined;
      this.failureScope = undefined;
    }
    this.emitChanged();
    try {
      return { acquired: true, value: await operation(operationId) };
    } finally {
      this.operations.reject(operationId);
      if (changesPhase) {
        this.starting = false;
      }
      this.emitChanged();
    }
  }

  localFailure(): RuntimeStartFailure | undefined {
    return this.failureScope?.kind === "remote" ? undefined : this.failure;
  }

  runtimeConfigTarget(): vscode.Uri | undefined {
    return this.runtimeConfigTargets.target(this.getStructuredTextSession());
  }

  runtimeConfigScope(
    target: vscode.Uri | undefined,
  ): vscode.ConfigurationTarget {
    return this.runtimeConfigTargets.scope(target);
  }

  async snapshot(): Promise<RuntimeLifecycleSnapshot> {
    const status = await runtimeStatusPayload({
      runtimeConfigTarget: () => this.runtimeConfigTarget(),
      getStructuredTextSession: () => this.getStructuredTextSession(),
      isSessionAccepted: (session) =>
        this.acceptedSessions.has(structuredTextSessionKey(session)),
    });
    return {
      status,
      ioState: this.liveValues.currentIoState(),
      adsState: this.liveValues.currentAdsState(),
      starting: this.starting,
      operation: this.operationState(),
      transitionTarget: this.transitionTarget(),
      activeTarget: this.activeTarget(),
      failure: this.failure,
      failureScope: this.failureScope,
    };
  }

  async requestIoState(
    options: RuntimeIoStateRequestOptions = {},
  ): Promise<RuntimeLifecycleResult> {
    return this.liveValues.requestIoState(options);
  }

  async requestIoStateAfterScan(
    previousScan: number | undefined,
    options: { readonly timeoutMs?: number } = {},
  ): Promise<RuntimeLifecycleResult> {
    return this.liveValues.requestIoStateAfterScan(previousScan, options);
  }

  async requestAdsState(): Promise<RuntimeLifecycleResult> {
    return this.liveValues.requestAdsState();
  }

  async requestLiveValuesState(): Promise<RuntimeLifecycleResult> {
    return this.liveValues.requestLiveValuesState();
  }

  async requestLiveValuesStateAfterScan(
    previousScan: number | undefined,
  ): Promise<RuntimeLifecycleResult> {
    return this.liveValues.requestLiveValuesStateAfterScan(previousScan);
  }

  async setRuntimeMode(
    mode: unknown,
    target: vscode.Uri | undefined = this.runtimeConfigTarget(),
  ): Promise<void> {
    const normalized = mode === "online" ? "online" : "simulate";
    const config = getTrustConfiguration(target);
    await config.update(
      "runtime.mode",
      normalized,
      this.runtimeConfigScope(target),
    );
    this.emitChanged();
  }

  async startRuntime(targetLabel?: string): Promise<RuntimeLifecycleResult> {
    const status = await this.snapshot();
    if (status.status.runtimeMode === "online") {
      return this.connectRemote(status.status.endpoint, targetLabel);
    }
    return this.startLocalSimulator();
  }

  async startLocalSimulator(): Promise<RuntimeLifecycleResult>;
  async startLocalSimulator(
    validateProject: LocalSimulatorProjectValidator,
  ): Promise<LocalSimulatorStartResult>;
  async startLocalSimulator(
    validateProject?: LocalSimulatorProjectValidator,
  ): Promise<LocalSimulatorStartResult> {
    if (this.starting || this.operations.active()) {
      return runtimeOperationConflict(
        "A runtime operation is already in progress. Wait for it to finish.",
      );
    }
    const accepted = this.acceptedLifecycleSession();
    if (accepted) {
      return accepted.configuration.request === "launch"
        ? { ok: true, message: "Simulator already running." }
        : runtimeOperationConflict(
            "Disconnect the remote runtime before starting the Simulator.",
          );
    }
    const attemptId = this.operations.begin("local_start", {
      kind: "simulator",
    });
    this.starting = true;
    this.failure = undefined;
    this.failureScope = undefined;
    this.emitChanged();

    const coordinated = await coordinateLocalSimulatorStart({
      attemptId,
      projectRoot: this.runtimeConfigTarget(),
      validateProject,
      isAttemptActive: (candidate) => this.operations.active() === candidate,
      setRuntimeMode: (mode, target) => this.setRuntimeMode(mode, target),
      executeDebugStart: this.executeLocalSimulatorDebugStart,
      sessionForAttempt: (candidate) => this.sessionForAttempt(candidate),
      waitForReady: (session, timeoutMs) =>
        this.liveValues.waitForSimulatorSessionReady(
          session,
          timeoutMs,
          (key) => this.sessions.has(key),
        ),
      hasSession: (key) => this.sessions.has(key),
      sessionStabilityMs: this.sessionStabilityMs,
    });
    if (coordinated.kind === "cancelled") {
      return coordinated.result;
    }
    if (coordinated.kind === "preparation") {
      const preparation = coordinated.result;
      if ("validationRejected" in preparation) {
        this.operations.reject(attemptId);
        this.starting = false;
        this.failure = undefined;
        this.failureScope = undefined;
        this.emitChanged();
        return preparation;
      }
      this.operations.reject(attemptId);
      this.starting = false;
      this.failure = preparation.failure;
      this.failureScope = { kind: "simulator" };
      this.emitChanged();
      return preparation;
    }
    return this.settleOwnedTransition(attemptId, coordinated.outcome, {
      kind: "simulator",
    });
  }

  // Connect (attach) to a configured remote runtime by its control endpoint. Points the runtime at
  // that endpoint, switches to online mode, then attaches. Honest: this is a "Connect", never a remote
  // "Start" — we attach to a runtime we don't own.
  async connectRemote(
    endpoint: string,
    targetLabel?: string,
  ): Promise<RuntimeLifecycleResult> {
    const trimmed = endpoint.trim();
    if (!trimmed) {
      return {
        ok: false,
        failure: { kind: "failed_spawn", message: "Runtime endpoint not set." },
      };
    }
    if (this.starting || this.operations.active()) {
      return runtimeOperationConflict(
        "A runtime operation is already in progress. Wait for it to finish.",
      );
    }
    const accepted = this.acceptedLifecycleSession();
    if (accepted) {
      const activeEndpoint =
        typeof accepted.configuration.endpoint === "string"
          ? accepted.configuration.endpoint.trim()
          : "";
      if (
        accepted.configuration.request === "attach" &&
        activeEndpoint === trimmed
      ) {
        return { ok: true, message: "Already connected to runtime." };
      }
      return runtimeOperationConflict(
        accepted.configuration.request === "launch"
          ? "Stop the Simulator before connecting to a remote runtime."
          : "Disconnect the current remote runtime before connecting to another one.",
      );
    }
    const targetScope: RuntimeLifecycleTarget = {
      kind: "remote",
      endpoint: trimmed,
      ...(targetLabel?.trim() ? { label: targetLabel.trim() } : {}),
    };
    const attemptId = this.operations.begin("remote_connect", targetScope);
    this.starting = true;
    this.failure = undefined;
    this.failureScope = undefined;
    this.emitChanged();
    return this.connectRemoteForAttempt(
      attemptId,
      trimmed,
      targetLabel,
      true,
      undefined,
    );
  }

  async connectRemoteWithinOperation(
    operationId: string,
    endpoint: string,
    targetLabel?: string,
    managedRuntimeId?: string,
  ): Promise<RuntimeLifecycleResult> {
    const trimmed = endpoint.trim();
    if (!trimmed) {
      return runtimeOperationConflict("Runtime endpoint not set.");
    }
    if (this.operations.active() !== operationId) {
      return runtimeOperationConflict(
        "The managed runtime operation is no longer active.",
      );
    }
    return this.connectRemoteForAttempt(
      operationId,
      trimmed,
      targetLabel,
      false,
      managedRuntimeId,
    );
  }

  private async connectRemoteForAttempt(
    attemptId: string,
    endpoint: string,
    targetLabel: string | undefined,
    releaseOperation: boolean,
    managedRuntimeId: string | undefined,
  ): Promise<RuntimeLifecycleResult> {
    const target = this.runtimeConfigTarget();
    const scope = this.runtimeConfigScope(target);
    const outcome = await runOwnedDebugTransition({
      start: async () => {
        const config = getTrustConfiguration(target);
        await config.update("runtime.controlEndpoint", endpoint, scope);
        await config.update("runtime.controlEndpointEnabled", true, scope);
        await config.update("runtime.mode", "online", scope);
        const status = (await this.snapshot()).status;
        return withTimeout(
          startOnlineRuntimeConnection(status, {
            configurationTarget: target,
            configurationScope: scope,
            targetLabel,
            lifecycleAttemptId: attemptId,
            managedRuntimeId,
          }),
          DEBUG_START_COMMAND_TIMEOUT_MS,
          "Connecting to the remote runtime timed out.",
        );
      },
      waitForSession: () =>
        waitForLifecycleSessionResult(
          () => this.sessionForAttempt(attemptId),
          SESSION_WAIT_TIMEOUT_MS,
        ),
      waitForReady: (session) =>
        this.liveValues.waitForAttachedSessionReady(
          session,
          SESSION_WAIT_TIMEOUT_MS,
          (key) => this.sessions.has(key),
        ),
      waitForStable: (session) =>
        waitForSessionStable(
          session,
          (key) => this.sessions.has(key),
          this.sessionStabilityMs,
        ),
      missingSessionMessage:
        "Timed out waiting for the remote debug attachment.",
      unstableSessionMessage:
        "Runtime debug attachment ended before it became stable.",
      successMessage: "Connected to runtime.",
    });
    return this.settleOwnedTransition(
      attemptId,
      outcome,
      {
        kind: "remote",
        endpoint,
      },
      releaseOperation,
    );
  }

  private async settleOwnedTransition(
    attemptId: string,
    outcome: OwnedDebugTransitionOutcome,
    failureScope: RuntimeLifecycleFailureScope,
    releaseOperation = true,
  ): Promise<RuntimeLifecycleResult> {
    const ownedSession = outcome.session ?? this.sessionForAttempt(attemptId);
    if (this.operations.active() !== attemptId) {
      if (ownedSession) {
        await this.terminateUnacceptedSession(ownedSession);
      }
      return runtimeOperationConflict(
        "Runtime operation was cancelled before it completed.",
      );
    }
    if (outcome.result.ok && ownedSession) {
      const key = structuredTextSessionKey(ownedSession);
      await this.selectAcceptedSessionTarget(ownedSession);
      if (
        this.operations.active() !== attemptId ||
        this.sessions.get(key) !== ownedSession
      ) {
        if (this.operations.active() !== attemptId) {
          return runtimeOperationConflict(
            "Runtime session ended before startup acceptance completed.",
          );
        }
        if (releaseOperation) {
          this.operations.reject(attemptId);
          this.starting = false;
        }
        this.failure = {
          kind: "failed_spawn",
          message:
            "Runtime session ended before startup acceptance completed.",
        };
        this.failureScope = failureScope;
        this.emitChanged();
        return { ok: false, failure: this.failure };
      }
      // Commit acceptance only after target persistence and an exact-session
      // recheck. Until this synchronous point every consumer sees Starting,
      // never a transient Running session that can still disappear.
      this.acceptedSessions.add(key);
      if (releaseOperation) {
        this.operations.accept(attemptId);
        this.starting = false;
      }
      this.externalTransitionTarget = undefined;
      this.failure = undefined;
      this.failureScope = undefined;
      this.emitChanged();
      return outcome.result;
    }
    if (releaseOperation) {
      this.operations.reject(attemptId);
      this.starting = false;
    }
    this.externalTransitionTarget = undefined;
    this.failure = outcome.result.ok
      ? {
          kind: "internal_startup",
          message:
            "Runtime operation completed without an owned debug session.",
        }
      : outcome.result.failure;
    this.failureScope = failureScope;
    debugChannel().appendLine(
      `Runtime operation failed: ${this.failure.message}`,
    );
    this.emitChanged();
    if (ownedSession) {
      if (releaseOperation) {
        await this.terminateUnacceptedSession(ownedSession);
      } else {
        setTimeout(() => void this.terminateUnacceptedSession(ownedSession), 0);
      }
    }
    return { ok: false, failure: this.failure };
  }

  async stopRuntime(operationId?: string): Promise<RuntimeLifecycleResult> {
    return runRuntimeStopOperation(operationId, {
      activeOperation: () => this.operations.active(),
      getSession: () => this.getStructuredTextSession(),
      runExclusive: (kind, target, operation) =>
        this.runExclusiveOperation(kind, target, operation),
      executeStop: this.executeRuntimeDebugStop,
      hasSession: (key) => this.sessions.has(key),
      snapshot: () => this.snapshot(),
      runtimeConfigTarget: () => this.runtimeConfigTarget(),
      runtimeConfigScope: (target) => this.runtimeConfigScope(target),
      markStopped: (message) => this.markStopped(message),
      emitChanged: () => this.emitChanged(),
    });
  }

  private sessionForAttempt(
    attemptId: string,
  ): vscode.DebugSession | undefined {
    return findLifecycleSessionForAttempt(
      vscode.debug.activeDebugSession,
      this.sessions.values(),
      attemptId,
      DEBUG_TYPE,
    );
  }

  private sessionDisposition(session: vscode.DebugSession) {
    const key = structuredTextSessionKey(session);
    return this.operations.disposition(
      lifecycleStartAttemptId(session.configuration),
      this.acceptedSessions.has(key),
      this.rejectedSessions.has(key),
    );
  }

  private async acceptExternalSession(
    session: vscode.DebugSession,
  ): Promise<void> {
    const key = structuredTextSessionKey(session);
    const attached =
      debugSessionAcceptancePath(session.configuration.request) ===
      "remote_attach";
    const ioStateResult = attached
      ? await this.liveValues.waitForAttachedSessionReady(
          session,
          SESSION_WAIT_TIMEOUT_MS,
          (sessionKey) => this.sessions.has(sessionKey),
        )
      : await this.liveValues.waitForSimulatorSessionReady(
          session,
          SESSION_WAIT_TIMEOUT_MS,
          (sessionKey) => this.sessions.has(sessionKey),
        );
    if (!this.sessions.has(key)) {
      return;
    }
    if (!ioStateResult.ok) {
      this.starting = false;
      this.externalTransitionTarget = undefined;
      this.failure = ioStateResult.failure;
      this.failureScope = runtimeFailureScopeForSession(session);
      await this.terminateUnacceptedSession(session);
      this.emitChanged();
      return;
    }
    if (
      !(await waitForSessionStable(
        session,
        (sessionKey) => this.sessions.has(sessionKey),
        this.sessionStabilityMs,
      ))
    ) {
      this.starting = false;
      this.externalTransitionTarget = undefined;
      this.failure = {
        kind: "failed_spawn",
        message: attached
          ? "Runtime debug attachment ended before it became stable."
          : "Simulator stopped during startup. Check the runtime port or target settings.",
      };
      this.failureScope = runtimeFailureScopeForSession(session);
      await this.terminateUnacceptedSession(session);
      this.emitChanged();
      return;
    }
    await this.selectAcceptedSessionTarget(session);
    if (this.sessions.get(key) !== session) {
      // A new operation/session may already own state. Never let this stale
      // acceptance continuation overwrite it.
      if (this.operations.active() || this.getStructuredTextSession()) {
        return;
      }
      this.starting = false;
      this.externalTransitionTarget = undefined;
      this.failure = {
        kind: "failed_spawn",
        message: "Runtime session ended before startup acceptance completed.",
      };
      this.failureScope = runtimeFailureScopeForSession(session);
      this.emitChanged();
      return;
    }
    // External sessions use the same commit point as owned Start/Connect:
    // persist selection, recheck exact identity, then publish acceptance.
    this.acceptedSessions.add(key);
    this.starting = false;
    this.externalTransitionTarget = undefined;
    this.failure = undefined;
    this.failureScope = undefined;
    this.emitChanged();
  }

  private async terminateUnacceptedSession(
    session: vscode.DebugSession,
  ): Promise<void> {
    const key = structuredTextSessionKey(session);
    if (this.acceptedSessions.has(key)) {
      return;
    }
    // Remove it from lifecycle selection before the asynchronous VS Code stop
    // finishes. Otherwise an active duplicate can transiently replace the
    // already accepted simulator across sidebar/status/canvas snapshots.
    this.rejectedSessions.add(key);
    this.sessions.delete(key);
    await terminateRejectedSession(session, (candidate) =>
      vscode.debug.stopDebugging(candidate),
    );
  }

  private markStopped(message: string): RuntimeLifecycleResult {
    this.liveValues.reset();
    this.acceptedSessions.clear();
    this.operations.cancel();
    this.starting = false;
    this.externalTransitionTarget = undefined;
    this.failure = undefined;
    this.failureScope = undefined;
    this.emitChanged();
    return { ok: true, message };
  }

  private acceptedLifecycleSession(): vscode.DebugSession | undefined {
    const session = this.getStructuredTextSession();
    return session &&
      this.acceptedSessions.has(structuredTextSessionKey(session))
      ? session
      : undefined;
  }

  private emitChanged(
    kind: RuntimeLifecycleChange["kind"] = "lifecycle",
  ): void {
    this.changeEmitter.fire({ kind });
  }
}

export const runtimeLifecycleService = new RuntimeLifecycleService();

export function registerRuntimeLifecycle(context: vscode.ExtensionContext): void {
  runtimeLifecycleService.register(context);
}
