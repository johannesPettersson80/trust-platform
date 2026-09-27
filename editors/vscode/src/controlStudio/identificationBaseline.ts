import type { ControlLoopDefinition } from "./loopDefinition";
import type { RecordedRun } from "./runValidation";
import { inspectSymbolicSignals, isUsableSymbolicInspectionValue, type SymbolicInspectionRequest } from "./symbolicInspection";
import { loopInspectionBindings, loopInspectionModeId } from "./loopInspectionBindings";

export interface IdentificationBaseline { input: number; output: number }

/** Observe the existing process; never infer model coefficients or hidden state. */
export async function readIdentificationBaseline(
  loop: ControlLoopDefinition, request: SymbolicInspectionRequest, isCancelled: () => boolean = () => false,
): Promise<IdentificationBaseline> {
  const mode = loop.parameterBinding?.mode;
  if (!mode) throw new Error("Confirm Manual mode before identification.");
  const bindings = loopInspectionBindings(loop);
  const modeId = loopInspectionModeId(loop, bindings, mode.symbol)!;
  const identity = { loopRevision: loop.revision, expectedProgramRevision: undefined as string | undefined };
  if (isCancelled()) throw new Error("Identification cancelled.");
  let result = await inspectSymbolicSignals(request, identity, bindings);
  for (let attempt = 0; result.kind === "pending" && attempt < 20; attempt++) {
    if (isCancelled()) throw new Error("Identification cancelled.");
    identity.expectedProgramRevision = result.programRevision;
    await new Promise(resolve => setTimeout(resolve, Math.max(50, loop.controller.taskPeriodMs)));
    if (isCancelled()) throw new Error("Identification cancelled.");
    result = await inspectSymbolicSignals(request, identity, bindings);
  }
  if (isCancelled()) throw new Error("Identification cancelled.");
  if (result.kind !== "available") throw new Error("Operating point unavailable; wait for a current simulator scan and retry.");
  const { sample } = result;
  const value = (id: string) => sample.values[sample.bindings.findIndex(binding => binding.id === id)];
  const age = Date.now() - sample.capturedAtMs;
  if (age < -1000 || age > Math.max(1000, 3 * loop.controller.taskPeriodMs)
      || !["manipulatedVariable", "processVariable", modeId].every(id => isUsableSymbolicInspectionValue(sample, id))
      || ![0, 1].includes(value(modeId)!)) {
    throw new Error("Identification needs fresh, good, unforced Manual readings. The isolated acquisition sets its own mode without changing the observed runtime.");
  }
  return { input: value("manipulatedVariable")!, output: value("processVariable")! };
}

/** Narrow deterministic-simulator check; this does not certify unobserved states. */
export function qualifyIdentificationBaseline(
  run: RecordedRun, loop: ControlLoopDefinition, point: IdentificationBaseline, onsetSeconds: number,
): void {
  const baseline = run.samples.filter(sample => sample.timeSeconds < onsetSeconds);
  const channels = [[loop.signals.manipulatedVariable.symbol, point.input],
    [loop.signals.processVariable.symbol, point.output]] as const;
  if (baseline.length < 3 || baseline.some(sample => channels.some(([id, expected]) => {
    const value = sample.values[id];
    return !value || value.quality !== "good" || value.value === null || !Number.isFinite(value.value)
      || Math.abs(value.value - expected) > 1e-6 * Math.max(1, Math.abs(expected));
  }))) {
    throw new Error("Identification baseline is incompatible or nonsteady. Review the retained trial and establish compatible plant initial conditions before retrying.");
  }
}
