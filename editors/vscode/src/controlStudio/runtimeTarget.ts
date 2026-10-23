/**
 * The runtime Control Studio is connected to (Specification 35, A2). Its facts come from the runtime's status report, never
 * from editor settings, the debug configuration, files or request labels.
 */

export interface RuntimeTarget {
  /** Simulation is enabled and every attached I/O driver is simulated (A2.4), as the runtime's status states it. */
  simulated: boolean;
  timeScale: number;
  /** The measured speed of a running simulation (specification 46, §6.1); absent before the first second is measured. */
  achievedTimeScale?: number;
  /** The speed words the runtime gives (specification 35, B9.2): "" at 1×, "×N", or "×A of ×N" when the speed is behind. */
  speedText: string;
  /** Whether the runtime says the achieved speed is behind the configured one (specification 35, B9.2). */
  speedBehind: boolean;
  cycleIntervalMs?: number;
}

/**
 * Types one target's facts from the runtime's status report. The runtime decides whether the target is simulated and words its speed
 * (specification 35, A2.4 and B9.2); this reads the report's fields as the runtime states them. Where the report states none it applies
 * a default of its own, the stricter reading: a report without `simulated` is a physical target, without a speed word it is at 1×, and
 * without a usable time scale the speed is 1. The numbers (`timeScale`, `achievedTimeScale`) only name the speed that the chooser highlights.
 */
export function runtimeTargetFromStatus(report: unknown): RuntimeTarget {
  const status: Record<string, unknown> = isRecord(report) ? report : {};
  const enabled = status.simulation_enabled === true;
  const scale = status.simulation_time_scale;
  const cycle = status.cycle_interval_ms;
  const achieved = status.simulation_achieved_time_scale;
  return {
    simulated: status.simulated === true,
    timeScale: enabled && isPositive(scale) ? scale : 1,
    ...(enabled && isPositive(achieved) ? { achievedTimeScale: achieved } : {}),
    speedText: typeof status.simulation_speed_text === "string" ? status.simulation_speed_text : "",
    speedBehind: status.simulation_speed_behind === true,
    ...(isPositive(cycle) ? { cycleIntervalMs: cycle } : {}),
  };
}

/** A simulation's speed: the configured whole-number speed and, once measured, the achieved speed (specification 46, §6.1). */
export interface SimulationSpeed { readonly configured: number; readonly achieved?: number }

/** The speed of a simulated target, from its report facts. */
export function simulationSpeed(target: Pick<RuntimeTarget, "timeScale" | "achievedTimeScale">): SimulationSpeed {
  return target.achievedTimeScale === undefined
    ? { configured: target.timeScale }
    : { configured: target.timeScale, achieved: target.achievedTimeScale };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function isPositive(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value > 0;
}
