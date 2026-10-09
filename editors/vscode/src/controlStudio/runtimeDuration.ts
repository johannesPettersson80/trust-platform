import { CONNECTION_LOSS_TIMEOUT_MS } from "./controlStudioLimits";
import { MainWorkflowError } from "./mainWorkflowFailure";

/** The runtime's clock in nanoseconds, from the scan that a status report carries (A3.1). A report without one cannot be observed. */
export function runtimeClockNs(status: { runtime_time_ns?: number }): number {
  const time = status.runtime_time_ns;
  if (typeof time !== "number" || !Number.isFinite(time)) throw new Error("The runtime does not report its clock in the transfer status.");
  return time;
}

/**
 * Process time and receipt supervision for one operation that reads the runtime (Specification 35, A3.1, A3.4). Process time
 * runs from the runtime timestamps of the readings, never from this host's clock. The connection-loss rule compares receipts
 * on this receiver's own clock: a newer runtime value must arrive within the connection-loss timeout.
 */
export class RuntimeReadingClock {
  private firstNs: number | undefined;
  private newestNs: number | undefined;
  private newestReceiptMs: number;

  constructor(private readonly wallClockMs: () => number = Date.now) {
    this.newestReceiptMs = wallClockMs();
  }

  /** Records one reading and returns the process time elapsed from the first reading to the newest one, in milliseconds. */
  observe(status: { runtime_time_ns?: number }): number {
    const time = runtimeClockNs(status);
    const first = this.firstNs ?? time;
    this.firstNs = first;
    const newest = this.newestNs;
    if (newest === undefined || time > newest) {
      this.newestNs = time;
      this.newestReceiptMs = this.wallClockMs();
    }
    return ((this.newestNs ?? time) - first) / 1e6;
  }

  /** Stops with "The runtime stopped sending values." when no newer runtime value arrived within the connection-loss timeout. */
  ensureFresh(): void {
    if (this.wallClockMs() - this.newestReceiptMs > CONNECTION_LOSS_TIMEOUT_MS) {
      throw new MainWorkflowError("The runtime stopped sending values.", { cause: "runtime", nextAction: "connect-runtime" });
    }
  }
}
