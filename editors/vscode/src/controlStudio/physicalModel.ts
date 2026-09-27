import type { GeneralModel, GeneralStateSpaceModel, ModelChannel } from "./generalModel";

export type PhysicalBalance =
  | { kind: "gravity-tank" | "gravity-tank-cascade"; areaM2: number; outflowCoefficient: number }
  | { kind: "thermal-balance"; heatCapacityJPerK: number; lossWattsPerK: number; ambientTemperature: number };

/** One physical definition supplies both its local Jacobian and native execution parameters. */
export function physicalStateModel(identity: {
  revision: string; source: GeneralModel["source"]; validation: GeneralModel["validation"]; evidence: readonly string[];
  inputs: readonly ModelChannel[]; outputs: readonly ModelChannel[];
}, physical: PhysicalBalance, operatingOutputs: readonly number[]): GeneralStateSpaceModel {
  const n = physical.kind === "gravity-tank-cascade" ? 2 : 1;
  if (identity.inputs.length !== 1 || identity.outputs.length !== n || operatingOutputs.length !== n
    || !operatingOutputs.every(Number.isFinite)) throw new Error("Physical balance channels or operating point are incompatible.");
  let a: number[][], b: number[][], input: number;
  if (physical.kind === "thermal-balance") {
    if (!Number.isFinite(physical.heatCapacityJPerK) || physical.heatCapacityJPerK <= 0
      || !Number.isFinite(physical.lossWattsPerK) || physical.lossWattsPerK <= 0 || !Number.isFinite(physical.ambientTemperature)
      || identity.inputs[0].unit !== "W" || !["K", "degC", "°C"].includes(identity.outputs[0].unit)) throw new Error("Thermal balance needs positive heat capacity/loss and W with K or Celsius coordinates.");
    a = [[-physical.lossWattsPerK / physical.heatCapacityJPerK]];
    b = [[1 / physical.heatCapacityJPerK]];
    input = physical.lossWattsPerK * (operatingOutputs[0] - physical.ambientTemperature);
  } else {
    if (!Number.isFinite(physical.areaM2) || physical.areaM2 <= 0 || !Number.isFinite(physical.outflowCoefficient) || physical.outflowCoefficient < 0
      || !["m3/s", "m^3/s", "m³/s"].includes(identity.inputs[0].unit) || identity.outputs.some(output => output.unit !== "m")
      || operatingOutputs.some(output => output <= 0) || identity.outputs.some(output => output.operatingRange.minimum < 0)
      || n === 2 && Math.abs(operatingOutputs[0] - operatingOutputs[1]) > 1e-9) throw new Error("Gravity balances need positive area/level, nonnegative outflow and SI flow/level coordinates; equal cascaded tanks share an equilibrium level.");
    const poles = operatingOutputs.map(level => physical.outflowCoefficient / (2 * physical.areaM2 * Math.sqrt(level)));
    a = n === 1 ? [[-poles[0]]] : [[-poles[0], 0], [poles[0], -poles[1]]];
    b = n === 1 ? [[1 / physical.areaM2]] : [[1 / physical.areaM2], [0]];
    input = physical.outflowCoefficient * Math.sqrt(operatingOutputs[0]);
  }
  const inside = (value: number, channel: ModelChannel) => Number.isFinite(value) && value >= channel.operatingRange.minimum && value <= channel.operatingRange.maximum;
  if (!inside(input, identity.inputs[0]) || operatingOutputs.some((value, index) => !inside(value, identity.outputs[index]))) throw new Error("The physical equilibrium exceeds the declared equipment range.");
  return { ...identity, kind: "continuous-state-space", physical, delaySeconds: 0, a, b,
    c: Array.from({ length: n }, (_, row) => Array.from({ length: n }, (_, column) => Number(row === column))), d: Array.from({ length: n }, () => [0]),
    operatingPoint: { inputs: [input], outputs: [...operatingOutputs] }, states: identity.outputs.map((output, index) => ({
      id: `deviation:${output.id}`, unit: output.unit, operatingRange: { minimum: output.operatingRange.minimum - operatingOutputs[index], maximum: output.operatingRange.maximum - operatingOutputs[index] },
    })) };
}

export function validatePhysicalStateModel(model: GeneralModel): boolean {
  if (!model.physical) return true;
  if (model.kind !== "continuous-state-space" || model.delaySeconds !== 0 || !model.operatingPoint) return false;
  try {
    const expected = physicalStateModel(model, model.physical, model.operatingPoint.outputs);
    const close = (left: number, right: number) => Math.abs(left - right) <= 1e-9 * Math.max(1, Math.abs(left), Math.abs(right));
    return model.states.length === expected.states.length && model.states.every((state, index) => state.unit === expected.states[index].unit)
      && ["a", "b", "c", "d"].every(key => {
        const matrix = key as "a" | "b" | "c" | "d";
        return model[matrix].every((row, i) => row.every((value, j) => close(value, expected[matrix][i][j])));
      }) && close(model.operatingPoint.inputs[0], expected.operatingPoint!.inputs[0]);
  } catch { return false; }
}
