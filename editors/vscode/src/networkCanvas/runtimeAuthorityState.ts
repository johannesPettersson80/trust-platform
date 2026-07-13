import type { ManagedRuntime } from "../localRuntimeModel";
import type { RuntimeLifecycleTarget } from "../runtimeLifecycleModel";
import { normalizeRuntimeAuthorityTarget } from "../runtimeAuthoritySelection";

/**
 * Owns the one inventory-validated lifecycle authority used by both canvas
 * rendering and host mutation. Raw debug-session metadata never authorizes a
 * managed process action before current inventory has confirmed it.
 */
export class NetworkCanvasRuntimeAuthority {
  private managed: ReadonlyArray<ManagedRuntime> = [];
  private inventoryReady = false;
  private target: RuntimeLifecycleTarget | undefined;
  private terminalProjectionTarget: RuntimeLifecycleTarget | undefined;

  beginFirstPaint(
    rawTarget: RuntimeLifecycleTarget | undefined,
  ): RuntimeLifecycleTarget | null | undefined {
    this.managed = [];
    this.inventoryReady = false;
    this.target = rawTarget?.kind === "simulator" ? rawTarget : undefined;
    this.terminalProjectionTarget = undefined;
    return this.target ?? (rawTarget ? null : undefined);
  }

  invalidateInventory(rawTarget: RuntimeLifecycleTarget | undefined): void {
    this.inventoryReady = false;
    this.captureTerminalSimulator(rawTarget);
    this.target = rawTarget?.kind === "simulator" ? rawTarget : undefined;
    if (rawTarget) {
      this.terminalProjectionTarget = undefined;
    }
  }

  reconcile(
    rawTarget: RuntimeLifecycleTarget | undefined,
    targetLabel?: string,
  ): RuntimeLifecycleTarget | undefined {
    this.captureTerminalSimulator(rawTarget);
    this.target =
      rawTarget?.kind === "simulator"
        ? rawTarget
        : this.inventoryReady
          ? normalizeRuntimeAuthorityTarget(
              rawTarget,
              this.managed,
              targetLabel,
            )
          : undefined;
    if (rawTarget) {
      this.terminalProjectionTarget = undefined;
    }
    return this.target;
  }

  acceptInventory(
    rawTarget: RuntimeLifecycleTarget | undefined,
    managed: ReadonlyArray<ManagedRuntime>,
    targetLabel?: string,
  ): RuntimeLifecycleTarget | undefined {
    this.managed = [...managed];
    this.inventoryReady = true;
    return this.reconcile(rawTarget, targetLabel);
  }

  activeTarget(): RuntimeLifecycleTarget | undefined {
    return this.target;
  }

  /**
   * Rendering keeps the just-stopped Simulator as terminal owner for one stable
   * stopped view. Mutation authority remains `activeTarget()` and is empty.
   */
  lifecycleProjectionTarget(): RuntimeLifecycleTarget | undefined {
    return this.target ?? this.terminalProjectionTarget;
  }

  managedRuntimes(): ReadonlyArray<ManagedRuntime> {
    return this.managed;
  }

  managedTarget(
    name: string,
    endpoint: string,
  ): Extract<RuntimeLifecycleTarget, { readonly kind: "managed" }> | undefined {
    if (!this.inventoryReady) {
      return undefined;
    }
    const normalizedName = name.trim();
    const normalizedEndpoint = endpoint.trim();
    const matches = this.managed.filter(
      (runtime) =>
        runtime.name === normalizedName &&
        runtime.controlEndpoint.trim() === normalizedEndpoint,
    );
    if (matches.length !== 1) {
      return undefined;
    }
    return {
      kind: "managed",
      id: matches[0].name,
      ...(normalizedEndpoint ? { endpoint: normalizedEndpoint } : {}),
    };
  }

  reset(): void {
    this.managed = [];
    this.inventoryReady = false;
    this.target = undefined;
    this.terminalProjectionTarget = undefined;
  }

  private captureTerminalSimulator(
    nextRawTarget: RuntimeLifecycleTarget | undefined,
  ): void {
    if (!nextRawTarget && this.target?.kind === "simulator") {
      this.terminalProjectionTarget = { kind: "simulator" };
    }
  }
}
