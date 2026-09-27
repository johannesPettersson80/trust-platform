import { createHash } from "crypto";
import { solveLeastSquares } from "./numerics";
import { physicalStateModel, type PhysicalBalance } from "./physicalModel";
import type { IdentificationChannels, IdentifiedStateModelResult, ValidationPredictionPoint } from "./identifiedStateModel";
import type { RecordedRun } from "./runValidation";

export interface PhysicalIdentification { kind: PhysicalBalance["kind"] }
export function validPhysicalIdentification(value: unknown): value is PhysicalIdentification {
  return !!value && typeof value === "object" && !Array.isArray(value)
    && ["gravity-tank", "gravity-tank-cascade", "thermal-balance"].includes(String((value as PhysicalIdentification).kind));
}

/** Integral balances avoid differentiating noisy measurements; validation predicts its own state history. */
export function identifyPhysicalModel(estimation: RecordedRun, validation: RecordedRun | undefined, channels: IdentificationChannels,
  definition: PhysicalIdentification, tolerance = 0.15): IdentifiedStateModelResult {
  const n = definition.kind === "gravity-tank-cascade" ? 2 : 1;
  if (!validPhysicalIdentification(definition) || channels.inputs.length !== 1 || channels.outputs.length !== n
    || validation && (estimation.id === validation.id || estimation.conditions.programRevision !== validation.conditions.programRevision
    || estimation.conditions.modelRevision !== validation.conditions.modelRevision) || !Number.isFinite(tolerance) || tolerance <= 0 || tolerance >= 1) throw new Error("Physical fitting needs independent compatible records and a declared error tolerance.");
  const read = (run: RecordedRun) => {
    if (run.samples.length < 30 || run.samples.length !== run.conditions.sampleTimesSeconds.length) throw new Error("Physical fitting requires complete independent records.");
    for (const channel of [...channels.inputs, ...channels.outputs]) if (!run.channels.some(value => value.id === channel.id && value.unit === channel.unit)) throw new Error("Physical fitting channel units disagree.");
    return run.samples.map((sample, index) => {
      if (sample.timeSeconds !== run.conditions.sampleTimesSeconds[index] || index && sample.sequence !== run.samples[index - 1].sequence + 1) throw new Error("Physical fitting records have missing or unordered samples.");
      const values = [...channels.inputs, ...channels.outputs].map(channel => sample.values[channel.id]);
      if (values.some(value => !value || value.quality !== "good" || value.value === null || !Number.isFinite(value.value))) throw new Error("Physical fitting needs finite, good-quality measurements.");
      return { time: sample.timeSeconds, input: values[0].value!, outputs: values.slice(1).map(value => value.value!) };
    });
  };
  const comparisonRecord = validation ?? estimation;
  const train = read(estimation), held = read(comparisonRecord), matrix: number[][] = [], targets: number[] = [];
  for (let index = 0; index < train.length - 1; index++) {
    const current = train[index], next = train[index + 1], dt = next.time - current.time;
    if (!(dt > 0)) throw new Error("Physical fitting requires increasing sample times.");
    if (definition.kind === "thermal-balance") {
      matrix.push([dt * current.input, -dt * (current.outputs[0] + next.outputs[0]) / 2, dt]);
      targets.push(next.outputs[0] - current.outputs[0]);
    } else {
      if ([...current.outputs, ...next.outputs].some(level => level <= 0)) throw new Error("Collect positive-level records to identify and linearize the gravity balance.");
      const roots = current.outputs.map((level, channel) => (Math.sqrt(level) + Math.sqrt(next.outputs[channel])) / 2);
      matrix.push([dt * current.input, -dt * roots[0]]); targets.push(next.outputs[0] - current.outputs[0]);
      if (n === 2) { matrix.push([0, dt * (roots[0] - roots[1])]); targets.push(next.outputs[1] - current.outputs[1]); }
    }
  }
  const fit = solveLeastSquares(matrix, targets, { absolute: 1e-10, relative: 1e-9, conditionLimit: 1e10 });
  if (fit.status !== "qualified" || fit.solution[0] <= 0 || fit.solution[1] <= 0) throw new Error("The physical parameters are not identifiable, or violate positive storage/loss constraints.");
  const [storageInverse, lossRate, ambientTerm] = fit.solution;
  const physical: PhysicalBalance = definition.kind === "thermal-balance"
    ? { kind: definition.kind, heatCapacityJPerK: 1 / storageInverse, lossWattsPerK: lossRate / storageInverse, ambientTemperature: ambientTerm / lossRate }
    : { kind: definition.kind, areaM2: 1 / storageInverse, outflowCoefficient: lossRate / storageInverse };
  let predicted = [...held[0].outputs];
  const errors = Array<number>(n).fill(0);
  const predictionPoints = channels.outputs.map(() => [] as ValidationPredictionPoint[]);
  for (let index = 0; index < held.length - 1; index++) {
    const dt = held[index + 1].time - held[index].time;
    if (!(dt > 0)) throw new Error("Physical validation requires increasing sample times.");
    if (definition.kind === "thermal-balance") predicted = [(predicted[0] * (1 - lossRate * dt / 2) + dt * (storageInverse * held[index].input + ambientTerm)) / (1 + lossRate * dt / 2)];
    else {
      const previous = [...predicted];
      for (let channel = 0; channel < n; channel++) {
        const inflow = channel === 0 ? storageInverse * held[index].input : lossRate * (Math.sqrt(previous[0]) + Math.sqrt(predicted[0])) / 2;
        const right = previous[channel] + dt * inflow - dt * lossRate * Math.sqrt(previous[channel]) / 2;
        if (right < 0) throw new Error("Independent gravity validation reaches the dry-level boundary; use a valid positive operating range.");
        const coefficient = dt * lossRate / 2;
        predicted[channel] = ((Math.sqrt(coefficient * coefficient + 4 * right) - coefficient) / 2) ** 2;
      }
    }
    errors.forEach((_, channel) => {
      const measured = held[index + 1].outputs[channel];
      predictionPoints[channel].push({
        timeSeconds: held[index + 1].time,
        measured,
        predicted: predicted[channel],
        residual: measured - predicted[channel],
      });
      errors[channel] += (predicted[channel] - measured) ** 2;
    });
  }
  const normalizedErrors = errors.map((error, channel) => Math.sqrt(error / (held.length - 1)) /
    Math.max(1e-9, Math.max(...held.map(sample => sample.outputs[channel])) - Math.min(...held.map(sample => sample.outputs[channel]))));
  if (normalizedErrors.some(error => !Number.isFinite(error) || error > tolerance)) throw new Error("The independently predicted physical balance does not meet the declared adequacy limit; retain data and revise the structure.");
  const operating = train.reduce((sum, sample) => sum + sample.outputs[0], 0) / train.length;
  const evidence = validation ? [digest(estimation), digest(validation)] : [digest(estimation)];
  const model = physicalStateModel({ revision: digest({ physical, operating, evidence }), source: "fitted", validation: validation ? "validated" : "unvalidated", evidence, ...channels }, physical, Array<number>(n).fill(operating));
  return { model, comparisons: [{ order: n, normalizedErrors, adequate: true,
    reason: validation ? "Identifiable integral-balance parameters passed independent free-running validation; the physical law and local Jacobian are retained together."
      : "Integral-balance estimation fit; independent validation is pending." }],
    validationPredictions: validation ? channels.outputs.map((channel, index) => ({
      channelId: channel.id,
      unit: channel.unit,
      recordingId: comparisonRecord.id,
      modelRevision: model.revision,
      points: predictionPoints[index],
    })) : [],
    ...(!validation ? { estimationPredictions: channels.outputs.map((channel, index) => ({ channelId: channel.id, unit: channel.unit, recordingId: estimation.id, modelRevision: model.revision, points: predictionPoints[index] })) } : {}) };
}
function digest(value: unknown): string { return `sha256:${createHash("sha256").update(JSON.stringify(value)).digest("hex")}`; }
