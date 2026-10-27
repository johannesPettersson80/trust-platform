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

export function isActiveWebviewSession(
  requestSessionId: string,
  activeSessionId: string | undefined
): boolean {
  return activeSessionId !== undefined && requestSessionId === activeSessionId;
}
