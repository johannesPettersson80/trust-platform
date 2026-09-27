import * as path from "path";
import { BoundedAnalysisJob } from "./boundedAnalysisJob";
import { ConnectedIdentification, type ConnectedIdentificationPorts } from "./connectedIdentification";
import { connectedModelChannels } from "./loopChannels";
import { recordingFingerprint, type IndependentModelValidation } from "./independentModelValidation";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import type { IdentificationCampaignRecord } from "./automaticIdentification";
import type { RuntimeRequest, RuntimeVerificationClient } from "./verificationClient";

export async function checkIndependentModel(loop: ControlLoopDefinition, model: GeneralModel, campaigns: readonly IdentificationCampaignRecord[],
  request: RuntimeRequest, ports: ConnectedIdentificationPorts | undefined, cancelled: () => boolean,
  attach: (acquisition: ConnectedIdentification, owner: Pick<RuntimeVerificationClient, "cancel" | "snapshot">) => void,
  retain: (campaign: IdentificationCampaignRecord) => void, progress: (payload: Record<string, unknown>) => Promise<void>): Promise<IndependentModelValidation> {
  if (!ports) throw new Error("The connected model-validation service is unavailable.");
  const estimation = campaigns.flatMap(campaign => campaign.runs).find(record => model.evidence.includes(recordingFingerprint(record.run)));
  if (!estimation) throw new Error("The provisional model's estimation recording is missing. Identify the plant again.");
  const mapping = connectedModelChannels(loop, model);
  const channels = { inputs: mapping.inputs.map((input, i) => ({ id: model.inputs[i].id, symbol: input.signal.symbol, unit: input.signal.unit })),
    outputs: mapping.outputs.map((output, i) => ({ id: model.outputs[i].id, symbol: output.signal.symbol, unit: output.signal.unit })) };
  const acquisition = new ConnectedIdentification(request, ports);
  const worker = new BoundedAnalysisJob(); const owner = acquisition.cancellationOwner();
  attach(acquisition, { cancel: async () => { worker.cancel(); await owner.cancel(); }, snapshot: owner.snapshot });
  try {
    await progress({ status: "running", liveModelValidation: true, activity: "model-check", reason: "Recording independent model validation…" });
    const record = await acquisition.recordValidation(loop, reason => progress({ status: "running", activity: "model-check", reason }));
    if (cancelled()) throw new Error("Model validation cancelled.");
    const report = await worker.run<IndependentModelValidation>(path.join(__dirname, "independentModelValidationWorker.js"), {
      model, estimation, record, channels, maximumNormalizedError: loop.identificationPolicy?.maximumNormalizedError ?? 0.15,
    });
    if (!report || cancelled()) throw new Error("Model validation cancelled.");
    return report;
  } finally { retain(acquisition.snapshot()); acquisition.dispose(); worker.cancel(); }
}
