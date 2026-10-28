import type { PlotSignal, SideSignal } from "../generated/OpenResult";
import type { LoopMonitorPayload } from "../messages";
import { openedOffers } from "./openedOffers";
import { displayEngineeringUnit, formatEngineeringNumber } from "../chartContract";
import { signalRoleName } from "./signalRoles";

/** The kind of a plotted signal, as the engine names it (`PlotSignal.kind`). */
type PlotSignalKind = PlotSignal["kind"];

/** The engine's live sample (B2, B9.2): `controlStudio/monitor`'s `sample`, which the panel posts as the engine answers it. */
export type LiveSample = LoopMonitorPayload;
type AvailableLiveSample = Extract<LiveSample, { availability: "available" }>;
type LiveSignal = NonNullable<AvailableLiveSample["processVariable"]>;

/** A signal of the loop as the live plot reads it: its plot identifier and kind when the engine's list has them, its symbol, its unit and the width of its range. */
interface LiveBinding { id?: string; kind?: PlotSignalKind; symbol: string; unit: string; span: number }

export interface SignalResponseGroup {
  name: string;
  unit: string;
  zeroSpan?: number;
  /** Where the recorded span begins, in s. A run starts at zero; its first averaged sample lies inside the span (A4.2). */
  timeStart?: number;
  series: { prediction?: boolean; signalId?: string; name: string; label?: string; points: (number | null)[][]; dashed?: boolean }[];
}

/** Bounded display projection of the existing monitor stream; never acquires or writes. */
export class ThreePageLiveSignals {
  private samples: AvailableLiveSample[] = [];
  private latest: LiveSample | undefined;
  private firstTime: number | undefined;

  receive(sample: LiveSample): void {
    if (sample.availability === "available") {
      const last = this.samples[this.samples.length - 1];
      if (last && last.runtimeSessionId !== sample.runtimeSessionId) {
        this.samples = []; this.firstTime = undefined;
      } else if (last && (sample.scan < last.scan || sample.observedAtMs < last.observedAtMs)) return;
      this.firstTime ??= sample.observedAtMs;
      if (!this.samples.length || this.samples[this.samples.length - 1].scan !== sample.scan) {
        this.samples.push(sample);
        // Keep exact points in a bounded recent view rather than interpolating gaps.
        if (this.samples.length > 600) this.samples.shift();
      }
    }
    this.latest = sample;
  }

  /** What an empty plot says: live values need the runtime; otherwise the first samples are on their way. */
  emptyMessage(): string {
    return this.latest?.availability === "available" ? "Waiting for the first samples" : "Start the runtime to see live values";
  }

  /**
   * The plot's groups. The sides and the signals the plots draw come from the engine's open answer (`liveSides`, B9.6); the value drawn at each
   * sample is the one the engine marks as plotted (`plotted`), and the page keeps no rule of its own for it.
   */
  responses(inputs: readonly string[], outputs: readonly string[], experimentSelection?: readonly string[]): SignalResponseGroup[] {
    const sides = openedOffers().liveSides;
    const catalogue: readonly PlotSignal[] = sides?.plotSignals ?? [];
    const bound = (side: SideSignal): LiveBinding => {
      const plotted = catalogue.find(signal => signal.symbol === side.symbol);
      return { id: plotted?.id, kind: plotted?.kind, symbol: side.symbol, unit: side.unit, span: plotted?.span ?? 0 };
    };
    const signals: { signal: LiveBinding; name: string }[] = experimentSelection ? catalogue.filter(signal => experimentSelection.includes(signal.id))
      .map(signal => ({ signal, name: signal.symbol })) : [
      ...(sides?.outputs ?? []).map(bound).filter(signal => outputs.includes(signal.symbol)).map(signal => ({ signal, name: "Measurement" })),
      ...(sides?.inputs ?? []).map(bound).filter(signal => inputs.includes(signal.symbol)).map(signal => ({ signal, name: "Output" })),
    ];
    const groups = new Map<string, SignalResponseGroup>();
    for (const { signal, name } of signals) {
      const current = this.latest?.availability === "available" ? this.read(this.latest, signal) : undefined;
      const unit = displayEngineeringUnit(signal.unit);
      const role = signalRoleName(sides, signal.symbol) ?? name;
      const value = current?.plotted ? `${formatEngineeringNumber(current.value)} ${unit}` : `${current?.quality ?? "unavailable"} (${unit})`;
      const group = groups.get(signal.unit) ?? { name: role, unit: signal.unit, series: [] };
      group.zeroSpan = Math.max(group.zeroSpan ?? 0, signal.span * 0.01);
      group.series.push({ signalId: signal.id, dashed: signal.kind === "setpoint", name: signal.symbol, label: `${role}: ${value}`, points: this.samples.map(sample => {
        const reading = this.read(sample, signal);
        return [(sample.observedAtMs - this.firstTime!) / 1000, reading?.plotted ? reading.value : null];
      }) });
      if (group.name !== role) group.name = "Response";
      groups.set(signal.unit, group);
    }
    return [...groups.values()];
  }

  private read(sample: AvailableLiveSample, signal: LiveBinding): LiveSignal | null | undefined {
    return [sample.setpoint, sample.processVariable, sample.manipulatedVariable, ...(sample.additional ?? [])]
      .find(reading => reading?.symbol === signal.symbol && reading.unit === signal.unit);
  }
}
