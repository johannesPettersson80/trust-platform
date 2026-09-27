import type { RuntimeLifecycleSnapshot } from "../runtimeLifecycle";
import type { ControlLoopDefinition, ControlSignalBinding } from "./loopDefinition";
import { controllerModeAddress } from "./controllerModeBinding";
import { loopInspectionBindings } from "./loopInspectionBindings";
import { bindRuntimeConnection } from "./runtimeConnection";
import { controlLoopStaleAfterMs, humanizeSignalIdentifier, type ControlLoopMonitor, type ControlMonitorSignal, type LoopMonitorSample } from "./loopMonitor";
import { inspectSymbolicSignals, isUsableSymbolicInspectionValue } from "./symbolicInspection";

/** One bounded inspection watch supplies every displayed quantity from the same actual scan. */
export class SymbolicLoopObservation {
  private programRevision: string | undefined;
  private connection: Awaited<ReturnType<typeof bindRuntimeConnection>> | undefined;
  private pending: Promise<LoopMonitorSample> | undefined;
  private sessionId: string | undefined;
  private generation = 0;

  isCurrentSession(sessionId: string | undefined): boolean { return this.sessionId === sessionId; }

  private reset(monitor: ControlLoopMonitor): void {
    this.generation++;
    this.connection = undefined; this.programRevision = undefined; this.pending = undefined;
    monitor.resetRuntimeObservation();
  }

  observe(monitor: ControlLoopMonitor, loop: ControlLoopDefinition,
    lifecycle: RuntimeLifecycleSnapshot, now: number): Promise<LoopMonitorSample> {
    if (lifecycle.sessionId !== this.sessionId) {
      this.sessionId = lifecycle.sessionId;
      this.reset(monitor);
    }
    if (!loop.controller.modeSignal || controllerModeAddress(loop) || !lifecycle.status.running) {
      if (!lifecycle.status.running) this.reset(monitor);
      return Promise.resolve(monitor.observe(loop, lifecycle, now));
    }
    if (!this.pending) {
      const pending = this.inspect(monitor, loop, lifecycle, now, this.generation).finally(() => {
        if (this.pending === pending) this.pending = undefined;
      });
      this.pending = pending;
    }
    return this.pending;
  }

  private async inspect(monitor: ControlLoopMonitor, loop: ControlLoopDefinition,
    lifecycle: RuntimeLifecycleSnapshot, now: number, generation: number): Promise<LoopMonitorSample> {
    const source = lifecycle.status.runtimeMode === "simulate" ? "Simulation" as const : "Physical" as const;
    const unavailable = (reason: string): LoopMonitorSample => ({ availability: "unavailable", source, loopId: loop.id, reason });
    try {
      if (this.connection && this.connection.endpoint !== lifecycle.status.endpoint) {
        this.connection = undefined; this.programRevision = undefined;
      }
      const connection = this.connection ?? await bindRuntimeConnection(loop.runtimeId);
      if (generation !== this.generation) return unavailable("The runtime session changed during inspection.");
      this.connection = connection;
      const bindings = loopInspectionBindings(loop);
      const modeId = bindings.find(binding => binding.symbol === loop.controller.modeSignal)?.id;
      let result = await inspectSymbolicSignals(connection.request,
        { loopRevision: loop.revision, expectedProgramRevision: this.programRevision }, bindings);
      if (generation !== this.generation) return unavailable("The runtime session changed during inspection.");
      const deadline = Date.now() + Math.min(5000, controlLoopStaleAfterMs(loop));
      while (result.kind === "pending" && Date.now() < deadline) {
        this.programRevision = result.programRevision;
        await new Promise(resolve => setTimeout(resolve, Math.min(200, Math.max(25, loop.observedSampleTiming.nominalPeriodMs))));
        if (generation !== this.generation) return unavailable("The runtime session changed during inspection.");
        result = await inspectSymbolicSignals(connection.request,
          { loopRevision: loop.revision, expectedProgramRevision: this.programRevision }, bindings);
        if (generation !== this.generation) return unavailable("The runtime session changed during inspection.");
      }
      if (result.kind === "pending") { this.programRevision = result.programRevision; return unavailable(result.reason); }
      const sample = result.sample;
      this.programRevision = sample.programRevision;
      const age = Date.now() - sample.capturedAtMs;
      if (age > controlLoopStaleAfterMs(loop) || age < -1000) return unavailable("The symbolic runtime scan is stale or its capture clock is incompatible.");
      const read = (id: string, binding: ControlSignalBinding): ControlMonitorSignal | undefined => {
        const index = sample.bindings.findIndex(item => item.id === id);
        if (index < 0 || typeof sample.values[index] !== "number" ||
            !["good", "forced"].includes(sample.quality[index])) return undefined;
        return { symbol: binding.symbol, address: binding.address, unit: binding.unit, value: sample.values[index]!,
          quality: sample.forced[index] || sample.quality[index] === "forced" ? "forced" : "good" };
      };
      const whenTrue = loop.controller.modeWhenTrue ?? "Auto";
      const mode = modeId && isUsableSymbolicInspectionValue(sample, modeId)
        ? sample.values[sample.bindings.findIndex(binding => binding.id === modeId)] === 1 ? whenTrue : whenTrue === "Manual" ? "Auto" : "Manual"
        : "Unavailable";
      return monitor.observeReadings(loop, source, { running: true, scan: sample.scan, controllerMode: mode,
        setpoint: loop.signals.setpoint ? read("setpoint", loop.signals.setpoint) : undefined,
        processVariable: read("processVariable", loop.signals.processVariable), manipulatedVariable: read("manipulatedVariable", loop.signals.manipulatedVariable),
        additional: (loop.signals.additional ?? []).map(binding => {
          const signal = read(binding.id, binding);
          return signal ? { ...signal, id: binding.id, channel: binding.channel, role: binding.role, displayName: humanizeSignalIdentifier(binding.id) } : undefined;
        }),
      }, now);
    } catch (error) { return unavailable(error instanceof Error ? error.message : String(error)); }
  }
}
