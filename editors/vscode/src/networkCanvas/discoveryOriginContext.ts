import {
  resolveRuntimeTargetFromSettings,
  type RuntimeTarget,
} from "../runtimeTarget";
import { getControlAuthToken } from "../runtimeAuth";
import { DiscoveryBrowseLeaseStore } from "./discoveryBrowseLease";
import {
  DiscoveryOriginTargetStore,
  resolveRegisteredDiscoveryOriginEndpoint,
} from "./discoveryOriginTargets";
import type { NCGraph } from "./webview/types";

/** Host-owned origin endpoints, credentials, and one scoped discovery-to-Browse lease. */
export class DiscoveryOriginContext {
  private readonly targets = new DiscoveryOriginTargetStore();
  private readonly endpoints = new Map<string, string>();
  private readonly browseLeases = new DiscoveryBrowseLeaseStore();

  clearCredentials(): void {
    this.browseLeases.clear();
    this.targets.clear();
  }

  clearEndpoints(): void {
    this.endpoints.clear();
  }

  pin(originId: string, target: RuntimeTarget | undefined): void {
    this.targets.pin(originId, target);
  }

  probeTarget(originId: string): RuntimeTarget | undefined {
    return this.targets.resolve(originId);
  }

  beginBrowse(
    originId: string,
    leaseId: string,
    webviewSessionId: string
  ): void {
    this.browseLeases.begin(originId, leaseId, webviewSessionId);
  }

  handoffToBrowse(
    active: {
      readonly sessionId: string;
      readonly requestId: number;
      readonly origin: string;
    } | undefined,
    message: Record<string, unknown>,
    webviewSessionId: string | undefined
  ): boolean {
    if (
      !active ||
      message.sessionId !== active.sessionId ||
      message.requestId !== active.requestId
    ) {
      return false;
    }
    const originRuntimeId =
      typeof message.originRuntimeId === "string"
        ? message.originRuntimeId
        : undefined;
    const leaseId =
      typeof message.leaseId === "string" && message.leaseId.length > 0
        ? message.leaseId
        : undefined;
    const validOrigin =
      message.protocol === "ads" &&
      (active.origin === "this_host"
        ? originRuntimeId === undefined
        : originRuntimeId === active.origin && leaseId !== undefined);
    if (validOrigin && originRuntimeId && leaseId && webviewSessionId) {
      this.beginBrowse(originRuntimeId, leaseId, webviewSessionId);
    } else {
      this.clearCredentials();
    }
    return true;
  }

  browseTarget(
    originId: string,
    leaseId: string | undefined,
    webviewSessionId: string | undefined,
    browseSessionId: string | undefined
  ): RuntimeTarget | undefined {
    return this.browseLeases.bindAndValidate(
      originId,
      leaseId,
      webviewSessionId,
      browseSessionId
    )
      ? this.targets.resolve(originId)
      : undefined;
  }

  releaseBrowse(
    originId: unknown,
    leaseId: unknown,
    browseSessionId?: unknown
  ): void {
    if (this.browseLeases.release(originId, leaseId, browseSessionId)) {
      this.targets.clear();
    }
  }

  updateEndpointRegistry(graph: NCGraph, primaryRuntime: RuntimeTarget): void {
    this.endpoints.clear();
    for (const host of graph.hosts) {
      const runtimes = [
        ...host.runtimes,
        ...host.containers.flatMap((container) => container.runtimes),
      ];
      for (const runtime of runtimes) {
        const endpoint =
          runtime.controlEndpoint?.trim() ||
          endpointFromHostOwnedSyntheticRuntimeId(runtime.id);
        if (endpoint) {
          this.endpoints.set(runtime.id, endpoint);
        } else if (runtime.id === "runtime:local" && primaryRuntime.endpoint) {
          this.endpoints.set(runtime.id, primaryRuntime.endpoint);
        }
      }
    }
  }

  async resolveDiscoveryTarget(
    originId: string,
    echoedEndpoint: string | undefined,
    activeRuntime: RuntimeTarget | undefined
  ): Promise<RuntimeTarget | undefined> {
    if (originId === "this_host") {
      return undefined;
    }
    const endpoint = resolveRegisteredDiscoveryOriginEndpoint(
      this.endpoints,
      originId,
      echoedEndpoint
    );
    if (!endpoint) {
      return undefined;
    }
    if (originId === "runtime:local" && activeRuntime?.endpoint === endpoint) {
      return activeRuntime;
    }
    return resolveRuntimeTargetFromSettings({
      mode: "online",
      endpoint,
      authToken: await getControlAuthToken(endpoint),
      endpointEnabled: true,
      label: originId,
    }).catch(() => undefined);
  }
}

function endpointFromHostOwnedSyntheticRuntimeId(
  runtimeId: string
): string | undefined {
  const prefix = "fleet:";
  const suffix = ":runtime";
  if (!runtimeId.startsWith(prefix) || !runtimeId.endsWith(suffix)) {
    return undefined;
  }
  const endpoint = runtimeId.slice(prefix.length, -suffix.length).trim();
  return endpoint || undefined;
}
