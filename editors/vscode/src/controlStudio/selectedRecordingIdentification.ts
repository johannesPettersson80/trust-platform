import { experimentFromRun } from "./identificationExperiment";
import * as path from "path";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { RuntimeVerificationResult } from "./verificationClient";
import type { BoundedAnalysisJob } from "./boundedAnalysisJob";
import type { AcceptedIdentificationModel, IdentificationSample } from "./identification";
import { identifiedModelPresentation } from "./identification";
import type { IdentifiedStateModelResult } from "./identifiedStateModel";
import type { GeneralModel } from "./generalModel";
import type { ModelComparison } from "./modelComparison";
import { acceptedIdentificationModel } from "./selectedModelAnalysis";
import { generalModelFitComparison } from "./generalModelFitPresentation";
import { selectedRecordingChannels, type RecordingSignalSelection } from "./openedRecordingChannels";
import { loopSignal } from "./loopChannels";

/** Reuse the existing independent-data fitters with explicit recording coordinates. */
export async function fitSelectedRecordingModel(loop: ControlLoopDefinition, runs: readonly RuntimeVerificationResult[],
  selection: RecordingSignalSelection, fitting: BoundedAnalysisJob, cancelled: () => boolean): Promise<{ model: GeneralModel; comparison: ModelComparison }> {
  if (runs.length !== 2 || runs[0].run.id === runs[1].run.id || runs[0].projectRevision !== runs[1].projectRevision) {
    throw new Error("Two independent recordings from the same project revision are required.");
  }
  const channels = selectedRecordingChannels(runs, selection);
  const inputId = (id: string) => loop.channels?.inputs.find(channel => loopSignal(loop, channel.signalId).symbol === id)?.modelChannelId ?? id;
  const outputId = (id: string) => loop.channels?.outputs.find(channel => loopSignal(loop, channel.signalId).symbol === id)?.modelChannelId ?? id;
  const modelChannels = { inputs: channels.inputs.map(channel => ({ ...channel, id: inputId(channel.id) })),
    outputs: channels.outputs.map(channel => ({ ...channel, id: outputId(channel.id) })) };
  if (channels.inputs.length === 1 && channels.outputs.length === 1) {
    const input = channels.inputs[0], output = channels.outputs[0];
    const samples = (result: RuntimeVerificationResult): IdentificationSample[] => result.run.samples.map(sample => ({
      timeSeconds: sample.timeSeconds, input: sample.values[input.id].value!, output: sample.values[output.id].value!,
      inputUnit: input.unit, outputUnit: output.unit, quality: "good",
    }));
    try {
      const accepted = await fitting.run<AcceptedIdentificationModel>(path.join(__dirname, "identificationWorker.js"), {
        estimation: samples(runs[0]), validation: samples(runs[1]), estimationRecordingId: runs[0].run.id, validationRecordingId: runs[1].run.id,
        maximumNormalizedError: loop.identificationPolicy?.maximumNormalizedError ?? 0.15,
      });
      if (!accepted || cancelled()) throw new Error("Identification cancelled.");
      const model = acceptedIdentificationModel(accepted, modelChannels);
      const presentation = identifiedModelPresentation(accepted.model, accepted.fit, input.unit, output.unit);
      presentation.experiment = experimentFromRun(loop, runs[1].run, channels);
      presentation.outputId = model.outputs[0].id;
      presentation.inputChannels = model.inputs.map(({ id, unit }) => ({ id, unit }));
      return { model, comparison: { modelRevision: model.revision, presentation } };
    } catch (error) { if (cancelled()) throw error; }
  }
  const fit = await fitting.run<IdentifiedStateModelResult>(path.join(__dirname, "generalIdentificationWorker.js"), {
    estimation: runs[0].run, validation: runs[1].run, channels,
    maximumNormalizedError: loop.identificationPolicy?.maximumNormalizedError ?? 0.15,
    maximumOrder: loop.identificationPolicy?.maximumOrder ?? 4,
  });
  if (!fit || cancelled()) throw new Error("Identification cancelled.");
  const model = { ...fit.model, inputs: modelChannels.inputs, outputs: modelChannels.outputs };
  const mappedFit = { ...fit, model,
    validationPredictions: fit.validationPredictions.map(series => ({ ...series, channelId: outputId(series.channelId) })) };
  return { model, comparison: generalModelFitComparison(model, mappedFit, runs, loop, channels) };
}
