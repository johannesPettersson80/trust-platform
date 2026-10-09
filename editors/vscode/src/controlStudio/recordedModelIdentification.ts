import * as path from "path";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { RuntimeVerificationResult } from "./verificationClient";
import type { BoundedAnalysisJob } from "./boundedAnalysisJob";
import type { AcceptedIdentificationModel, IdentificationSample } from "./identification";
import type { AutomaticIdentificationResult } from "./automaticIdentification";
import { fitRecordedStateModel } from "./recordedStateIdentification";
import { fitMultichannelRecordings } from "./multichannelIdentification";
import { gainSignContradiction } from "./numerics/identification";
import { baselineEndSeconds, experimentStartSeconds } from "./recordedExperimentStart";

/** A fitted model, with the reasons that simpler attempts before it were not used. */
export type RecordedModelFit = AutomaticIdentificationResult & { fallbackReasons?: readonly string[] };

/** The same independent-data model selection for live campaigns and opened recordings. */
export async function fitRecordedModel(loop: ControlLoopDefinition, runs: readonly RuntimeVerificationResult[],
  fitting: BoundedAnalysisJob, cancelled: () => boolean): Promise<RecordedModelFit> {
  if (runs.length < 1 || runs.length > 2) throw new Error("One estimation recording or two independent recordings are required.");
  if ((loop.channels?.inputs.length ?? 1) > 1 || (loop.channels?.outputs.length ?? 1) > 1) {
    const fit = await fitMultichannelRecordings(loop, runs, fitting, cancelled);
    return { model: fit.model, runs, fit };
  }
  // One recording: fit the simple models first, from a step or from any other recorded excitation. The executed
  // experiment decides: Automatic may run a setpoint step, whose closed-loop input is an arbitrary excitation.
  const executed = (runs[0].request as { experiment?: { kind?: unknown } } | undefined)?.experiment?.kind;
  const kind = typeof executed === "string" ? executed : loop.experiment?.kind ?? "";
  const provisional = ["mv_step", "mv_pulse", "output_step"].includes(kind) ? { worker: "provisionalStepIdentificationWorker.js", label: "Step-response fit" }
    : ["relay", "prbs", "multisine", "setpoint_step", "load_step"].includes(kind) ? { worker: "provisionalExcitationIdentificationWorker.js", label: "Excitation fit" } : undefined;
  // Every attempt that does not yield the model keeps its reason, in the order the attempts ran.
  const failed: { label: string; reason: string }[] = [];
  if (runs.length === 1 && provisional) {
    let result: AutomaticIdentificationResult | { refusal: string } | undefined;
    try {
      result = await fitting.run<AutomaticIdentificationResult | { refusal: string }>(path.join(__dirname, provisional.worker), {
        run: runs[0], fitFromSeconds: experimentStartSeconds(runs[0]), baselineEndSeconds: baselineEndSeconds(runs[0]),
        input: loop.signals.manipulatedVariable, output: loop.signals.processVariable,
        inputId: loop.channels?.inputs[0].modelChannelId, outputId: loop.channels?.outputs[0].modelChannelId,
        maximumNormalizedError: loop.identificationPolicy?.maximumNormalizedError ?? 0.15,
        declaredProcess: declaredProcessOf(loop),
      });
      if (!result || cancelled()) throw new Error("Identification cancelled.");
    } catch (error) {
      if (cancelled()) throw error;
      failed.push({ label: provisional.label, reason: reasonOf(error) });
    }
    // Outside the attempt: a refusal is the answer and no state model may replace it; a fit that is not refused is checked for its sign.
    if (result && "refusal" in result) throw new Error(result.refusal);
    if (result && !("refusal" in result)) return checkedAction(loop, result);
  }
  if (runs.length === 2) {
    const mv = loop.signals.manipulatedVariable, pv = loop.signals.processVariable;
    const samples = (result: RuntimeVerificationResult): IdentificationSample[] => result.run.samples
      .filter(sample => sample.timeSeconds >= experimentStartSeconds(result) - 1e-9).map(sample => ({
      timeSeconds: sample.timeSeconds, input: sample.values[mv.symbol]?.value ?? NaN, output: sample.values[pv.symbol]?.value ?? NaN,
      inputUnit: mv.unit, outputUnit: pv.unit,
      quality: sample.values[mv.symbol]?.quality === "good" && sample.values[pv.symbol]?.quality === "good" ? "good" : "bad",
    }));
    let accepted: AcceptedIdentificationModel | undefined;
    try {
      accepted = await fitting.run<AcceptedIdentificationModel>(path.join(__dirname, "identificationWorker.js"), {
        estimation: samples(runs[0]), validation: samples(runs[1]),
        estimationRecordingId: runs[0].run.id, validationRecordingId: runs[1].run.id,
        maximumNormalizedError: loop.identificationPolicy?.maximumNormalizedError ?? 0.15,
      });
      if (!accepted || cancelled()) throw new Error("Identification cancelled.");
    } catch (error) {
      if (cancelled()) throw error;
      failed.push({ label: "Two-recording fit", reason: reasonOf(error) });
    }
    if (accepted) return checkedAction(loop, { accepted, runs });
  }
  try {
    const fit = await fitRecordedStateModel(loop, runs, fitting, cancelled);
    return { model: fit.model, runs, fit, ...(failed.length ? { fallbackReasons: failed.map(attempt => attempt.reason) } : {}) };
  } catch (error) {
    if (cancelled()) throw error;
    throw new Error([...failed.map(attempt => sentence(`${attempt.label}: ${attempt.reason}`)), sentence(`State model: ${reasonOf(error)}`)].join(" "));
  }
}

/** Refuses a fitted gain whose sign contradicts the loop's controller action (B3.9, N6.3). A zero gain is not judged here. */
export function checkGainSign(loop: ControlLoopDefinition, gain: number, integrating: boolean): void {
  const gainUnit = `${loop.signals.processVariable.unit}/${loop.signals.manipulatedVariable.unit}${integrating ? "/s" : ""}`;
  const message = gainSignContradiction(gain, gainUnit, loop.controller.action);
  if (message) throw new Error(message);
}

function checkedAction(loop: ControlLoopDefinition, result: AutomaticIdentificationResult): AutomaticIdentificationResult {
  if ("accepted" in result) {
    checkGainSign(loop, result.accepted.model.gain, result.accepted.model.kind === "integrating");
    // A two-recording fit carries its refusals in the review it returns; they are raised here, outside the attempt loop.
    const fitted = result.accepted.review?.result;
    if (fitted?.refusal) throw new Error(fitted.refusal);
    if (fitted?.integratorRefusal && result.accepted.model.kind === "integrating" && declaredProcessOf(loop) !== "integrating") {
      throw new Error(fitted.integratorRefusal);
    }
  }
  else if ("numerator" in result.model) {
    const transfer = transferGain(result.model);
    if (transfer) checkGainSign(loop, transfer.gain, transfer.integrating);
  }
  return result;
}

/** The static gain of a transfer model, or its rate gain when the model integrates. The polynomials are in descending powers, so the
 * constant term is the last coefficient. Undefined for a model with neither a static nor a rate gain. */
function transferGain(model: { readonly numerator: readonly number[]; readonly denominator: readonly number[] }):
  { gain: number; integrating: boolean } | undefined {
  const top = model.numerator[model.numerator.length - 1];
  const constant = model.denominator[model.denominator.length - 1];
  if (constant !== 0) return { gain: top / constant, integrating: false };
  const rate = model.denominator[model.denominator.length - 2];
  return rate !== undefined && rate !== 0 ? { gain: top / rate, integrating: true } : undefined;
}

/** The process the loop's identification policy declares (B1.4). */
function declaredProcessOf(loop: ControlLoopDefinition): string | undefined {
  return loop.identificationPolicy?.process;
}

function reasonOf(error: unknown): string { return error instanceof Error ? error.message : String(error); }
/** Ends a reason with one full stop, without doubling one it already has. */
function sentence(text: string): string { return text.replace(/\.?$/, "."); }
