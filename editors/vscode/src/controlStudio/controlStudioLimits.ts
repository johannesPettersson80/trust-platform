/**
 * Control Studio's limits (Specification 35, Part A7). Each limit is enforced in one place per component and quoted from
 * here in messages. Durations are milliseconds of process time unless the name says wall time.
 */
export const MINIMUM_EXPERIMENT_DURATION_MS = 1_000;
export const MAXIMUM_EXPERIMENT_DURATION_MS = 86_400_000;
export const RECORDING_DURATION_LIMIT_MS = 86_400_000;
export const RECORDING_SAMPLE_LIMIT = 100_000;
export const RECORDING_CHANNEL_LIMIT = 32;
export const RECORDING_CELL_LIMIT = 1_600_000;
export const RECORDINGS_PER_RUNTIME = 2;
export const FIT_POINT_LIMIT = 5_000;
export const ISOLATED_SIMULATION_DURATION_LIMIT_MS = 86_400_000;
export const ISOLATED_SIMULATION_SAMPLE_LIMIT = 100_000;
export const ISOLATED_SIMULATION_CHANNEL_LIMIT = 64;
export const ISOLATED_SIMULATION_CELL_LIMIT = 1_600_000;
export const ISOLATED_SIMULATION_SCAN_LIMIT = 10_000_000;
export const ISOLATED_SIMULATION_WALL_TIME_LIMIT_MS = 600_000;
export const SEARCH_TRIAL_LIMIT = 64;
export const SEARCH_WALL_TIME_LIMIT_MS = 3_600_000;
export const APPLY_OBSERVATION_LIMIT_MS = 86_400_000;
/** Restoration is confirmed by the third execution of the loop's task after the restoring write. */
export const RESTORATION_CONFIRMATION_EXECUTIONS = 3;
export const STUDY_STORAGE_LIMIT_BYTES = 256 * 1024 * 1024;

/** Wall time without a newer scan after which a value is stale (A3.4). */
export const CONNECTION_LOSS_TIMEOUT_MS = 5_000;

/** The connection-loss timeout: 5 s, or three resource cycles when that is longer. An unknown cycle keeps 5 s. */
export function connectionLossTimeoutMs(resourceCycleMs?: number): number {
  return resourceCycleMs !== undefined && Number.isFinite(resourceCycleMs) && resourceCycleMs > 0
    ? Math.max(CONNECTION_LOSS_TIMEOUT_MS, 3 * resourceCycleMs)
    : CONNECTION_LOSS_TIMEOUT_MS;
}

/** Samples a recording keeps over the expected settling time of the response it records, and at least in all (A4.2). */
export const SAMPLES_PER_SETTLING_TIME = 200;

/**
 * The record period of A4.2 in executions. It follows the process: about 200 samples over the expected settling time, and at least
 * 200 in all, so a slow process is not recorded at every execution of a fast task. It is never below the smallest period for which
 * the planned duration fits the sample limit, and without an expected settling time that period is the record period.
 */
export function recordPeriodExecutions(plannedDurationMs: number, taskPeriodMs: number, sampleLimit = RECORDING_SAMPLE_LIMIT,
  expectedSettlingSeconds?: number): number {
  const fits = Math.max(1, Math.ceil(plannedDurationMs / (taskPeriodMs * sampleLimit)));
  if (expectedSettlingSeconds === undefined || !(expectedSettlingSeconds > 0)) return fits;
  const perSettling = Math.floor(expectedSettlingSeconds * 1000 / (taskPeriodMs * SAMPLES_PER_SETTLING_TIME));
  const perRecording = Math.floor(plannedDurationMs / (taskPeriodMs * SAMPLES_PER_SETTLING_TIME));
  return Math.max(fits, Math.min(perSettling, perRecording), 1);
}
