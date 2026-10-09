import { acquisitionSettings, configuredAcquisition } from "../acquisitionSettings";
import type { AutomaticIdentificationResult } from "../automaticIdentification";
import { ConnectedIdentification } from "../connectedIdentification";
import { assertQualifiedModelResult } from "../engineerModelCompatibility";
import { modelReadyReason } from "../engineerWorkflowPresentation";
import { generalModelFitComparison } from "../generalModelFitPresentation";
import type { ModelComparison } from "../modelComparison";
import type { RecordedModelFit } from "../recordedModelIdentification";
import { connectRuntimeSearch } from "../runtimeSearchConnection";
import { requireLiveWrite } from "../runtimeTarget";
import { bindingRuntime, MainWorkflowError, qualifyModel } from "../mainWorkflowFailure";
import type { EngineerActionContext, EngineerActionHost } from "./host";
import { randomUUID } from "crypto";

/** Identify: runs the connected identification campaign and hands its accepted model to the study. */
export async function identifyPlant(host: EngineerActionHost, { payload, respond, generation }: EngineerActionContext): Promise<boolean> {
  host.state.recordingCatalog = undefined;
  if (payload.acquisitionSettings !== undefined) {
    const settings = acquisitionSettings(payload.acquisitionSettings);
    if (!settings) throw new MainWorkflowError("Review acquisition method, duration, amplitude and relay direction before finding a model.", { cause: "settings", nextAction: "open-limits" });
    host.state.acquisitionPreferences = settings;
  }
  const acquisitionLoop = configuredAcquisition(host.loop(), host.state.acquisitionPreferences);
  host.state.campaignPassed = false; host.state.verifiedInputs = undefined;
  host.state.design = undefined; host.state.verified = undefined; host.state.initialization = undefined; host.state.values = undefined; host.state.review = undefined;
  host.state.busy = "verify";
  try {
    await respond({ status: "running", reason: "Preparing the simulator…" });
    const cancelled = () => generation !== host.state.generation;
    const connection = await bindingRuntime(() => connectRuntimeSearch(host.loop(), { cancelled }), cancelled);
    requireLiveWrite(connection, "automatic identification campaign");
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
    await respond(acceptIdentificationResult(host, found));
  } finally { if (!host.state.cancelling && host.state.cancellationOwner?.snapshot()?.status !== "cancellation-unconfirmed") { host.state.busy = undefined; host.state.cancellationOwner = undefined; } }
  return true;
}

/** A simpler model that was tried first and not used is one line of the comparison the engineer reads. */
export function withSimplerModelNote(comparison: ModelComparison, fallbackReasons: readonly string[]): ModelComparison {
  if (!fallbackReasons.length) return comparison;
  const note = `A simpler model was not used: ${fallbackReasons.join(" ").replace(/\.?$/, ".")}`;
  return { ...comparison, presentation: { ...comparison.presentation, warnings: [...comparison.presentation.warnings, note] } };
}

/**
 * Stores a finished identification as Identify stores one: the fitted model becomes the selected model, its comparison is
 * kept, and the stage payload the page shows is returned. Identify and a resumed campaign (A8.5) share it.
 */
export function acceptIdentificationResult(host: EngineerActionHost, found: AutomaticIdentificationResult): Record<string, unknown> {
  host.state.modelValidation = undefined;
  if ("model" in found) {
    if (!host.acceptFittedModel) throw new Error("Identification has no selected-model handoff.");
    host.acceptFittedModel(found.model);
  } else host.acceptIdentification(found.accepted);
  host.clearIncompatiblePin();
  const selected = host.model();
  if (!selected) throw new Error("Find model did not select its accepted model.");
  qualifyModel(() => assertQualifiedModelResult(host.loop(), selected));
  if ("accepted" in found) host.retainIdentificationComparison(found.accepted);
  else host.state.modelComparison = found.fit ? generalModelFitComparison(selected, found.fit, found.runs, host.loop()) : undefined;
  if (host.state.modelComparison) host.state.modelComparison = withSimplerModelNote(host.state.modelComparison, (found as RecordedModelFit).fallbackReasons ?? []);
  const relayCycle = host.relayCycle();
  return { status: "model-ready", model: selected, compatibleControllerRequired: host.currentStageEvidence()?.hasModel !== true, identification: found,
    ...(host.state.modelComparison ? { identificationPresentation: host.state.modelComparison.presentation } : {}), ...(relayCycle ? { relayCycle } : {}),
    reason: modelReadyReason(selected, host.availableFamilies()) };
}
