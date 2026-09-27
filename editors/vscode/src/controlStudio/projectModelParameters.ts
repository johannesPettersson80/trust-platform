import { linearTransferModel } from "./linearTransferModel";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel, GeneralStateSpaceModel } from "./generalModel";
import type { PhysicalBalance } from "./physicalModel";
import { sampleModel } from "./stateControllerDesign";

export type ProjectModelParameter =
  | { kind: "sample-time" | "plant-delay" | "plant-gain" | "inverse-plant-gain" | "plant-time-constant" }
  | { kind: "physical"; parameter: "areaM2" | "outflowCoefficient" | "heatCapacityJPerK" | "lossWattsPerK" | "ambientTemperature" }
  | { kind: "sampled-matrix"; matrix: "a" | "b" | "c" | "d"; row: number; column: number };
export function projectModelParameters(loop: ControlLoopDefinition, model: GeneralModel,
  mapping: Readonly<Record<string, ProjectModelParameter>> = {}): Record<string, number> {
  const result: Record<string, number> = {};
  let sampled: ReturnType<typeof sampleModel> | undefined;
  for (const [symbol, source] of Object.entries(mapping)) {
    const parameter = loop.parameterBinding?.parameters.find(parameter => parameter.symbol === symbol);
    if (!parameter || !source || typeof source !== "object") throw new Error("A model-derived setting must name an actual bound ST parameter.");
    let value: number;
    if (source.kind === "sample-time" || source.kind === "plant-delay") {
      if (parameter.unit !== "s") throw new Error("Model timing fields require seconds.");
      value = source.kind === "sample-time" ? loop.controller.taskPeriodMs / 1000 : model.delaySeconds;
    } else if (source.kind === "physical") {
      if (!model.physical || !(source.parameter in model.physical)) throw new Error("The selected physical model does not supply this parameter.");
      assertPhysicalUnit(parameter.unit, source.parameter, model.physical.kind, model);
      value = Number((model.physical as unknown as Record<string, unknown>)[source.parameter]);
    } else if (source.kind === "sampled-matrix") {
      if (model.kind === "transfer-function" || model.delaySeconds !== 0 || !["a", "b", "c", "d"].includes(source.matrix)
        || !Number.isSafeInteger(source.row) || !Number.isSafeInteger(source.column) || source.row < 0 || source.column < 0) throw new Error("Declare compatible sampled model matrix coordinates.");
      sampled ??= sampleModel(model, loop.controller.taskPeriodMs / 1000);
      const matrix = sampled[source.matrix];
      if (!matrix[source.row] || source.column >= matrix[source.row].length) throw new Error("The sampled-matrix binding is outside the selected model dimensions.");
      assertMatrixUnit(parameter.unit, source.matrix, source.row, source.column, model);
      value = Number(matrix[source.row][source.column]);
    } else {
      const transfer = linearTransferModel(model);
      if (source.kind === "plant-time-constant") {
        if (transfer.denominator.length !== 2 || transfer.denominator[1] <= 0 || parameter.unit !== "s") throw new Error("The selected model does not supply a stable first-order time constant in seconds.");
        value = transfer.denominator[0] / transfer.denominator[1];
      } else if (source.kind === "plant-gain" || source.kind === "inverse-plant-gain") {
        const gain = transfer.numerator[transfer.numerator.length - 1] / transfer.denominator[transfer.denominator.length - 1];
        const expected = source.kind === "plant-gain"
          ? `${transfer.outputs[0].unit}/${transfer.inputs[0].unit}`
          : `${transfer.inputs[0].unit}/${transfer.outputs[0].unit}`;
        if (parameter.unit !== expected) throw new Error(`Model gain fields require ${expected} units.`);
        value = source.kind === "plant-gain" ? gain : 1 / gain;
      } else throw new Error("Unknown model-derived controller parameter.");
    }
    const rounded = parameter.iec_type === "REAL" ? Math.fround(value) : value;
    if (!Number.isFinite(rounded) || rounded < parameter.minimum || rounded > parameter.maximum) throw new Error(`Model-derived parameter ${symbol} exceeds its representable bound.`);
    result[symbol] = rounded;
  }
  return result;
}

function assertPhysicalUnit(unit: string, parameter: Extract<ProjectModelParameter, { kind: "physical" }>['parameter'],
  kind: PhysicalBalance["kind"], model: GeneralModel): void {
  const outputUnit = model.outputs[0]?.unit;
  const accepted = parameter === "areaM2" ? ["m2", "m^2", "m²"]
    : parameter === "outflowCoefficient" ? ["m3/s/sqrt(m)", "m^3/s/sqrt(m)", "m³/s/√m"]
    : parameter === "heatCapacityJPerK" ? ["J/K", "J/degC", "J/°C"]
    : parameter === "lossWattsPerK" ? ["W/K", "W/degC", "W/°C"]
    : outputUnit ? [outputUnit] : [];
  if (kind === "thermal-balance" && ["areaM2", "outflowCoefficient"].includes(parameter)
    || kind !== "thermal-balance" && ["heatCapacityJPerK", "lossWattsPerK", "ambientTemperature"].includes(parameter)) {
    throw new Error(`The ${parameter} model field is incompatible with the selected physical balance.`);
  }
  if (!accepted.includes(unit)) throw new Error(`Physical model field ${parameter} requires a compatible engineering unit.`);
}

function assertMatrixUnit(unit: string, matrix: "a" | "b" | "c" | "d", row: number, column: number,
  model: GeneralStateSpaceModel): void {
  const state = model.states[row]?.unit;
  const columnState = model.states[column]?.unit;
  const input = model.inputs[column]?.unit;
  const output = model.outputs[row]?.unit;
  const expected = matrix === "a" && state && columnState ? ratioUnit(state, columnState)
    : matrix === "b" && state && input ? `${state}/${input}`
    : matrix === "c" && output && columnState ? `${output}/${columnState}`
    : matrix === "d" && output && input ? `${output}/${input}` : undefined;
  if (!expected || unit !== expected) {
    throw new Error(`Sampled ${matrix.toUpperCase()} coefficient ${row},${column} requires ${expected ?? "compatible model-coordinate"} units.`);
  }
}

function ratioUnit(numerator: string, denominator: string): string {
  return numerator === denominator ? "1" : `${numerator}/${denominator}`;
}
