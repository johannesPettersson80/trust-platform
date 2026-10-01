import { formatEngineeringNumber } from "./chartContract";

/**
 * Waits until the runtime has executed `durationMs` of its own time (Specification 35 §12). A runtime that runs behind
 * real time, under load or on a slower controller, is still observed for the whole duration; one that falls further
 * behind than `boundMs` of wall-clock time is reported.
 */
export async function awaitRuntimeDuration(clock: () => Promise<number>, durationMs: number,
  options: { checkCancelled: () => void; boundMs: number; pollMs?: number }): Promise<void> {
  const start = await clock();
  const deadline = Date.now() + options.boundMs;
  for (;;) {
    options.checkCancelled();
    const executedMs = (await clock() - start) / 1e6;
    if (executedMs >= durationMs) return;
    if (Date.now() > deadline) {
      throw new Error(`The runtime executed ${formatEngineeringNumber(Math.max(0, executedMs) / 1000)} s of the reviewed ${formatEngineeringNumber(durationMs / 1000)} s `
        + `within ${formatEngineeringNumber(options.boundMs / 1000)} s; it runs too far behind real time to observe the response.`);
    }
    await new Promise(resolve => setTimeout(resolve, options.pollMs ?? 100));
  }
}

/** The wall-clock bound for observing `durationSeconds` of runtime time. */
export function runtimeObservationBoundMs(durationSeconds: number): number {
  return Math.max(3 * durationSeconds, durationSeconds + 60) * 1000;
}

/** The runtime's own clock in nanoseconds, from the committed scan that a transfer status reports. */
export function transferClock(transfer: { status(): Promise<{ runtime_time_ns?: number }> }): () => Promise<number> {
  return async () => {
    const time = (await transfer.status()).runtime_time_ns;
    if (typeof time !== "number" || !Number.isFinite(time)) throw new Error("The runtime does not report its clock in the transfer status.");
    return time;
  };
}
