import type { LiveSides } from "../generated/OpenResult";
import { displayEngineeringUnit } from "../chartContract";
import { isAuxiliary } from "./plotSignalKinds";

/**
 * Names a signal by its role in the loop (Specification 35 B9.9): Measurement, Output, Setpoint, or an auxiliary channel's id. The names come from
 * the engine's open answer (`liveSides`, B3.6, B9.6): the measured outputs and the plant inputs in channel order, and the signals the plots draw
 * with their kinds (`PlotSignal.kind`). Several channels of one kind are numbered in the engine's order, so every name stays distinct. The page
 * resolves no signal of the loop and reads no identifier. The full symbol is the hover text.
 */
export function signalRoleName(sides: LiveSides | null | undefined, symbol: string): string | undefined {
  if (!sides) return undefined;
  const measured = sides.outputs.map(signal => signal.symbol);
  const actuated = sides.inputs.map(signal => signal.symbol);
  if (measured.includes(symbol)) return numbered("Measurement", measured, symbol);
  if (actuated.includes(symbol)) return numbered("Output", actuated, symbol);
  const plotted = sides.plotSignals?.find(signal => signal.symbol === symbol);
  if (plotted?.kind === "setpoint") return "Setpoint";
  return plotted && isAuxiliary(plotted) ? plotted.id : undefined;
}

/** "Measurement (°C)", "Output (%)": the role with its unit in parentheses, or the bare role for a dimensionless signal. */
export function signalRoleLabel(sides: LiveSides | null | undefined, symbol: string, unit: string): string {
  const role = signalRoleName(sides, symbol) ?? symbol;
  const display = unit.trim() === "" || unit.trim() === "1" ? "" : displayEngineeringUnit(unit);
  return display ? `${role} (${display})` : role;
}

/** The loop's signal names for a panel title or a hover text, such as "Measurement: P1.RoomC · Output: Heater". Only a signal with a role is named. */
export function signalRoleTitle(sides: LiveSides | null | undefined): string {
  if (!sides) return "";
  const symbols = [...sides.outputs, ...sides.inputs, ...(sides.plotSignals ?? [])].map(signal => signal.symbol);
  return [...new Set(symbols)].flatMap(symbol => {
    const role = signalRoleName(sides, symbol);
    return role ? [`${role}: ${symbol}`] : [];
  }).join(" · ");
}

function numbered(role: string, channels: readonly string[], symbol: string): string {
  return channels.length > 1 ? `${role} ${channels.indexOf(symbol) + 1}` : role;
}
