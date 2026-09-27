import type { ControlLoopDefinition } from "./loopDefinition";
import { controllerModeAddress } from "./controllerModeBinding";
import type { SymbolicInspectionBinding } from "./symbolicInspection";

/** The native runtime owns one coherent watch; readers of a loop share its exact selection. */
export function loopInspectionBindings(loop: ControlLoopDefinition): SymbolicInspectionBinding[] {
  const bindings: SymbolicInspectionBinding[] = [
    ...(loop.signals.setpoint ? [{ id: "setpoint", ...loop.signals.setpoint }] : []),
    { id: "processVariable", ...loop.signals.processVariable },
    { id: "manipulatedVariable", ...loop.signals.manipulatedVariable },
    ...(loop.signals.actuator ? [{ id: "actuator", ...loop.signals.actuator }] : []),
    ...(loop.signals.additional ?? []),
  ].map(binding => ({ id: binding.id, address: binding.address, iec_type: binding.iecType }));
  for (const [name, symbol] of [["controllerMode", loop.controller.modeSignal],
    ["acquisitionMode", loop.parameterBinding?.mode.symbol]] as const) {
    if (!symbol || loopInspectionModeId(loop, bindings, symbol)) continue;
    const address = controllerModeAddress(loop, symbol);
    let id: string = name;
    while (bindings.some(binding => binding.id === id)) id += "_";
    bindings.push({ id, ...(address ? { address } : { symbol }), iec_type: "BOOL" });
  }
  return bindings;
}

/** Mode aliases share their declared addressed BOOL channel when one exists. */
export function loopInspectionModeId(loop: ControlLoopDefinition, bindings: readonly SymbolicInspectionBinding[], symbol: string): string | undefined {
  const address = controllerModeAddress(loop, symbol);
  return bindings.find(binding => binding.iec_type === "BOOL" && (address
    ? binding.address === address : binding.symbol?.toLowerCase() === symbol.toLowerCase()))?.id;
}
