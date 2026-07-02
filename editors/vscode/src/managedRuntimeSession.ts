import { debugChannel } from "./debug/configuration";
import { waitForEndpointReachable } from "./io-panel/status";
import {
  managedRuntimeLabel,
  type ManagedLifecycleResult,
} from "./localRuntimeModel";
import { runtimeLifecycleService } from "./runtimeLifecycle";
import { setSelectedRuntimeId } from "./selectedRuntime";

export interface ManagedRuntimeAttachResult {
  readonly ok: boolean;
  readonly message?: string;
}

export async function attachManagedRuntimeAfterStart(
  name: string,
  result: ManagedLifecycleResult
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

  const connect = await runtimeLifecycleService.connectRemote(
    result.controlEndpoint,
    managedRuntimeLabel(name)
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
  result: ManagedLifecycleResult
): Promise<void> {
  const snapshot = await runtimeLifecycleService.snapshot();
  const stoppedEndpoint = result.controlEndpoint?.trim();
  const attachedEndpoint = snapshot.status.endpoint.trim();
  const attachedLabel = snapshot.status.targetLabel?.trim();
  const managedLabel = managedRuntimeLabel(name);
  const sameEndpoint =
    !!stoppedEndpoint && attachedEndpoint === stoppedEndpoint;
  const sameManagedTarget = attachedLabel === managedLabel;
  if (
    snapshot.status.runtimeMode === "online" &&
    snapshot.status.runtimeState === "connected" &&
    (sameEndpoint || sameManagedTarget)
  ) {
    await runtimeLifecycleService.stopRuntime();
    return;
  }
  if (!stoppedEndpoint) {
    debugChannel().appendLine(
      `Managed runtime ${name} stopped without a reported control endpoint; no matching Live Values session was connected.`
    );
  }
}
