/**
 * The runtime Control Studio is connected to (Specification 35, A2). Its facts come from the runtime's status report, never
 * from editor settings, the debug configuration, files or request labels.
 */

export interface RuntimeTask { name: string; intervalMs: number }

export interface RuntimeTarget {
  /** Simulation is enabled and every attached I/O driver is simulated (A2.4). Anything else is physical. */
  simulated: boolean;
  timeScale: number;
  /** The measured speed of a running simulation (specification 33, §6.1); absent before the first second is measured. */
  achievedTimeScale?: number;
  sessionEpoch?: string;
  tasks: RuntimeTask[];
  cycleIntervalMs?: number;
}

/** Where a live write is applied: the target's simulation, or the physical plant (A2.4). */
export type LiveWriteSource = "simulation" | "physical";

/** The engineer's approval of one reviewed live campaign on a physical target (A2.5). The approval page builds it; the
 * guards here check only that one is present. */
export interface PhysicalApproval { readonly operation: string }

/** The authority a live write carries: its source, and the approval a physical write needs immediately before it starts. */
export interface LiveWriteAuthority { readonly source: LiveWriteSource; readonly approval?: PhysicalApproval }

export interface LiveWritePolicy { readonly source: LiveWriteSource; readonly approvalRequired: boolean }

/** A live write on a physical target was attempted without an approval (A2.5). Callers recognise it by class. */
export class PhysicalApprovalRequired extends Error {
  constructor(readonly operation: string) {
    super(`Physical approval is required for ${operation} on a physical target. Review it on the page and approve it immediately before it starts.`);
    this.name = "PhysicalApprovalRequired";
  }
}

/** Reads one target's facts from a status report. A malformed field takes its safe value: a report that does not state
 * simulation for every attached I/O is physical. */
export function runtimeTargetFromStatus(report: unknown): RuntimeTarget {
  const status: Record<string, unknown> = isRecord(report) ? report : {};
  const enabled = status.simulation_enabled === true;
  const scale = status.simulation_time_scale;
  const epoch = status.session_epoch;
  const cycle = status.cycle_interval_ms;
  const achieved = status.simulation_achieved_time_scale;
  return {
    simulated: enabled && status.simulation_only_io === true,
    timeScale: enabled && isPositive(scale) ? scale : 1,
    ...(enabled && isPositive(achieved) ? { achievedTimeScale: achieved } : {}),
    ...(typeof epoch === "string" && epoch.length > 0 ? { sessionEpoch: epoch } : {}),
    tasks: taskList(status.tasks),
    ...(isPositive(cycle) ? { cycleIntervalMs: cycle } : {}),
  };
}

/** Reads the target through the runtime's own status operation. */
export async function readRuntimeTarget(
  request: (operation: string, params: Record<string, unknown>) => Promise<unknown>
): Promise<RuntimeTarget> {
  const report = await request("status", {});
  if (!isRecord(report)) throw new Error("The runtime returned an invalid status report.");
  return runtimeTargetFromStatus(report);
}

/** A simulated target needs no approval; a physical target always needs one for each live write (A2.4, A2.5). */
export function liveWritePolicy(target: Pick<RuntimeTarget, "simulated">): LiveWritePolicy {
  return target.simulated ? { source: "simulation", approvalRequired: false } : { source: "physical", approvalRequired: true };
}

/** The authority for one live write on a connection, or PhysicalApprovalRequired naming the operation (A2.5). */
export function requireLiveWrite(connection: { readonly source: LiveWriteSource }, operation: string,
  approval?: PhysicalApproval): LiveWriteAuthority {
  const policy = liveWritePolicy({ simulated: connection.source === "simulation" });
  if (!policy.approvalRequired) return { source: policy.source };
  if (approval === undefined) throw new PhysicalApprovalRequired(operation);
  return { source: policy.source, approval };
}

/** A simulation's speed: the configured whole-number speed and, once measured, the achieved speed (specification 33, §6.1). */
export interface SimulationSpeed { readonly configured: number; readonly achieved?: number }

/** The speed of a simulated target, from its report facts. */
export function simulationSpeed(target: Pick<RuntimeTarget, "timeScale" | "achievedTimeScale">): SimulationSpeed {
  return target.achievedTimeScale === undefined
    ? { configured: target.timeScale }
    : { configured: target.timeScale, achieved: target.achievedTimeScale };
}

/**
 * Whether the achieved speed is behind the configured one (specification 35, B9.2): only above 1x, and only when the
 * achieved speed is below 0.95 times the configured speed. At 1x no achieved speed is shown, so nothing is behind.
 */
export function simulationSpeedBehind(speed: SimulationSpeed): boolean {
  return speed.configured > 1 && speed.achieved !== undefined && speed.achieved < speed.configured * 0.95;
}

/**
 * The speed text after a simulation's name, the one rule every surface uses (specification 35, B9.2): "" at 1x; "×N"
 * above 1x; "×A of ×N" only above 1x and when the achieved speed A is below 0.95 times N.
 */
export function simulationSpeedText(speed: SimulationSpeed): string {
  if (speed.configured <= 1) return "";
  if (simulationSpeedBehind(speed) && speed.achieved !== undefined) {
    return `×${formatSpeed(speed.achieved)} of ×${speed.configured}`;
  }
  return `×${speed.configured}`;
}

/** The selected target's name for Control Studio (specification 35, B9.2): "Simulation", "Simulation ×10", "Simulation ×7 of ×10" or "Physical". */
export function simulationTargetLabel(target: Pick<RuntimeTarget, "simulated" | "timeScale" | "achievedTimeScale">): string {
  if (!target.simulated) return "Physical";
  const text = simulationSpeedText(simulationSpeed(target));
  return text ? `Simulation ${text}` : "Simulation";
}

/** One decimal place, rounded down, so a shown achieved speed is never at or above the threshold it is behind. */
function formatSpeed(value: number): string {
  const tenths = Math.floor(value * 10 + 1e-9);
  return tenths % 10 === 0 ? String(tenths / 10) : (tenths / 10).toFixed(1);
}

function taskList(value: unknown): RuntimeTask[] {
  if (!Array.isArray(value)) return [];
  const tasks: RuntimeTask[] = [];
  for (const item of value) {
    if (isRecord(item) && typeof item.name === "string" && item.name.length > 0 && isPositive(item.interval_ms)) {
      tasks.push({ name: item.name, intervalMs: item.interval_ms });
    }
  }
  return tasks;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function isPositive(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value > 0;
}
