export interface LatestRefreshContext {
  readonly generation: number;
  isCurrent(): boolean;
}

export function networkCanvasRefreshDelayMs(): number {
  const value = Number(
    process.env.TRUST_VSCODE_NETWORK_CANVAS_REFRESH_DELAY_MS ?? 0
  );
  if (!Number.isFinite(value) || value <= 0) {
    return 0;
  }
  return Math.min(Math.floor(value), 10_000);
}

type RefreshTask = (context: LatestRefreshContext) => Promise<void>;

interface PendingRefresh {
  readonly generation: number;
  readonly task: RefreshTask;
}

/**
 * Serializes refresh work and coalesces bursts to the newest requested task.
 * A task may perform slow reads, but it must gate its final commit through
 * `context.isCurrent()` so an older snapshot can never overwrite newer state.
 */
export class LatestRefreshCoordinator {
  private requestedGeneration = 0;
  private pending: PendingRefresh | undefined;
  private running: Promise<void> | undefined;

  request(task: RefreshTask): Promise<void> {
    const generation = ++this.requestedGeneration;
    this.pending = { generation, task };
    if (!this.running) {
      this.running = this.drain();
    }
    return this.running;
  }

  invalidate(): void {
    this.requestedGeneration += 1;
    this.pending = undefined;
  }

  private async drain(): Promise<void> {
    try {
      while (this.pending) {
        const current = this.pending;
        this.pending = undefined;
        await current.task({
          generation: current.generation,
          isCurrent: () => current.generation === this.requestedGeneration,
        });
      }
    } finally {
      // Clear the single-flight marker in the same async continuation that observed
      // the queue empty. A request can otherwise arrive after `drain()` resolves but
      // before an external promise finalizer runs, leaving that request stranded.
      this.running = undefined;
      if (this.pending) {
        this.running = this.drain();
      }
    }
  }
}
