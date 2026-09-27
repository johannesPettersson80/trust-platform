/** Schedules one bounded status refresh while a runtime-owned Test remains active. */
export class ExperimentStatusPoller {
  private timer: NodeJS.Timeout | undefined;

  constructor(private readonly refresh: () => void, private readonly delayMs = 250) {}

  schedule(): void {
    if (this.timer) return;
    this.timer = setTimeout(() => {
      this.timer = undefined;
      this.refresh();
    }, this.delayMs);
  }

  dispose(): void {
    if (this.timer) clearTimeout(this.timer);
    this.timer = undefined;
  }
}
