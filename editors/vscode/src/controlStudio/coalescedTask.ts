/** Runs a task once for a burst of requests: after the requests pause for the delay, the latest one runs. */
export class CoalescedTask {
  private timer: ReturnType<typeof setTimeout> | undefined;
  constructor(private readonly delayMs: number) {}
  /** Requests the task; a later request within the delay replaces it. */
  schedule(task: () => void): void {
    this.cancel();
    this.timer = setTimeout(() => { this.timer = undefined; task(); }, this.delayMs);
  }
  /** Drops a request that has not run. */
  cancel(): void {
    if (this.timer !== undefined) clearTimeout(this.timer);
    this.timer = undefined;
  }
}
