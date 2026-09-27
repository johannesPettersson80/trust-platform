import type { AvailableLoopSample } from "../loopMonitor";
import type { WorkflowResultSample } from "../workflowResult";
import { readableSignalName } from "../signalNaming";

export type RecordedSignalSample = Pick<WorkflowResultSample,
  "sp" | "pv" | "appliedMv" | "units" | "quality" | "additional">;

export interface SignalRow {
  id: string;
  name: string;
  role: string;
  channel: string;
  symbol: string;
  value: number | null;
  unit: string;
  quality: string;
}

/** Binding metadata may be reused; the selected evidence owns every reading. */
export function controlSignalRows(
  live: AvailableLoopSample | undefined,
  historical?: { sample?: RecordedSignalSample },
  topology: "closed-loop" | "open-loop" = "closed-loop",
): SignalRow[] {
  const openLoop = topology === "open-loop";
  const liveRows: SignalRow[] = live ? [
    ...(!openLoop && live.setpoint ? [{ id: "sp", name: "Setpoint", role: "SP", channel: "Primary loop", ...live.setpoint }] : []),
    { id: "pv", name: openLoop ? "Measured output" : "Process value", role: "PV", channel: "Primary loop", ...live.processVariable },
    { id: "mv", name: openLoop ? "Applied input" : readableSignalName(live.manipulatedVariable.symbol), role: "MV",
      channel: "Primary loop", ...live.manipulatedVariable },
    ...(live.additional ?? []).map(signal => ({ name: signal.displayName, ...signal })),
  ] : [];
  if (!historical) return liveRows;
  const sample = historical.sample;
  if (!sample) return [];
  const primary = [
    ...(openLoop || sample.sp === null ? [] : [{ id: "sp", name: "Setpoint", value: sample.sp, unit: sample.units.sp }]),
    { id: "pv", name: openLoop ? "Measured output" : "Process value", value: sample.pv, unit: sample.units.pv },
    { id: "mv", name: openLoop ? "Applied input" : "Applied output", value: sample.appliedMv, unit: sample.units.mv },
  ].map(signal => {
    const binding = liveRows.find(row => row.id === signal.id && row.unit === signal.unit);
    return { ...signal, name: binding?.name ?? signal.name,
      symbol: binding?.symbol ?? `Recorded ${signal.id.toUpperCase()}`,
      role: signal.id.toUpperCase(), channel: "Primary loop", quality: sample.quality };
  });
  return [...primary, ...(sample.additional ?? []).map(signal => {
    const binding = liveRows.find(row => row.id === signal.id && row.unit === signal.unit &&
      row.role === signal.role && row.channel === signal.channel);
    return { ...signal, name: binding?.name ?? readableSignalName(signal.id),
      symbol: binding?.symbol ?? `Recorded channel ${signal.id}` };
  })];
}
