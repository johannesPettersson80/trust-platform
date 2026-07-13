import * as vscode from "vscode";

import {
  hasActiveOrRecoveringAdsConnection,
  type AdsStatusReport,
} from "../adsStatusSummary";
import { isLocalControlEndpoint } from "../runtimeControl";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import type { RuntimeTarget } from "../runtimeTarget";
import {
  planAdsServicePorts,
  probeAdsServicesSequentially,
} from "./adsServiceProbeModel";
import {
  offlineBrowseSymbols,
  type BrowseSymbolsResponse,
  type DiscoverCandidate,
} from "./offlineComm";

export interface AdsServiceProbeControllerDependencies {
  readonly panel: () => vscode.WebviewPanel | undefined;
  readonly extensionContext: () => vscode.ExtensionContext | undefined;
  readonly runtimeTargetForOrigin: (originId: string) => RuntimeTarget | undefined;
  readonly runtimeTargetOnDiscoveryComputer?: () => RuntimeTarget | undefined;
  readonly requestIsCurrent: (request: AdsServiceProbeRequestIdentity) => boolean;
  readonly runtimeControlRequest?: typeof sendRuntimeControlRequest;
}

export const ACTIVE_ADS_CONNECTION_PROBE_SAFETY_MESSAGE =
  "The selected runtime already owns an active ADS connection. Read-only service checks are paused to protect live PLC I/O. Stop that ADS connection before retrying.";
export const UNKNOWN_ADS_CONNECTION_PROBE_SAFETY_MESSAGE =
  "truST could not verify whether the selected runtime owns an ADS connection, so read-only service checks were paused to protect PLC I/O. Reconnect or update that runtime, then retry.";

export function adsStatusProbeSafetyMessage(
  report: unknown
): string | undefined {
  if (!isVerifiableAdsStatusReport(report)) {
    return UNKNOWN_ADS_CONNECTION_PROBE_SAFETY_MESSAGE;
  }
  return hasActiveOrRecoveringAdsConnection(report as AdsStatusReport)
    ? ACTIVE_ADS_CONNECTION_PROBE_SAFETY_MESSAGE
    : undefined;
}

export function localRuntimeTargetForAdsProbe(
  target: RuntimeTarget | undefined
): RuntimeTarget | undefined {
  return target?.mode === "online" &&
    target.endpoint &&
    isLocalControlEndpoint(target.endpoint) &&
    target.reachable &&
    target.status === "online_reachable"
    ? target
    : undefined;
}

export interface AdsServiceProbeRequestIdentity {
  readonly sessionId: string;
  readonly requestId: number;
  readonly origin: string;
  readonly candidate: DiscoverCandidate;
}

interface AdsServiceProbeRequest extends AdsServiceProbeRequestIdentity {
  readonly ports: readonly number[];
}

export interface ActiveAdsDiscoveryRequest {
  readonly sessionId: string;
  readonly requestId: number;
  readonly origin: string;
}

export function isCurrentAdsServiceProbeRequest(
  message: Record<string, unknown>,
  active: ActiveAdsDiscoveryRequest | undefined,
  activeWebviewSessionId: string | undefined
): boolean {
  if (
    !active ||
    message.sessionId !== active.sessionId ||
    message.requestId !== active.requestId ||
    message.sessionId !== activeWebviewSessionId ||
    typeof message.origin !== "string" ||
    message.origin.length === 0 ||
    message.origin !== active.origin ||
    !isRecord(message.candidate)
  ) {
    return false;
  }
  const candidateOrigin =
    typeof message.candidate.originRuntimeId === "string"
      ? message.candidate.originRuntimeId
      : undefined;
  return active.origin === "this_host"
    ? candidateOrigin === undefined
    : candidateOrigin === active.origin;
}

/** Runs bounded ADS service inspection after host identity discovery has completed. */
export class AdsServiceProbeController {
  private activeCancellation: vscode.CancellationTokenSource | undefined;

  constructor(
    private readonly dependencies: AdsServiceProbeControllerDependencies
  ) {}

  cancel(): void {
    this.activeCancellation?.cancel();
    this.activeCancellation?.dispose();
    this.activeCancellation = undefined;
  }

  async probe(message: Record<string, unknown>): Promise<void> {
    const request = parseProbeRequest(message);
    const panel = this.dependencies.panel();
    const context = this.dependencies.extensionContext();
    if (!request || !panel || !context) {
      return;
    }
    if (!this.isCurrent(request, panel)) {
      return;
    }
    this.cancel();
    const cancellation = new vscode.CancellationTokenSource();
    this.activeCancellation = cancellation;

    try {
      if (request.origin !== "this_host") {
        const runtime = this.dependencies.runtimeTargetForOrigin(request.origin);
        if (!isReachableRuntime(runtime)) {
          if (!this.isActive(request, panel, cancellation.token)) {
            return;
          }
          void panel.webview.postMessage({
            type: "adsServiceProbeResults",
            sessionId: request.sessionId,
            requestId: request.requestId,
            candidateId: request.candidate.id,
            results: [],
            error: "The selected discovery runtime is no longer reachable. Reconnect it and discover ADS devices again.",
          });
          return;
        }
        const safetyError = await this.remoteProbeSafetyError(
          runtime,
          cancellation.token
        );
        if (!this.isActive(request, panel, cancellation.token)) {
          return;
        }
        if (safetyError) {
          void panel.webview.postMessage({
            type: "adsServiceProbeResults",
            sessionId: request.sessionId,
            requestId: request.requestId,
            candidateId: request.candidate.id,
            results: [],
            error: safetyError,
          });
          return;
        }
      }
      if (request.origin === "this_host") {
        const localRuntime =
          this.dependencies.runtimeTargetOnDiscoveryComputer?.();
        if (localRuntime) {
          const safetyError = isReachableRuntime(localRuntime)
            ? await this.remoteProbeSafetyError(
                localRuntime,
                cancellation.token
              )
            : UNKNOWN_ADS_CONNECTION_PROBE_SAFETY_MESSAGE;
          if (!this.isActive(request, panel, cancellation.token)) {
            return;
          }
          if (safetyError) {
            void panel.webview.postMessage({
              type: "adsServiceProbeResults",
              sessionId: request.sessionId,
              requestId: request.requestId,
              candidateId: request.candidate.id,
              results: [],
              error: safetyError,
            });
            return;
          }
        }
      }

      const ports = planAdsServicePorts(request.ports);
      const results = await probeAdsServicesSequentially(
        ports,
        (port) =>
          this.browsePort(request, context, port, cancellation.token),
        {
          isActive: () =>
            !cancellation.token.isCancellationRequested &&
            this.isCurrent(request, panel),
          onBeforeProbe: async (port, index, total) => {
            if (
              cancellation.token.isCancellationRequested ||
              !this.isCurrent(request, panel)
            ) {
              return;
            }
            await panel.webview.postMessage({
              type: "adsServiceProbeProgress",
              sessionId: request.sessionId,
              requestId: request.requestId,
              candidateId: request.candidate.id,
              port,
              index,
              total,
            });
          },
        }
      );
      if (
        cancellation.token.isCancellationRequested ||
        !this.isCurrent(request, panel)
      ) {
        return;
      }
      void panel.webview.postMessage({
        type: "adsServiceProbeResults",
        sessionId: request.sessionId,
        requestId: request.requestId,
        candidateId: request.candidate.id,
        results,
      });
    } catch (error) {
      if (
        cancellation.token.isCancellationRequested ||
        !this.isCurrent(request, panel)
      ) {
        return;
      }
      void panel.webview.postMessage({
        type: "adsServiceProbeResults",
        sessionId: request.sessionId,
        requestId: request.requestId,
        candidateId: request.candidate.id,
        results: [],
        error: error instanceof Error ? error.message : String(error),
      });
    } finally {
      if (this.activeCancellation === cancellation) {
        this.activeCancellation = undefined;
        cancellation.dispose();
      }
    }
  }

  private isCurrent(
    request: AdsServiceProbeRequest,
    panel: vscode.WebviewPanel
  ): boolean {
    return (
      this.dependencies.panel() === panel &&
      panel.visible &&
      this.dependencies.requestIsCurrent(request)
    );
  }

  private isActive(
    request: AdsServiceProbeRequest,
    panel: vscode.WebviewPanel,
    cancellationToken: vscode.CancellationToken
  ): boolean {
    return (
      !cancellationToken.isCancellationRequested &&
      this.isCurrent(request, panel)
    );
  }

  private async remoteProbeSafetyError(
    runtime: RuntimeTarget & { endpoint: string },
    cancellationToken: vscode.CancellationToken
  ): Promise<string | undefined> {
    const requestControl =
      this.dependencies.runtimeControlRequest ?? sendRuntimeControlRequest;
    try {
      const report = await requestControl<AdsStatusReport>(
        runtime.endpoint,
        runtime.authToken,
        "ads.status",
        undefined,
        { timeoutMs: 1_000, cancellationToken }
      );
      return adsStatusProbeSafetyMessage(report);
    } catch {
      return UNKNOWN_ADS_CONNECTION_PROBE_SAFETY_MESSAGE;
    }
  }

  private async browsePort(
    request: AdsServiceProbeRequest,
    context: vscode.ExtensionContext,
    port: number,
    cancellationToken: vscode.CancellationToken
  ): Promise<BrowseSymbolsResponse> {
    const target: Record<string, unknown> = {
      ...request.candidate.params,
      ams_port: port,
    };
    const runtime = this.dependencies.runtimeTargetForOrigin(request.origin);
    if (request.origin !== "this_host") {
      if (!isReachableRuntime(runtime) || !runtime.endpoint) {
        throw new Error(
          "The selected discovery runtime is no longer reachable. Reconnect it and discover ADS devices again."
        );
      }
      try {
        return await sendRuntimeControlRequest<BrowseSymbolsResponse>(
          runtime.endpoint,
          runtime.authToken,
          "comm.browse_symbols",
          {
            protocol: "ads",
            target,
            kind: "symbols",
            connection_name:
              typeof target.name === "string" ? target.name : undefined,
          },
          { timeoutMs: 20_000, cancellationToken }
        );
      } catch (error) {
        return failedBrowseResponse(error);
      }
    }

    const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    return (
      (await offlineBrowseSymbols(
        context,
        "ads",
        target,
        "symbols",
        typeof target.name === "string" ? target.name : undefined,
        projectDir,
        cancellationToken
      )) ?? failedBrowseResponse("ADS service probe returned no response.")
    );
  }
}

function isReachableRuntime(
  runtime: RuntimeTarget | undefined
): runtime is RuntimeTarget & { endpoint: string } {
  return Boolean(
    runtime?.status === "online_reachable" && runtime.endpoint
  );
}

function parseProbeRequest(
  message: Record<string, unknown>
): AdsServiceProbeRequest | undefined {
  if (
    typeof message.sessionId !== "string" ||
    typeof message.origin !== "string" ||
    message.origin.length === 0 ||
    !Number.isSafeInteger(message.requestId) ||
    typeof message.requestId !== "number" ||
    !isRecord(message.candidate) ||
    typeof message.candidate.id !== "string" ||
    !isRecord(message.candidate.params)
  ) {
    return undefined;
  }
  const candidate: DiscoverCandidate = {
    id: message.candidate.id,
    label:
      typeof message.candidate.label === "string"
        ? message.candidate.label
        : "ADS device",
    protocol: "ads",
    source:
      typeof message.candidate.source === "string"
        ? message.candidate.source
        : "manual",
    confidence:
      typeof message.candidate.confidence === "string"
        ? message.candidate.confidence
        : "observed",
    originRuntimeId:
      typeof message.candidate.originRuntimeId === "string"
        ? message.candidate.originRuntimeId
        : undefined,
    params: message.candidate.params,
  };
  return {
    sessionId: message.sessionId,
    requestId: message.requestId,
    origin: message.origin,
    candidate,
    ports: Array.isArray(message.ports)
      ? message.ports.filter(
          (port): port is number =>
            typeof port === "number" && Number.isSafeInteger(port)
        )
      : [],
  };
}

export function failedBrowseResponse(error: unknown): BrowseSymbolsResponse {
  const message = error instanceof Error ? error.message : String(error);
  // A failed control exchange is not a reply from the logical ADS service.
  // Preserve its detail, but do not infer service availability from wording.
  return {
    schema_version: 1,
    protocol: "ads",
    kind: "symbols",
    tree: [],
    error: {
      code: "control_request_failed",
      message,
    },
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function isVerifiableAdsStatusReport(
  value: unknown
): boolean {
  if (
    !isRecord(value) ||
    typeof value.overall !== "string" ||
    !Array.isArray(value.connections) ||
    !value.connections.every(
      (connection) =>
        isRecord(connection) &&
        typeof connection.state === "string" &&
        connection.state.trim().length > 0
    )
  ) {
    return false;
  }
  return value.connections.length > 0 || value.overall === "disabled";
}
