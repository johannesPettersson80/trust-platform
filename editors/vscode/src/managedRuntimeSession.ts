import { debugChannel } from "./debug/configuration";
import { waitForEndpointReachable } from "./io-panel/status";
import {
  managedRuntimeLabel,
  type ManagedLifecycleResult,
} from "./localRuntimeModel";
import {
  runtimeLifecycleService,
  type RuntimeLifecycleResult,
  type RuntimeLifecycleSnapshot,
  type RuntimeLifecycleTarget,
} from "./runtimeLifecycle";
import { setSelectedRuntimeId } from "./selectedRuntime";

export interface ManagedRuntimeAttachResult {
  readonly ok: boolean;
  readonly message?: string;
}

export interface ManagedRuntimeDisconnectDependencies {
  readonly snapshot: () => Promise<RuntimeLifecycleSnapshot>;
  readonly stopRuntime: (
    operationId: string,
  ) => Promise<RuntimeLifecycleResult>;
}

export async function attachManagedRuntimeAfterStart(
  name: string,
  result: ManagedLifecycleResult,
  operationId?: string,
): Promise<ManagedRuntimeAttachResult> {
  if (!result.ok) {
    return {
      ok: false,
      message: result.message || `Could not start ${name}.`,
    };
  }
  if (!result.controlEndpoint) {
    const message =
      `Runtime ${name} started, but it did not report a control endpoint. ` +
      "Live Values could not connect.";
    debugChannel().appendLine(message);
    return { ok: false, message };
  }

  // A freshly-started managed runtime's control socket can need a beat to bind. Poll for readiness
  // (cache-bypassing) before attaching, so a cold Start does not surface a false "Live Values could not
  // connect" on the happy path (F-11). A genuinely unreachable endpoint still falls through to the honest
  // failure below — we never fabricate a connection.
  await waitForEndpointReachable(result.controlEndpoint);

  const connect = operationId
    ? await runtimeLifecycleService.connectRemoteWithinOperation(
        operationId,
        result.controlEndpoint,
        managedRuntimeLabel(name),
        name,
      )
    : await runtimeLifecycleService.connectRemote(
        result.controlEndpoint,
        managedRuntimeLabel(name),
      );
  if (!connect.ok) {
    return {
      ok: false,
      message: `Runtime started, but Live Values could not connect: ${connect.failure.message}`,
    };
  }
  await setSelectedRuntimeId(name);
  return { ok: true };
}

export async function disconnectManagedRuntimeAfterStop(
  name: string,
  result: ManagedLifecycleResult,
  operationId: string,
  validatedAuthority: RuntimeLifecycleTarget | undefined,
  lifecycle: ManagedRuntimeDisconnectDependencies = runtimeLifecycleService,
): Promise<RuntimeLifecycleResult> {
  const snapshot = await lifecycle.snapshot();
  const stoppedEndpoint = result.controlEndpoint?.trim();
  const attachedEndpoint = snapshot.status.endpoint.trim();
  const sameEndpoint =
    !!stoppedEndpoint && attachedEndpoint === stoppedEndpoint;
  const authorityEndpoint =
    validatedAuthority?.kind === "managed" ||
    validatedAuthority?.kind === "remote"
      ? validatedAuthority.endpoint?.trim()
      : undefined;
  const sameManagedTarget =
    validatedAuthority?.kind === "managed" &&
    validatedAuthority.id === name &&
    (!authorityEndpoint || authorityEndpoint === attachedEndpoint) &&
    (!stoppedEndpoint ||
      !authorityEndpoint ||
      authorityEndpoint === stoppedEndpoint);
  const sameLegacyRemoteTarget =
    validatedAuthority?.kind === "remote" &&
    authorityEndpoint === attachedEndpoint &&
    sameEndpoint;
  if (
    snapshot.status.runtimeMode === "online" &&
    snapshot.status.runtimeState === "connected"
  ) {
    if (sameManagedTarget || sameLegacyRemoteTarget) {
      return lifecycle.stopRuntime(operationId);
    }
    return {
      ok: false,
      failure: {
        kind: "stale_runtime",
        message:
          `Runtime ${name} stopped, but the remaining Live Values session ` +
          "could not be safely matched. Disconnect it before starting another runtime.",
      },
    };
  }
  if (!stoppedEndpoint) {
    debugChannel().appendLine(
      `Managed runtime ${name} stopped without a reported control endpoint; no matching Live Values session was connected.`,
    );
  }
  return {
    ok: true,
    message: `Managed runtime ${name} stopped; no matching Live Values session was connected.`,
  };
}
