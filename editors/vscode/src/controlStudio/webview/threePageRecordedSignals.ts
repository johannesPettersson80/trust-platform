import type { PlotSignal, RecordedChartSample } from "../generated/OpenResult";

/**
 * What the chart reads of a kept recording's sample: its process time and the value each plotted signal takes, by the signal's
 * identifier (`RecordedChartSample.plotted`, B9.6). The engine names each value by the identifier of `liveSides.plotSignals` and marks the
 * gaps with null, so the page maps no signal onto a field of the sample and holds no rule of which value is drawn.
 */
export type ChartSample = Pick<RecordedChartSample, "elapsedSeconds" | "plotted">;

/**
 * The signal that names an identification experiment's recorded input trace: the engine's list gives the symbol each trace of a
 * signal carries (`recordedAs`, else the signal's own symbol), so the page looks the trace's symbol up and resolves nothing.
 */
export function experimentInputBinding(signals: readonly PlotSignal[], traceSymbol: string): PlotSignal | undefined {
  return signals.find(signal => (signal.recordedAs ?? signal.symbol) === traceSymbol);
}

/** One series for each selected signal of a kept recording, grouped by unit. A null value is a gap, never zero (B9.6, B10.3). */
export function recordedSignalResponses(signals: readonly PlotSignal[], samples: readonly ChartSample[], selected: readonly string[]) {
  const groups = new Map<string, { name: string; unit: string; series: { name: string; signalId: string; dashed: boolean; points: (number | null)[][] }[] }>();
  for (const signal of signals.filter(item => selected.includes(item.id))) {
    const points = samples.map(sample => [sample.elapsedSeconds, sample.plotted[signal.id] ?? null]);
    const group = groups.get(signal.unit) ?? { name: "", unit: signal.unit, series: [] };
    group.series.push({ name: signal.symbol, signalId: signal.id, dashed: signal.kind === "setpoint", points });
    group.name = group.series.length === 1 ? signal.symbol : "Response";
    groups.set(signal.unit, group);
  }
  return [...groups.values()];
}
