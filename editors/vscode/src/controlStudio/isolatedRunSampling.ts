import { ISOLATED_SIMULATION_CELL_LIMIT, ISOLATED_SIMULATION_SAMPLE_LIMIT } from "./controlStudioLimits";

/**
 * The sample grid of an isolated run (Specification 35, A4.2; runtime protocol annex, E4.4 and E8). Plain functions, so the
 * webview's planner and the extension's client compute the same grid.
 */
interface IsolatedRunShape {
  step_ms: number;
  duration_ms: number;
  observations: readonly unknown[];
}

/** The scans a run executes: one at time zero and one per step to the end. */
export function scanCount(request: Pick<IsolatedRunShape, "step_ms" | "duration_ms">): number {
  return Math.floor(request.duration_ms / request.step_ms) + 1;
}

/** The smallest number of scans per recorded sample that keeps a run within the sample and channel-sample limits (A4.2). */
export function isolatedRecordEvery(request: Pick<IsolatedRunShape, "step_ms" | "duration_ms" | "observations">): number {
  const scans = scanCount(request), channels = Math.max(1, request.observations.length);
  let every = Math.max(1, Math.ceil(scans / ISOLATED_SIMULATION_SAMPLE_LIMIT));
  while (Math.ceil(scans / every) * channels > ISOLATED_SIMULATION_CELL_LIMIT) every += 1;
  return every;
}

/** The times of a run's recorded samples: each averages `every` consecutive scans and takes their mean time; the last may average fewer. */
export function recordedSampleTimes(request: Pick<IsolatedRunShape, "step_ms" | "duration_ms">, every: number): number[] {
  const scans = scanCount(request), times: number[] = [];
  for (let first = 0; first < scans; first += every) {
    const last = Math.min(first + every - 1, scans - 1);
    times.push((first + last) * request.step_ms / 2 / 1000);
  }
  return times;
}
