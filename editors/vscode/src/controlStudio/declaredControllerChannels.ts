import type { ControlLoopDefinition } from "./loopDefinition";
import { loopSignal } from "./loopChannels";
import type { TransferMember } from "./proportionalTransfer";

export function transferSignalMember(signal: ControlLoopDefinition["signals"]["processVariable"], actuator = false): TransferMember {
  if (actuator && signal.iecType === "BOOL" && signal.unit === "1" && signal.range.minimum === 0 && signal.range.maximum === 1) return { symbol: signal.symbol, iec_type: "BOOL", unit: "1", minimum: 0, maximum: 1 };
  if (signal.iecType !== "REAL" && signal.iecType !== "LREAL") throw new Error("Controller reference and output transfer bindings require REAL/LREAL coordinates.");
  return { symbol: signal.symbol, iec_type: signal.iecType, unit: signal.unit, minimum: signal.range.minimum, maximum: signal.range.maximum };
}

/** Reference and actuator roles are independent; a nonsquare plant is not padded with fictitious channels. */
export function declaredControllerChannels(loop: ControlLoopDefinition) {
  const references = loop.channels?.outputs.filter(output => output.setpointSignalId).map(output => ({
    reference: transferSignalMember(loopSignal(loop, output.setpointSignalId!)),
    process: transferSignalMember(loopSignal(loop, output.signalId)),
  })) ?? (loop.signals.setpoint ? [{ reference: transferSignalMember(loop.signals.setpoint), process: transferSignalMember(loop.signals.processVariable) }] : []);
  const outputs = loop.channels?.inputs.map(input => {
    const signal = loopSignal(loop, input.signalId), primary = signal.symbol === loop.signals.manipulatedVariable.symbol;
    const command = primary ? loop.controller.manualOutputSignal : input.manualOutputSignal;
    const type = primary ? loop.controller.manualOutputType : input.manualOutputType;
    if (!command || type !== "REAL" && type !== "LREAL" && type !== "BOOL") throw new Error(`Declare the compiled Manual command and IEC type for ${signal.symbol}.`);
    const output = { ...transferSignalMember(signal, true), minimum: input.actuator.minimum, maximum: input.actuator.maximum };
    return { output, command: { ...output, symbol: command, iec_type: type } };
  }) ?? (() => {
    if (!loop.controller.manualOutputSignal || !loop.controller.manualOutputType) throw new Error("Declare the compiled Manual command and type.");
    const output = { ...transferSignalMember(loop.signals.manipulatedVariable, true), minimum: loop.limits.outputMinimum, maximum: loop.limits.outputMaximum };
    return [{ output, command: { ...output, symbol: loop.controller.manualOutputSignal, iec_type: loop.controller.manualOutputType } }];
  })();
  const primaryReference = references.find(reference => reference.reference.symbol === loop.signals.setpoint?.symbol);
  const primaryOutput = outputs.find(output => output.output.symbol === loop.signals.manipulatedVariable.symbol);
  if (!primaryReference || !primaryOutput) throw new Error("The loop's primary reference and output must belong to the declared model channel bindings.");
  const measurements = loop.channels?.outputs.map(output => transferSignalMember(loopSignal(loop, output.signalId)))
    ?? [transferSignalMember(loop.signals.processVariable)];
  return { measurements, references: [primaryReference, ...references.filter(reference => reference !== primaryReference)],
    outputs: [primaryOutput, ...outputs.filter(output => output !== primaryOutput)] };
}
