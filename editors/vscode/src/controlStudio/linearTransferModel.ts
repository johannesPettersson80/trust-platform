import { decodeGeneralModel, type GeneralTransferModel, type GeneralStateSpaceModel } from "./generalModel";

/** Full characteristic denominator is retained: hidden state modes must not disappear through cancellation. */
export function linearTransferModel(value: unknown): GeneralTransferModel {
  const model = decodeGeneralModel(value);
  if (!model) throw new Error("Select a valid linear model before controller analysis.");
  if (model.kind === "transfer-function") return model;
  if (model.kind === "discrete-state-space") {
    if (model.states.length !== 1 || model.inputs.length !== 1 || model.outputs.length !== 1
      || model.a[0][0] <= 0 || model.d[0][0] !== 0) {
      throw new Error("Classical tuning supports a positive one-state sampled model without direct feedthrough; use a compatible sampled/state controller for this model.");
    }
    const transition = model.a[0][0], period = model.sampleTimeSeconds!;
    const pole = Math.log(transition) / period;
    const input = model.b[0][0] * (transition === 1 ? 1 / period : pole / (transition - 1));
    const gain = model.c[0][0] * input;
    if (!Number.isFinite(pole) || !Number.isFinite(gain)) throw new Error("The sampled model has no finite supported held-input equivalent.");
    // Analysis only: keep the selected model and its native sample/update path intact.
    return { kind: "transfer-function", numerator: [gain], denominator: [1, pole === 0 ? 0 : -pole],
      revision: model.revision, source: model.source, validation: model.validation, evidence: model.evidence,
      inputs: model.inputs, outputs: model.outputs, delaySeconds: model.delaySeconds,
      ...(model.operatingPoint ? { operatingPoint: model.operatingPoint } : {}) };
  }
  const { numerator, denominator } = stateSpaceTransferPolynomials(model);
  return { kind: "transfer-function", numerator, denominator, revision: model.revision,
    source: model.source, validation: model.validation, evidence: model.evidence,
    inputs: model.inputs, outputs: model.outputs, delaySeconds: model.delaySeconds,
    ...(model.operatingPoint ? { operatingPoint: model.operatingPoint } : {}) };
}


/** Exact SISO resolvent coefficients in the model's own s or z domain. */
export function stateSpaceTransferPolynomials(model: GeneralStateSpaceModel): { numerator: number[]; denominator: number[] } {
  if (model.inputs.length !== 1 || model.outputs.length !== 1) throw new Error("Transfer analysis requires a SISO model.");
  const n = model.states.length;
  if (n > 16) throw new Error("State-space conversion exceeds the supported transfer-polynomial order.");
  let adjugate = Array.from({ length: n }, (_, row) => Array.from({ length: n }, (_, column) => Number(row === column)));
  const denominator = [1];
  const numerator = [model.d[0][0]];
  let scale = 1;
  for (let order = 1; order <= n; order++) {
    const next = model.a.map(row => Array.from({ length: n }, (_, column) =>
      row.reduce((sum, entry, k) => sum + entry * adjugate[k][column], 0)));
    const coefficient = -next.reduce((sum, row, i) => sum + row[i], 0) / order;
    denominator.push(coefficient === 0 ? 0 : coefficient);
    const markov = model.c[0].reduce((sum, entry, row) => sum + entry *
      adjugate[row].reduce((total, value, column) => total + value * model.b[column][0], 0), 0);
    numerator.push(model.d[0][0] * coefficient + markov);
    scale = Math.max(scale, Math.abs(coefficient), ...next.flat().map(Math.abs));
    adjugate = next.map((row, i) => row.map((entry, j) => entry + (i === j ? coefficient : 0)));
  }
  if (![...denominator, ...numerator, ...adjugate.flat(), scale].every(Number.isFinite)
      || Math.max(...adjugate.flat().map(Math.abs)) > 1e-8 * scale) {
    throw new Error("State-space transfer conversion has numerical uncertainty.");
  }
  while (numerator.length > 1 && numerator[0] === 0) numerator.shift();
  return { numerator, denominator };
}
