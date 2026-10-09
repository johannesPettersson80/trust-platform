import type { GeneralModel } from "./generalModel";
import { openLoopStepResponse, twoPointPath } from "./pathReduction";
import { oneToOnePairing } from "./mimoPairing";
import { relativeGainArray } from "./numerics/mimoRobustAdaptive";
import { tuneController, type TuningObjective } from "./tuning";

export interface DecentralizedPair { inputId: string; outputId: string }
export interface DecentralizedMember extends DecentralizedPair {
  /** Steady-state gain of the member's own path, Kᵢᵢ. */
  pathGain: number;
  /** Relative gain λᵢᵢ of the pairing. */
  relativeGain: number;
  /** Kᵢᵢ/λᵢᵢ: the gain the member sees when the other loops are closed. */
  effectiveGain: number;
  timeConstantSeconds: number;
  delaySeconds: number;
  proportionalGain: number;
  integralGain: number;
  derivativeGain: 0;
}
export interface DecentralizedPiDesign { gainMatrix: number[][]; relativeGains: number[][]; members: DecentralizedMember[] }

const tolerance = { absolute: 1e-12, relative: 1e-10, conditionLimit: 1e12 };

/** Specification 35 §17.9: one SIMC PI per member, on its own path, for the gain it sees with the other loops closed. */
export function designDecentralizedPi(model: GeneralModel, pairs: readonly DecentralizedPair[], objective: TuningObjective,
  action: "direct" | "reverse"): DecentralizedPiDesign {
  const inputs = pairs.map(pair => model.inputs.findIndex(input => input.id === pair.inputId));
  const outputs = pairs.map(pair => model.outputs.findIndex(output => output.id === pair.outputId));
  if (pairs.length < 2 || inputs.some(index => index < 0) || outputs.some(index => index < 0)
    || new Set(inputs).size !== pairs.length || new Set(outputs).size !== pairs.length) {
    throw new Error("Pair every group member's input and output channel once.");
  }
  const responses = inputs.map(input => openLoopStepResponse(model, input, "group member"));
  const gainMatrix = outputs.map(output => responses.map(response => response.final[output]));
  const rga = relativeGainArray(gainMatrix, tolerance) as { status: string; matrix?: number[][] };
  if (rga.status !== "qualified" || !rga.matrix) throw new Error("The members' steady-state gains are singular: their outputs cannot be set independently.");
  const relativeGains = rga.matrix;
  const conflicting = pairs.findIndex((_pair, index) => !(relativeGains[index][index] > 0));
  if (conflicting >= 0) {
    const pairing = oneToOnePairing(pairs.map(pair => pair.inputId), pairs.map(pair => pair.outputId), relativeGains);
    const better = pairing.every(decision => decision.status === "candidate")
      ? ` Pair ${[...pairing].sort((left, right) => left.inputId.localeCompare(right.inputId)).map(decision => `${decision.inputId} → ${decision.outputId}`).join(" and ")} instead.`
      : " No pairing of these channels has positive relative gains.";
    throw new Error(`The relative gain of ${pairs[conflicting].inputId} → ${pairs[conflicting].outputId} is ${significant(relativeGains[conflicting][conflicting])}, `
      + `so that loop would work against the others.${better}`);
  }
  const members = pairs.map((pair, index): DecentralizedMember => {
    const { timeConstantSeconds, delaySeconds } = twoPointPath(responses[index].times, responses[index].outputs[outputs[index]], "group member");
    const path = { timeConstantSeconds, delaySeconds };
    const effectiveGain = gainMatrix[index][index] / relativeGains[index][index];
    // Tune checks the signed effective gain against the action (N6.3) and returns the signed Kp (N6.4).
    const tuning = tuneController({ kind: path.delaySeconds > 0 ? "first_order_dead_time" : "first_order", gain: effectiveGain,
      timeConstantSeconds: path.timeConstantSeconds, deadTimeSeconds: path.delaySeconds, normalizedError: 0, stable: true }, { method: "SIMC", objective, action });
    const proportionalGain = tuning.controller.kp;
    return { ...pair, pathGain: gainMatrix[index][index], relativeGain: relativeGains[index][index], effectiveGain, ...path,
      proportionalGain, integralGain: proportionalGain / tuning.controller.tiSeconds!, derivativeGain: 0 };
  });
  return { gainMatrix, relativeGains, members };
}

function significant(value: number): string { return String(Number(value.toPrecision(2))); }
