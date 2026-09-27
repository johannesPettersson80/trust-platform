import type { ControlLoopDefinition } from "../loopDefinition";
import type { WorkflowResultSample } from "../workflowResult";

/** One binding catalogue for live and retained experiment readings. */
export function experimentSignalBindings(loop: ControlLoopDefinition) {
  return [
    ...(loop.signals.setpoint ? [{ ...loop.signals.setpoint, id: "sp" }] : []),
    { ...loop.signals.processVariable, id: "pv" },
    { ...(loop.signals.actuator ?? loop.signals.manipulatedVariable), id: "mv" },
    ...(loop.signals.actuator ? [{ ...loop.signals.manipulatedVariable, id: "requestedMv" }] : []),
    ...(loop.signals.additional ?? []),
  ];
}

/** Preserve signal identity and units; invalid samples are gaps, never zero. */
export function recordedSignalResponses(loop: ControlLoopDefinition, samples: readonly WorkflowResultSample[], selected: readonly string[]) {
  const signals = experimentSignalBindings(loop).filter(signal => selected.includes(signal.id));
  const groups = new Map<string, { name: string; unit: string; series: { name: string; signalId: string; dashed: boolean; points: (number | null)[][] }[] }>();
  for (const signal of signals) {
    const points = samples.map(sample => {
      const additional = sample.additional?.find(value => value.id === signal.id);
      const value = signal.id === "sp" ? sample.sp : signal.id === "pv" ? sample.pv : signal.id === "mv" ? sample.appliedMv : signal.id === "requestedMv" ? sample.requestedMv : additional?.value;
      const quality = additional?.quality ?? sample.quality;
      return [sample.elapsedSeconds, quality === "good" && typeof value === "number" && Number.isFinite(value) ? value : null];
    });
    const group = groups.get(signal.unit) ?? { name: "", unit: signal.unit, series: [] };
    group.series.push({ name: signal.symbol, signalId: signal.id, dashed: signal.id === "sp", points });
    group.name = group.series.length === 1 ? signal.symbol : "Response";
    groups.set(signal.unit, group);
  }
  return [...groups.values()];
}
