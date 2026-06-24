import { debugChannel } from "./debug/configuration";
import type { ManagedLifecycleResult } from "./localRuntimeModel";
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

  const connect = await runtimeLifecycleService.connectRemote(result.controlEndpoint);
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
  result: ManagedLifecycleResult
): Promise<void> {
  if (!result.controlEndpoint) {
    debugChannel().appendLine(
      "Managed runtime stopped without a reported control endpoint; no Live Values session was disconnected."
    );
    return;
  }
  const snapshot = await runtimeLifecycleService.snapshot();
  if (
    snapshot.status.runtimeMode === "online" &&
    snapshot.status.runtimeState === "connected" &&
    snapshot.status.endpoint === result.controlEndpoint
  ) {
    await runtimeLifecycleService.stopRuntime();
  }
}
