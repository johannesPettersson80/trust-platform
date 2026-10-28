import type { PlotSignal } from "../generated/OpenResult";

/**
 * Whether the plots draw the signal beside the three fixed ones (the setpoint, the measurement and the output the process received): the
 * output the controller requested and every additional signal of the loop (B9.6). The engine names each signal's kind (`PlotSignal.kind`),
 * and this is the one place that groups the kinds, so that the naming of a signal (`signalRoles.ts`) and its colour
 * (`threePageResponseSeries.ts`) follow one classification and the page holds no list of identifiers.
 */
export function isAuxiliary(signal: Pick<PlotSignal, "kind">): boolean {
  return signal.kind === "requestedOutput" || signal.kind === "additional";
}
