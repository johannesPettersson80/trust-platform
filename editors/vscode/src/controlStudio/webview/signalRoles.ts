import type { ControlLoopDefinition } from "../loopDefinition";
import { displayEngineeringUnit } from "../chartContract";
import { loopSignal } from "../loopChannels";

/**
 * Names a signal by its role in the loop (Specification 35 B9.9): Measurement, Output, Setpoint, or an auxiliary
 * channel's id. Several channels of one kind are numbered, so every name stays distinct. The full symbol is the hover text.
 */
export function signalRoleName(loop: ControlLoopDefinition | null | undefined, symbol: string): string | undefined {
  if (!loop) return undefined;
  const measured = loop.channels?.outputs.map(channel => loopSignal(loop, channel.signalId).symbol) ?? [loop.signals.processVariable.symbol];
  const actuated = loop.channels?.inputs.map(channel => loopSignal(loop, channel.signalId).symbol) ?? [loop.signals.manipulatedVariable.symbol];
  if (measured.includes(symbol)) return numbered("Measurement", measured, symbol);
  if (actuated.includes(symbol)) return numbered("Output", actuated, symbol);
  if (loop.signals.setpoint && symbol === loop.signals.setpoint.symbol) return "Setpoint";
  const auxiliary = loop.signals.additional?.find(signal => signal.symbol === symbol);
  return auxiliary ? auxiliary.id : undefined;
}

/** "Measurement (°C)", "Output (%)": the role with its unit in parentheses, or the bare role for a dimensionless signal. */
export function signalRoleLabel(loop: ControlLoopDefinition | null | undefined, symbol: string, unit: string): string {
  const role = signalRoleName(loop, symbol) ?? symbol;
  const display = unit.trim() === "" || unit.trim() === "1" ? "" : displayEngineeringUnit(unit);
  return display ? `${role} (${display})` : role;
}

/** The loop's signal names for a panel title or a hover text, such as "Measurement: P1.RoomC · Output: Heater". */
export function signalRoleTitle(loop: ControlLoopDefinition | null | undefined): string {
  if (!loop) return "";
  const symbols = [loop.signals.processVariable.symbol, loop.signals.manipulatedVariable.symbol,
    ...(loop.signals.setpoint ? [loop.signals.setpoint.symbol] : []), ...(loop.signals.additional ?? []).map(signal => signal.symbol)];
  return [...new Set(symbols)].map(symbol => `${signalRoleName(loop, symbol) ?? symbol}: ${symbol}`).join(" · ");
}

function numbered(role: string, channels: readonly string[], symbol: string): string {
  return channels.length > 1 ? `${role} ${channels.indexOf(symbol) + 1}` : role;
}
