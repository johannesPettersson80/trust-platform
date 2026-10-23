export interface DiscoveryRequestToken {
  readonly generation: number;
}

/** Owns discovery request identity independently of React rendering state. */
export class DiscoveryRequestTracker<TOwner extends object> {
  private generation = 0;
  private owner: TOwner | undefined;

  start(owner: TOwner): DiscoveryRequestToken {
    this.owner = owner;
    return { generation: ++this.generation };
  }

  invalidate(): void {
    this.owner = undefined;
    this.generation += 1;
  }

  isCurrent(token: DiscoveryRequestToken, owner: TOwner): boolean {
    return this.owner === owner && token.generation === this.generation;
  }
}

export function candidateDisabledReason(
  protocol: string,
  discoverProtocols: ReadonlySet<string>,
  sessionCurrent: boolean,
  originCurrent = true
): string | undefined {
  if (!sessionCurrent) {
    return "Discovery context changed. Scan again before adding this result.";
  }
  if (!originCurrent) {
    return "The runtime that found this result is no longer available. Scan again before adding it.";
  }
  if (!discoverProtocols.has(protocol)) {
    return "This discovery action is no longer available. Refresh the runtime and scan again.";
  }
  return undefined;
}

export function isActiveWebviewSession(
  requestSessionId: string,
  activeSessionId: string | undefined
): boolean {
  return activeSessionId !== undefined && requestSessionId === activeSessionId;
}
