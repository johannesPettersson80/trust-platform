import type { ControlAuxiliarySignalRole, ControlSignalDirection } from "./loopDefinition";

/** The loop's declared experiment execution mode, as read from `loop.experiment?.execution`. */
export type ExperimentExecutionMode = "control-studio" | "project-scripted" | undefined;

export type PrimaryControlSignalName =
  | "setpoint" | "processVariable" | "manipulatedVariable" | "actuator";

/**
 * A project-scripted lesson's runnable PLC project owns every primary signal, including
 * the manipulated variable, so Control Studio may only read it there; a Control
 * Studio-driven experiment still owns and writes the manipulated variable itself.
 */
export function expectedPrimarySignalDirection(
  execution: ExperimentExecutionMode,
  signal: PrimaryControlSignalName
): ControlSignalDirection {
  if (execution === "project-scripted") return "read";
  return signal === "manipulatedVariable" ? "write" : "read";
}

/**
 * A project-scripted lesson's project also owns every auxiliary signal, so no role may
 * write once Control Studio has stepped back to pure observation; a Control Studio-driven
 * experiment keeps today's per-role contract (disturbance either way, MV write, else read).
 */
export function isAuxiliarySignalDirectionAccepted(
  execution: ExperimentExecutionMode,
  role: ControlAuxiliarySignalRole,
  direction: ControlSignalDirection
): boolean {
  if (execution === "project-scripted") return direction === "read";
  if (role === "disturbance") return direction === "read" || direction === "write";
  return direction === (role === "MV" ? "write" : "read");
}
