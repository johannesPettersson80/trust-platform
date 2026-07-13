/** Owns the Network Canvas refresh timer independently from panel orchestration. */
export class NetworkCanvasPolling {
  private timer: NodeJS.Timeout | undefined;
  private refreshInFlight = false;

  constructor(
    private readonly refresh: () => void | Promise<void>,
    private readonly intervalMs: number
  ) {}

  start(): void {
    if (this.timer) {
      return;
    }
    this.timer = setInterval(() => {
      void this.poll();
    }, this.intervalMs);
  }

  stop(): void {
    if (!this.timer) {
      return;
    }
    clearInterval(this.timer);
    this.timer = undefined;
  }

  private async poll(): Promise<void> {
    if (this.refreshInFlight) {
      return;
    }
    this.refreshInFlight = true;
    try {
      await this.refresh();
    } finally {
      this.refreshInFlight = false;
    }
  }
}
