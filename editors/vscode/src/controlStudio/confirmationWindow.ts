import { CONNECTION_LOSS_TIMEOUT_MS } from "./controlStudioLimits";

type StatusRequest = (operation: string, params: Record<string, unknown>) => Promise<unknown>;

/** How long a transfer or parameter change may take to be confirmed, and how often its status is read. */
export interface ConfirmationTiming {
  readonly windowMs: number;
  readonly pollMs: number;
}

/** The floor of the confirmation window (specification 35, A7). */
const MINIMUM_CONFIRMATION_WINDOW_MS = 10_000;
const FASTEST_POLL_MS = 25;
const SLOWEST_POLL_MS = 1_000;

/**
 * The runtime confirms a transfer or parameter change after the loop task's next execution. The window is 10 s, or three
 * executions of a slower task plus the connection-loss timeout (A7). The status is read about four times per execution,
 * at most every second. A runtime that stops answering fails the status request itself.
 */
export async function confirmationTiming(request: StatusRequest, taskName: string): Promise<ConfirmationTiming> {
  const status = await Promise.resolve().then(() => request("status", {})).catch(() => undefined);
  const interval = taskIntervalMs(status, taskName);
  return {
    windowMs: Math.max(MINIMUM_CONFIRMATION_WINDOW_MS, 3 * interval + CONNECTION_LOSS_TIMEOUT_MS),
    pollMs: Math.min(SLOWEST_POLL_MS, Math.max(FASTEST_POLL_MS, interval / 4)),
  };
}

function taskIntervalMs(status: unknown, taskName: string): number {
  const tasks: unknown[] = record(status) && Array.isArray(status.tasks) ? status.tasks : [];
  const task = tasks.find(entry => record(entry) && entry.name === taskName) as Record<string, unknown> | undefined;
  const interval = task?.interval_ms;
  return typeof interval === "number" && Number.isFinite(interval) && interval > 0 ? interval : 0;
}

function record(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
