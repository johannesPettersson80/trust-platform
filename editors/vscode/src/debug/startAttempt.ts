import type {
  RuntimeLifecycleOperationKind,
  RuntimeLifecycleOperationState,
  RuntimeLifecycleTarget,
} from "../runtimeLifecycleModel";

export const LIFECYCLE_START_ATTEMPT_FIELD = "lifecycleAttemptId";

export type LifecycleSessionDisposition =
  | "external"
  | "active"
  | "accepted"
  | "rejected";

/**
 * Owns the identity of the single Structured Text lifecycle transition. VS
 * Code cannot cancel a pending startDebugging call, so a
 * session may be announced after our timeout. Only the current attempt (or an
 * already accepted session) is allowed to affect product state.
 */
export class RuntimeLifecycleAttemptRegistry {
  private sequence = 0;
  private activeOperation: RuntimeLifecycleOperationState | undefined;

  constructor(
    private readonly instanceId = `${Date.now().toString(36)}-${process.pid.toString(36)}`
  ) {}

  begin(
    kind: RuntimeLifecycleOperationKind = "local_start",
    target: RuntimeLifecycleTarget = { kind: "simulator" }
  ): string {
    if (this.activeOperation) {
      throw new Error("A runtime operation is already in progress.");
    }
    this.sequence += 1;
    const id = `${this.instanceId}-${this.sequence.toString(36)}`;
    this.activeOperation = { id, kind, target };
    return id;
  }

  active(): string | undefined {
    return this.activeOperation?.id;
  }

  current(): RuntimeLifecycleOperationState | undefined {
    return this.activeOperation;
  }

  accept(attemptId: string): void {
    if (this.activeOperation?.id === attemptId) {
      this.activeOperation = undefined;
    }
  }

  reject(attemptId: string): void {
    if (this.activeOperation?.id === attemptId) {
      this.activeOperation = undefined;
    }
  }

  cancel(): void {
    this.activeOperation = undefined;
  }

  disposition(
    attemptId: string | undefined,
    sessionAccepted: boolean,
    sessionRejected = false,
  ): LifecycleSessionDisposition {
    if (sessionRejected) {
      return "rejected";
    }
    if (sessionAccepted) {
      return "accepted";
    }
    if (!attemptId) {
      return "external";
    }
    return attemptId === this.activeOperation?.id ? "active" : "rejected";
  }
}

export { RuntimeLifecycleAttemptRegistry as RuntimeStartAttemptRegistry };

export function lifecycleStartAttemptId(value: unknown): string | undefined {
  if (!value || typeof value !== "object") {
    return undefined;
  }
  const raw = (value as Record<string, unknown>)[LIFECYCLE_START_ATTEMPT_FIELD];
  return typeof raw === "string" && raw.trim() ? raw.trim() : undefined;
}

export function findLifecycleSessionForAttempt<
  T extends { readonly type: string; readonly configuration: unknown },
>(
  active: T | undefined,
  tracked: Iterable<T>,
  attemptId: string,
  debugType: string,
): T | undefined {
  if (
    active?.type === debugType &&
    lifecycleStartAttemptId(active.configuration) === attemptId
  ) {
    return active;
  }
  for (const session of tracked) {
    if (lifecycleStartAttemptId(session.configuration) === attemptId) {
      return session;
    }
  }
  return undefined;
}
