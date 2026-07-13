/**
 * Selects the one session exposed to lifecycle consumers. An accepted session
 * always wins over a merely active/pending session, and rejected duplicates
 * are invisible before VS Code finishes terminating them.
 */
export function selectLifecycleDebugSession<T>(
  active: T | undefined,
  tracked: Iterable<T>,
  keyOf: (session: T) => string,
  isAccepted: (session: T) => boolean,
  isRejected: (session: T) => boolean
): T | undefined {
  const candidates = new Map<string, T>();
  if (active) {
    candidates.set(keyOf(active), active);
  }
  for (const session of tracked) {
    const key = keyOf(session);
    if (!candidates.has(key)) {
      candidates.set(key, session);
    }
  }
  for (const session of candidates.values()) {
    if (!isRejected(session) && isAccepted(session)) {
      return session;
    }
  }
  for (const session of candidates.values()) {
    if (!isRejected(session)) {
      return session;
    }
  }
  return undefined;
}

export function debugSessionAcceptancePath(
  request: unknown
): "remote_attach" | "local_simulator" {
  return request === "attach" ? "remote_attach" : "local_simulator";
}

export function terminatedSessionOwnsLifecycleState(
  wasTracked: boolean,
  wasCurrentAttempt: boolean
): boolean {
  return wasTracked || wasCurrentAttempt;
}
