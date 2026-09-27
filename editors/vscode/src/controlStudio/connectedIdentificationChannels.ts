import { loopSignal } from "./loopChannels";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopMonitorSample } from "./loopMonitor";
import type { RuntimeRequest } from "./verificationClient";

/** Excite each bound input in its own interval; all outputs share the same native recording. */
export async function connectedIdentificationChannels(loop: ControlLoopDefinition, request: RuntimeRequest) {
  if (!loop.channels || loop.channels.inputs.length === 1) return [{ loop, sample: (sample: LoopMonitorSample) => sample }];
  const raw = await request("io.list", {}) as { inputs?: { name?: string; address: string }[]; outputs?: { name?: string; address: string }[]; memory?: { name?: string; address: string }[] };
  const entries = [...raw.inputs ?? [], ...raw.outputs ?? [], ...raw.memory ?? []];
  return loop.channels.inputs.map(input => {
    const signal = loopSignal(loop, input.signalId);
    if (signal.symbol === loop.signals.manipulatedVariable.symbol) return { loop, sample: (sample: LoopMonitorSample) => sample };
    const command = input.manualOutputSignal;
    const address = entries.find(entry => entry.name?.toLowerCase() === command?.toLowerCase())?.address;
    if (!command || !address || !input.manualOutputType) throw new Error(`The connected runtime has no addressed Manual command for ${signal.symbol}.`);
    return { loop: { ...loop, signals: { ...loop.signals, manipulatedVariable: signal },
      controller: { ...loop.controller, manualOutputSignal: command, manualOutputAddress: address, manualOutputType: input.manualOutputType },
      limits: { outputMinimum: input.actuator.minimum, outputMaximum: input.actuator.maximum } },
      sample: (sample: LoopMonitorSample): LoopMonitorSample => {
        if (sample.availability !== "available") return sample;
        const measured = sample.additional?.find(channel => channel.symbol === signal.symbol);
        if (!measured) throw new Error(`No current measured input for ${signal.symbol}.`);
        return { ...sample, manipulatedVariable: measured };
      } };
  });
}
