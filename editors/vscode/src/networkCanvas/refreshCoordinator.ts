export interface LatestRefreshContext {
  readonly generation: number;
  isCurrent(): boolean;
}

type RefreshTask = (context: LatestRefreshContext) => Promise<void>;

interface PendingRefresh {
  readonly generation: number;
  readonly task: RefreshTask;
}

/**
 * Serializes refresh work and coalesces bursts to the newest pending task.
 * Slow active work remains current until the owner explicitly invalidates it;
 * ordinary polling must not starve every commit merely by queuing a follow-up.
 */
export class LatestRefreshCoordinator {
  private requestedGeneration = 0;
  private invalidatedGeneration = 0;
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
    this.invalidatedGeneration = this.requestedGeneration;
    this.pending = undefined;
  }

  private async drain(): Promise<void> {
    try {
      while (this.pending) {
        const current = this.pending;
        this.pending = undefined;
        await current.task({
          generation: current.generation,
          isCurrent: () => current.generation > this.invalidatedGeneration,
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
