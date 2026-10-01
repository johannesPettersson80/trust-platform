import { acquisitionSettings, configuredAcquisition } from "../acquisitionSettings";
import type { AutomaticIdentificationResult } from "../automaticIdentification";
import { ConnectedIdentification } from "../connectedIdentification";
import { assertQualifiedModelResult } from "../engineerModelCompatibility";
import { modelReadyReason } from "../engineerWorkflowPresentation";
import { generalModelFitComparison } from "../generalModelFitPresentation";
import { connectRuntimeSearch } from "../runtimeSearchConnection";
import type { EngineerActionContext, EngineerActionHost } from "./host";
import { randomUUID } from "crypto";

/** Identify: runs the connected identification campaign and hands its accepted model to the study. */
export async function identifyPlant(host: EngineerActionHost, { payload, respond, generation }: EngineerActionContext): Promise<boolean> {
  host.state.recordingCatalog = undefined;
  if (payload.acquisitionSettings !== undefined) {
    const settings = acquisitionSettings(payload.acquisitionSettings);
    if (!settings) throw new Error("Review acquisition method, duration, amplitude and relay direction before finding a model.");
    host.state.acquisitionPreferences = settings;
  }
  const acquisitionLoop = configuredAcquisition(host.loop(), host.state.acquisitionPreferences);
  host.state.campaignPassed = false; host.state.verifiedInputs = undefined;
  host.state.design = undefined; host.state.verified = undefined; host.state.initialization = undefined; host.state.values = undefined; host.state.review = undefined;
  host.state.busy = "verify";
  try {
    await respond({ status: "running", reason: "Preparing the simulator…" });
    const connection = await connectRuntimeSearch(host.loop(), { cancelled: () => generation !== host.state.generation });
    if (connection.source !== "simulation") throw new Error("This automatic campaign requires the local simulator.");
    if (generation !== host.state.generation) return true;
    if (!host.liveIdentification) throw new Error("The live plant acquisition service is unavailable.");
    host.state.identification = new ConnectedIdentification(connection.request, host.liveIdentification);
    const campaign = host.state.identification;
    host.state.cancellationOwner = campaign.cancellationOwner();
    let found: AutomaticIdentificationResult;
    await respond({ status: "running", liveIdentification: true });
    const finding = campaign.run(acquisitionLoop, reason => respond({ status: "running", reason }));
    host.state.pendingFind = finding;
    try { found = await finding; }
    finally {
      if (host.state.pendingFind === finding) host.state.pendingFind = undefined;
      host.state.campaigns.push({ id: randomUUID(), ...campaign.snapshot() });
      host.retainAttempt(campaign.cancellationOwner().snapshot());
      campaign.dispose();
    }
    if (generation !== host.state.generation) return true;
    host.state.modelValidation = undefined;
    if ("model" in found) {
      if (!host.acceptFittedModel) throw new Error("Identification has no selected-model handoff.");
      host.acceptFittedModel(found.model);
    } else host.acceptIdentification(found.accepted);
    host.clearIncompatiblePin();
    const selected = host.model();
    if (!selected) throw new Error("Find model did not select its accepted model.");
    assertQualifiedModelResult(host.loop(), selected);
    if ("accepted" in found) host.retainIdentificationComparison(found.accepted);
    else host.state.modelComparison = found.fit ? generalModelFitComparison(selected, found.fit, found.runs, host.loop()) : undefined;
    const relayCycle = host.relayCycle();
    await respond({ status: "model-ready", model: selected, compatibleControllerRequired: host.currentStageEvidence()?.hasModel !== true, identification: found,
      ...(host.state.modelComparison ? { identificationPresentation: host.state.modelComparison.presentation } : {}), ...(relayCycle ? { relayCycle } : {}),
      reason: modelReadyReason(selected, host.availableFamilies()) });
  } finally { if (!host.state.cancelling && host.state.cancellationOwner?.snapshot()?.status !== "cancellation-unconfirmed") { host.state.busy = undefined; host.state.cancellationOwner = undefined; } }
  return true;
}
