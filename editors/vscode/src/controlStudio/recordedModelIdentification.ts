import * as path from "path";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { RuntimeVerificationResult } from "./verificationClient";
import type { BoundedAnalysisJob } from "./boundedAnalysisJob";
import type { AcceptedIdentificationModel, IdentificationSample } from "./identification";
import type { AutomaticIdentificationResult } from "./automaticIdentification";
import { fitRecordedStateModel } from "./recordedStateIdentification";
import { fitMultichannelRecordings } from "./multichannelIdentification";

/** The same independent-data model selection for live campaigns and opened recordings. */
export async function fitRecordedModel(loop: ControlLoopDefinition, runs: readonly RuntimeVerificationResult[],
  fitting: BoundedAnalysisJob, cancelled: () => boolean): Promise<AutomaticIdentificationResult> {
  if (runs.length < 1 || runs.length > 2) throw new Error("One estimation recording or two independent recordings are required.");
  if ((loop.channels?.inputs.length ?? 1) > 1 || (loop.channels?.outputs.length ?? 1) > 1) {
    const fit = await fitMultichannelRecordings(loop, runs, fitting, cancelled);
    return { model: fit.model, runs, fit };
  }
  if (runs.length === 1 && !loop.physicalIdentification && ["mv_step", "mv_pulse"].includes(loop.experiment?.kind ?? "")) {
    try {
      const result = await fitting.run<AutomaticIdentificationResult>(path.join(__dirname, "provisionalStepIdentificationWorker.js"), {
        run: runs[0], input: loop.signals.manipulatedVariable, output: loop.signals.processVariable,
        inputId: loop.channels?.inputs[0].modelChannelId, outputId: loop.channels?.outputs[0].modelChannelId,
      });
      if (!result || cancelled()) throw new Error("Identification cancelled.");
      return result;
    } catch (error) { if (cancelled()) throw error; }
  }
  if (runs.length === 2 && !loop.physicalIdentification) {
    const mv = loop.signals.manipulatedVariable, pv = loop.signals.processVariable;
    const samples = (result: RuntimeVerificationResult): IdentificationSample[] => result.run.samples.map(sample => ({
      timeSeconds: sample.timeSeconds, input: sample.values[mv.symbol]?.value ?? NaN, output: sample.values[pv.symbol]?.value ?? NaN,
      inputUnit: mv.unit, outputUnit: pv.unit,
      quality: sample.values[mv.symbol]?.quality === "good" && sample.values[pv.symbol]?.quality === "good" ? "good" : "bad",
    }));
    try {
      const accepted = await fitting.run<AcceptedIdentificationModel>(path.join(__dirname, "identificationWorker.js"), {
        estimation: samples(runs[0]), validation: samples(runs[1]),
        estimationRecordingId: runs[0].run.id, validationRecordingId: runs[1].run.id,
        maximumNormalizedError: loop.identificationPolicy?.maximumNormalizedError ?? 0.15,
      });
      if (!accepted || cancelled()) throw new Error("Identification cancelled.");
      return { accepted, runs };
    } catch (error) {
      if (cancelled()) throw error;
    }
  }
  const fit = await fitRecordedStateModel(loop, runs, fitting, cancelled);
  return { model: fit.model, runs, fit };
}
