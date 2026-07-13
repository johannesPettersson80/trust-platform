import * as vscode from "vscode";

import {
  EMPTY_ADS_LIVE_VALUES_STATE,
  normalizeAdsLiveValuesState,
  type AdsLiveValuesState,
} from "./adsLiveValuesModel";
import type { IoState } from "./io-panel/types";
import { classifyRuntimeStartFailure } from "./networkCanvas/runtimeFailures";
import {
  delay,
  EMPTY_IO_STATE,
  IO_NEXT_SCAN_POLL_MS,
  IO_NEXT_SCAN_TIMEOUT_MS,
  withTimeout,
  type RuntimeLifecycleResult,
  type RuntimeStartFailure,
} from "./runtimeLifecycleModel";
import {
  waitForAttachedSessionReady,
  waitForSimulatorSessionReady,
} from "./runtimeSessionReadiness";

export type RuntimeIoStateRequestOptions = {
  readonly persistFailure?: boolean;
  readonly afterScan?: number;
  readonly session?: vscode.DebugSession;
  readonly timeoutMs?: number;
};

type RuntimeLifecycleLiveValuesDependencies = {
  readonly acceptedSession: () => vscode.DebugSession | undefined;
  readonly isAcceptedAndTracked: (session: vscode.DebugSession) => boolean;
  readonly persistFailure: (
    failure: RuntimeStartFailure,
    session: vscode.DebugSession,
  ) => void;
  readonly emitIoChange: () => void;
};

/** Owns the cached I/O and ADS values plus their DAP request sequencing. */
export class RuntimeLifecycleLiveValues {
  private ioState: IoState = EMPTY_IO_STATE;
  private adsState: AdsLiveValuesState = EMPTY_ADS_LIVE_VALUES_STATE;

  constructor(
    private readonly dependencies: RuntimeLifecycleLiveValuesDependencies,
  ) {}

  currentIoState(): IoState {
    return this.ioState;
  }

  currentAdsState(): AdsLiveValuesState {
    return this.adsState;
  }

  setIoState(value: IoState): void {
    this.ioState = value;
  }

  setAdsState(value: AdsLiveValuesState): void {
    this.adsState = value;
  }

  reset(): void {
    this.ioState = EMPTY_IO_STATE;
    this.adsState = EMPTY_ADS_LIVE_VALUES_STATE;
  }

  async requestIoState(
    options: RuntimeIoStateRequestOptions = {},
  ): Promise<RuntimeLifecycleResult> {
    // Explicit sessions are used only by startup readiness. Every ordinary
    // caller must stay on the accepted lifecycle authority.
    const session = options.session ?? this.dependencies.acceptedSession();
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
      const request = session.customRequest(
        "stIoState",
        options.afterScan === undefined
          ? undefined
          : { afterScan: options.afterScan },
      );
      if (options.timeoutMs !== undefined) {
        await withTimeout(
          request,
          Math.max(1, options.timeoutMs),
          "I/O state request timed out.",
        );
      } else {
        await request;
      }
      return { ok: true, message: "I/O state requested." };
    } catch (err) {
      const failure = classifyRuntimeStartFailure(err);
      const ioFailure = {
        ...failure,
        message: `I/O state request failed: ${failure.message}`,
      };
      if (options.persistFailure) {
        this.dependencies.persistFailure(ioFailure, session);
      }
      return { ok: false, failure: ioFailure };
    }
  }

  async requestIoStateAfterScan(
    previousScan: number | undefined,
    options: { readonly timeoutMs?: number } = {},
  ): Promise<RuntimeLifecycleResult> {
    const deadline =
      Date.now() + (options.timeoutMs ?? IO_NEXT_SCAN_TIMEOUT_MS);
    let lastResult: RuntimeLifecycleResult = {
      ok: true,
      message: "I/O state requested.",
    };
    do {
      lastResult = await this.requestIoState({ afterScan: previousScan });
      if (!lastResult.ok) {
        return lastResult;
      }
      const nextScan = this.ioState.scan;
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

  async requestAdsState(): Promise<RuntimeLifecycleResult> {
    const session = this.dependencies.acceptedSession();
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
      const body = await session.customRequest("stAdsState");
      if (this.dependencies.isAcceptedAndTracked(session)) {
        this.adsState = normalizeAdsLiveValuesState(body);
        this.dependencies.emitIoChange();
      }
      return { ok: true, message: "ADS state requested." };
    } catch (err) {
      const failure = classifyRuntimeStartFailure(err);
      if (this.dependencies.isAcceptedAndTracked(session)) {
        this.adsState = EMPTY_ADS_LIVE_VALUES_STATE;
        this.dependencies.emitIoChange();
      }
      return {
        ok: false,
        failure: {
          ...failure,
          message: `ADS state request failed: ${failure.message}`,
        },
      };
    }
  }

  async requestLiveValuesState(): Promise<RuntimeLifecycleResult> {
    const ioResult = await this.requestIoState();
    if (!ioResult.ok) {
      return ioResult;
    }
    const adsResult = await this.requestAdsState();
    // Older adapters may not implement stAdsState. I/O remains usable, while
    // the absence of ADS data stays an honest empty section.
    return adsResult.ok ? ioResult : adsResult;
  }

  async requestLiveValuesStateAfterScan(
    previousScan: number | undefined,
  ): Promise<RuntimeLifecycleResult> {
    const ioResult = await this.requestIoStateAfterScan(previousScan);
    if (!ioResult.ok) {
      return ioResult;
    }
    const adsResult = await this.requestAdsState();
    return adsResult.ok ? ioResult : adsResult;
  }

  async waitForAttachedSessionReady(
    session: vscode.DebugSession,
    timeoutMs: number,
    hasSession: (key: string) => boolean,
  ): Promise<RuntimeLifecycleResult> {
    return waitForAttachedSessionReady(
      session,
      timeoutMs,
      hasSession,
      (candidate, requestTimeoutMs) =>
        this.requestIoState({
          session: candidate,
          timeoutMs: requestTimeoutMs,
        }),
    );
  }

  async waitForSimulatorSessionReady(
    session: vscode.DebugSession,
    timeoutMs: number,
    hasSession: (key: string) => boolean,
  ): Promise<RuntimeLifecycleResult> {
    return waitForSimulatorSessionReady(
      session,
      timeoutMs,
      hasSession,
      (candidate, requestTimeoutMs) =>
        this.requestIoState({
          session: candidate,
          timeoutMs: requestTimeoutMs,
        }),
    );
  }
}
