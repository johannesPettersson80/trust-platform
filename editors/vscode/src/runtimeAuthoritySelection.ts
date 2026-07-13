import type { ManagedRuntime } from "./localRuntimeModel";
import type {
  RuntimeLifecycleSnapshot,
  RuntimeLifecycleTarget,
} from "./runtimeLifecycleModel";
import {
  remoteLabelFromEndpoint,
  SIMULATOR_RUNTIME_ID,
  type RemoteRuntime,
  type RuntimeModelSnapshot,
} from "./trustHomeModel";

export interface RuntimeAuthoritySelection {
  readonly target?: RuntimeLifecycleTarget;
  readonly selectedId: string;
  readonly remotes: RemoteRuntime[];
  /** Managed target that still owns the accepted attach session, if any. */
  readonly managedSessionId?: string;
}

/**
 * An in-flight or accepted session is stronger than persisted UI selection.
 * Remote sessions absent from fleet settings are added only to this render;
 * no configuration is mutated merely to show an active attach.
 */
export function runtimeAuthoritySelection(
  snapshot: RuntimeLifecycleSnapshot,
  configuredRemotes: ReadonlyArray<RemoteRuntime>,
  managed: ReadonlyArray<ManagedRuntime>,
  storedSelectedId: string,
): RuntimeAuthoritySelection {
  const rawTarget = snapshot.starting
    ? snapshot.transitionTarget
    : snapshot.activeTarget;
  const target = normalizeRuntimeAuthorityTarget(
    rawTarget,
    managed,
    snapshot.status.targetLabel,
  );
  if (!target) {
    return {
      selectedId: storedSelectedId,
      remotes: [...configuredRemotes],
    };
  }
  if (target.kind === "simulator") {
    return {
      target,
      selectedId: SIMULATOR_RUNTIME_ID,
      remotes: [...configuredRemotes],
    };
  }
  if (target.kind === "managed") {
    return {
      target,
      selectedId: target.id,
      remotes: [...configuredRemotes],
      managedSessionId: target.id,
    };
  }

  return remoteAuthoritySelection(target, snapshot, configuredRemotes);
}

/**
 * Resolves a lifecycle target once for every UI surface. Explicit managed IDs
 * are accepted only when their endpoint agrees; legacy remote attaches map to
 * a managed runtime only when exactly one configured endpoint matches.
 */
export function normalizeRuntimeAuthorityTarget(
  target: RuntimeLifecycleTarget | undefined,
  managed: ReadonlyArray<ManagedRuntime>,
  targetLabel?: string,
): RuntimeLifecycleTarget | undefined {
  if (!target || target.kind === "simulator") {
    return target;
  }
  const targetEndpoint = target.endpoint?.trim() ?? "";
  const endpointMatches = targetEndpoint
    ? managed.filter(
        (runtime) => runtime.controlEndpoint.trim() === targetEndpoint,
      )
    : [];
  if (target.kind === "managed") {
    const declared = managed.find((runtime) => runtime.name === target.id);
    const declaredEndpoint = declared?.controlEndpoint.trim() ?? "";
    if (
      declared &&
      (!targetEndpoint ||
        !declaredEndpoint ||
        targetEndpoint === declaredEndpoint)
    ) {
      return {
        kind: "managed",
        id: declared.name,
        ...(targetEndpoint || declaredEndpoint
          ? { endpoint: targetEndpoint || declaredEndpoint }
          : {}),
      };
    }
    if (!targetEndpoint) {
      // A managed Start operation is created from a selected inventory item
      // before its control endpoint is known. Its explicit operation ID is the
      // only available authority during that short transition.
      return target;
    }
  }
  if (endpointMatches.length === 1) {
    return {
      kind: "managed",
      id: endpointMatches[0].name,
      endpoint: targetEndpoint,
    };
  }
  return {
    kind: "remote",
    endpoint: targetEndpoint,
    ...(target.kind === "remote" && target.label?.trim()
      ? { label: target.label.trim() }
      : targetLabel?.trim()
        ? { label: targetLabel.trim() }
        : {}),
  };
}

function remoteAuthoritySelection(
  target: Extract<RuntimeLifecycleTarget, { readonly kind: "remote" }>,
  snapshot: RuntimeLifecycleSnapshot,
  configuredRemotes: ReadonlyArray<RemoteRuntime>,
): RuntimeAuthoritySelection {
  const remotes = [...configuredRemotes];
  if (
    target.endpoint &&
    !remotes.some((runtime) => runtime.id === target.endpoint)
  ) {
    remotes.push({
      id: target.endpoint,
      label:
        target.label?.trim() ||
        snapshot.status.targetLabel?.trim() ||
        remoteLabelFromEndpoint(target.endpoint),
    });
  }
  return {
    target,
    selectedId: target.endpoint,
    remotes,
  };
}

/**
 * Projects transition/accepted-session authority over configuration-derived
 * status. This keeps direct F5 launch/attach honest even when persisted mode,
 * endpoint, or selected target is stale.
 */
export function runtimeModelSnapshotForLifecycle(
  snapshot: RuntimeLifecycleSnapshot,
  normalizedTarget?: RuntimeLifecycleTarget,
): RuntimeModelSnapshot {
  const target =
    normalizedTarget ??
    (snapshot.starting ? snapshot.transitionTarget : snapshot.activeTarget);
  const transitionTargetId = targetId(target);

  if (target?.kind === "simulator") {
    return {
      runtimeMode: "simulate",
      runtimeState: snapshot.starting ? "stopped" : "running",
      endpoint: "",
      endpointConfigured: false,
      endpointReachable: !snapshot.starting,
      starting: snapshot.starting,
      transitionTargetId,
    };
  }
  if (target?.kind === "remote") {
    return {
      runtimeMode: "online",
      runtimeState: snapshot.starting ? "stopped" : "connected",
      endpoint: target.endpoint,
      endpointConfigured: Boolean(target.endpoint),
      endpointReachable: snapshot.starting
        ? snapshot.status.endpointReachable
        : true,
      starting: snapshot.starting,
      transitionTargetId,
    };
  }

  return {
    runtimeMode: snapshot.status.runtimeMode,
    runtimeState: snapshot.status.runtimeState,
    endpoint: snapshot.status.endpoint,
    endpointConfigured: snapshot.status.endpointConfigured,
    endpointReachable: snapshot.status.endpointReachable,
    starting: snapshot.starting,
    transitionTargetId,
  };
}

function targetId(
  target: RuntimeLifecycleTarget | undefined,
): string | undefined {
  switch (target?.kind) {
    case "simulator":
      return SIMULATOR_RUNTIME_ID;
    case "managed":
      return target.id;
    case "remote":
      return target.endpoint;
    default:
      return undefined;
  }
}
