/** Owns the Network Canvas refresh timer independently from panel orchestration. */
export class NetworkCanvasPolling {
  private timer: NodeJS.Timeout | undefined;

  constructor(
    private readonly refresh: () => void | Promise<void>,
    private readonly intervalMs: number
  ) {}

  start(): void {
    if (this.timer) {
      return;
    }
    this.timer = setInterval(() => {
      void this.refresh();
    }, this.intervalMs);
  }

  stop(): void {
    if (!this.timer) {
      return;
    }
    clearInterval(this.timer);
    this.timer = undefined;
  }
}
