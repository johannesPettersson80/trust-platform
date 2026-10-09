import type { ControlLoopDefinition, ControlSignalBinding } from "../loopDefinition";
import type { AvailableLoopSample, ControlMonitorSignal, LoopMonitorSample } from "../loopMonitor";
import { experimentSignalBindings } from "./threePageRecordedSignals";
import { loopSignal } from "../loopChannels";
import { displayEngineeringUnit, formatEngineeringNumber } from "../chartContract";
import { signalRoleName } from "./signalRoles";

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
  private samples: AvailableLoopSample[] = [];
  private latest: LoopMonitorSample | undefined;
  private firstTime: number | undefined;

  constructor(private readonly loop: ControlLoopDefinition) {}

  receive(sample: LoopMonitorSample): void {
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

  responses(inputs: readonly string[], outputs: readonly string[], experimentSelection?: readonly string[]): SignalResponseGroup[] {
    const channels = (kind: "inputs" | "outputs") => this.loop.channels?.[kind].map(channel => loopSignal(this.loop, channel.signalId))
      ?? [kind === "inputs" ? this.loop.signals.manipulatedVariable : this.loop.signals.processVariable];
    const catalogue = experimentSignalBindings(this.loop);
    const signals = experimentSelection ? catalogue.filter(signal => experimentSelection.includes(signal.id))
      .map(signal => ({ signal, name: signal.symbol })) : [
      ...channels("outputs").filter(signal => outputs.includes(signal.symbol)).map(signal => ({ signal, name: "Measurement" })),
      ...channels("inputs").filter(signal => inputs.includes(signal.symbol)).map(signal => ({ signal, name: "Output" })),
    ];
    const groups = new Map<string, SignalResponseGroup>();
    for (const { signal, name } of signals) {
      const current = this.latest?.availability === "available" ? this.read(this.latest, signal) : undefined;
      const unit = displayEngineeringUnit(signal.unit);
      const role = signalRoleName(this.loop, signal.symbol) ?? name;
      const value = current?.quality === "good" && Number.isFinite(current.value)
        ? `${formatEngineeringNumber(current.value)} ${unit}` : `${current?.quality ?? "unavailable"} (${unit})`;
      const group = groups.get(signal.unit) ?? { name: role, unit: signal.unit, series: [] };
      group.zeroSpan = Math.max(group.zeroSpan ?? 0, (signal.range.maximum - signal.range.minimum) * 0.01);
      const signalId = catalogue.find(item => item.symbol === signal.symbol)?.id;
      group.series.push({ signalId, dashed: signalId === "sp", name: signal.symbol, label: `${role}: ${value}`, points: this.samples.map(sample => {
        const reading = this.read(sample, signal);
        return [(sample.observedAtMs - this.firstTime!) / 1000,
          reading?.quality === "good" && Number.isFinite(reading.value) ? reading.value : null];
      }) });
      if (group.name !== role) group.name = "Response";
      groups.set(signal.unit, group);
    }
    return [...groups.values()];
  }

  private read(sample: AvailableLoopSample, signal: ControlSignalBinding): ControlMonitorSignal | undefined {
    return [sample.setpoint, sample.processVariable, sample.manipulatedVariable, ...(sample.additional ?? [])]
      .find(reading => reading?.symbol === signal.symbol && reading.unit === signal.unit);
  }
}
