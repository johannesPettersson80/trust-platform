import type { ControlLoopDefinition } from "./loopDefinition";

/** Resolves declared symbolic mode names through the same compiled channel binding. */
export function controllerModeAddress(loop: ControlLoopDefinition, mode = loop.controller.modeSignal): string | undefined {
  if (!mode) return undefined;
  const auxiliary = loop.signals.additional?.find(binding => binding.iecType.toUpperCase() === "BOOL" &&
    (binding.symbol.toLowerCase() === mode.toLowerCase() || binding.address.toUpperCase() === mode.toUpperCase()));
  if (auxiliary) return auxiliary.address;
  return /^%[IQM]X\d+\.\d+$/i.test(mode) ? mode.toUpperCase() : undefined;
}
