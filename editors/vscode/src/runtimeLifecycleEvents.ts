import * as vscode from "vscode";

import {
  EMPTY_ADS_LIVE_VALUES_STATE,
  normalizeAdsLiveValuesState,
} from "./adsLiveValuesModel";
import { affectsTrustConfiguration } from "./configuration";
import { debugChannel, DEBUG_TYPE } from "./debug/configuration";
import {
  lifecycleStartAttemptId,
  RuntimeLifecycleAttemptRegistry,
} from "./debug/startAttempt";
import { terminatedSessionOwnsLifecycleState } from "./debug/sessionSelection";
import {
  EMPTY_IO_STATE,
  normalizeIoState,
  runtimeTargetForSession,
  structuredTextSessionKey,
  type RuntimeLifecycleChange,
  type RuntimeLifecycleTarget,
} from "./runtimeLifecycleModel";

export interface RuntimeLifecycleEventDependencies {
  readonly sessions: Map<string, vscode.DebugSession>;
  readonly acceptedSessions: Set<string>;
  readonly rejectedSessions: Set<string>;
  readonly operations: RuntimeLifecycleAttemptRegistry;
  readonly starting: () => boolean;
  readonly setStarting: (value: boolean) => void;
  readonly setTransitionTarget: (
    value: RuntimeLifecycleTarget | undefined
  ) => void;
  readonly clearFailure: () => void;
  readonly setIoState: (value: ReturnType<typeof normalizeIoState>) => void;
  readonly setAdsState: (
    value: ReturnType<typeof normalizeAdsLiveValuesState>
  ) => void;
  readonly getSession: () => vscode.DebugSession | undefined;
  readonly requestIoState: (session: vscode.DebugSession) => void;
  readonly requestAdsState: () => void;
  readonly acceptExternal: (session: vscode.DebugSession) => void;
  readonly terminateUnaccepted: (session: vscode.DebugSession) => void;
  readonly captureEditor: (editor: vscode.TextEditor | undefined) => void;
  readonly emit: (kind?: RuntimeLifecycleChange["kind"]) => void;
}

export function registerRuntimeLifecycleEvents(
  context: vscode.ExtensionContext,
  deps: RuntimeLifecycleEventDependencies
): void {
  const track = (session: vscode.DebugSession) => {
    const key = structuredTextSessionKey(session);
    if (!deps.rejectedSessions.has(key)) {
      deps.sessions.set(key, session);
    }
  };
  const disposition = (session: vscode.DebugSession) => {
    const key = structuredTextSessionKey(session);
    return deps.rejectedSessions.has(key)
      ? ("rejected" as const)
      : deps.operations.disposition(
          lifecycleStartAttemptId(session.configuration),
          deps.acceptedSessions.has(key)
        );
  };

  const active = vscode.debug.activeDebugSession;
  if (active?.type === DEBUG_TYPE) {
    track(active);
    deps.setStarting(true);
    deps.setTransitionTarget(runtimeTargetForSession(active));
    deps.acceptExternal(active);
  }

  context.subscriptions.push(
    vscode.debug.onDidReceiveDebugSessionCustomEvent((event) => {
      const key = structuredTextSessionKey(event.session);
      if (
        (event.event !== "stIoState" && event.event !== "stAdsState") ||
        event.session.type !== DEBUG_TYPE ||
        !deps.sessions.has(key) ||
        disposition(event.session) === "rejected"
      ) {
        return;
      }
      if (event.event === "stAdsState") {
        // ADS values are never startup-readiness evidence. Accept them only
        // from the exact session already committed as lifecycle authority.
        if (!deps.acceptedSessions.has(key)) {
          return;
        }
        deps.setAdsState(normalizeAdsLiveValuesState(event.body));
      } else {
        deps.setIoState(normalizeIoState(event.body));
      }
      deps.emit("io");
    }),
    vscode.debug.onDidStartDebugSession((session) => {
      if (session.type !== DEBUG_TYPE) {
        return;
      }
      const key = structuredTextSessionKey(session);
      const attemptId = lifecycleStartAttemptId(session.configuration);
      const sessionDisposition = disposition(session);
      const conflictsWithTracked = [...deps.sessions.keys()].some(
        (candidate) => candidate !== key
      );
      const conflictsWithOperation =
        sessionDisposition === "external" && Boolean(deps.operations.active());
      if (
        sessionDisposition === "rejected" ||
        conflictsWithOperation ||
        conflictsWithTracked
      ) {
        deps.rejectedSessions.add(key);
        debugChannel().appendLine(
          attemptId
            ? `Rejecting late Structured Text session from inactive operation ${attemptId}.`
            : "Rejecting a second Structured Text session; the lifecycle owns one at a time."
        );
        deps.terminateUnaccepted(session);
        return;
      }
      track(session);
      deps.clearFailure();
      if (sessionDisposition === "external" && !deps.starting()) {
        deps.setStarting(true);
        deps.setTransitionTarget(runtimeTargetForSession(session));
        deps.acceptExternal(session);
      }
      deps.emit();
    }),
    vscode.debug.onDidTerminateDebugSession((session) => {
      if (session.type !== DEBUG_TYPE) {
        return;
      }
      const key = structuredTextSessionKey(session);
      const attemptId = lifecycleStartAttemptId(session.configuration);
      const wasCurrent = attemptId === deps.operations.active();
      const wasTracked = deps.sessions.delete(key);
      if (attemptId && wasCurrent) {
        deps.operations.reject(attemptId);
      }
      deps.acceptedSessions.delete(key);
      deps.rejectedSessions.delete(key);
      if (!deps.getSession()) {
        deps.setIoState(EMPTY_IO_STATE);
        deps.setAdsState(EMPTY_ADS_LIVE_VALUES_STATE);
        deps.setTransitionTarget(undefined);
      }
      if (terminatedSessionOwnsLifecycleState(wasTracked, wasCurrent)) {
        deps.setStarting(false);
      }
      deps.emit();
    }),
    vscode.debug.onDidChangeActiveDebugSession((session) => {
      if (session?.type === DEBUG_TYPE) {
        const key = structuredTextSessionKey(session);
        const sessionDisposition = disposition(session);
        const conflictsWithTracked = [...deps.sessions.keys()].some(
          (candidate) => candidate !== key
        );
        if (
          sessionDisposition === "rejected" ||
          (sessionDisposition === "external" && Boolean(deps.operations.active())) ||
          conflictsWithTracked
        ) {
          deps.rejectedSessions.add(key);
          deps.terminateUnaccepted(session);
        } else {
          track(session);
          if (deps.acceptedSessions.has(key)) {
            deps.requestIoState(session);
            deps.requestAdsState();
          }
        }
      }
      deps.emit();
    }),
    vscode.window.onDidChangeActiveTextEditor(deps.captureEditor),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (
        affectsTrustConfiguration(event, "runtime.controlEndpoint") ||
        affectsTrustConfiguration(event, "runtime.controlEndpointEnabled") ||
        affectsTrustConfiguration(event, "runtime.inlineValuesEnabled") ||
        affectsTrustConfiguration(event, "runtime.mode")
      ) {
        deps.emit();
      }
    })
  );
}
