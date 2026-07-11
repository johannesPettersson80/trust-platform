import type { RuntimeTarget } from "../runtimeTarget";

/** Keeps discovery execution credentials on the extension host, keyed by immutable origin id. */
export class DiscoveryOriginTargetStore {
  private readonly targets = new Map<string, RuntimeTarget>();

  pin(originId: string, target: RuntimeTarget | undefined): void {
    if (originId === "this_host" || !target) {
      this.targets.delete(originId);
      return;
    }
    this.targets.set(originId, target);
  }

  resolve(originId: string): RuntimeTarget | undefined {
    return this.targets.get(originId);
  }

  clear(): void {
    this.targets.clear();
  }
}

export function resolveRegisteredDiscoveryOriginEndpoint(
  endpoints: ReadonlyMap<string, string>,
  originId: string,
  echoedEndpoint?: string
): string | undefined {
  const registered = endpoints.get(originId);
  const echoed = echoedEndpoint?.trim();
  return registered && (!echoed || echoed === registered)
    ? registered
    : undefined;
}
