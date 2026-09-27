import { experimentFromSamples } from "./identificationExperiment";
import type { StudioAnalysisSessions } from "./studioAnalysisSessions";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { BoundedAnalysisJob } from "./boundedAnalysisJob";
import { decodeWorkflowResult } from "./workflowResult";
import { MAX_RECORDING_IMPORT_BYTES, readIdentificationRecordingImport } from "./identificationRecordingImport";
import { restoreIdentificationInput } from "./identificationState";
import { identifiedModelPresentation } from "./identification";
import { fitRecordedModel } from "./recordedModelIdentification";
import { generalModelFitComparison } from "./generalModelFitPresentation";
import { configuredRecordingSelection, openedRecordingCatalog } from "./openedRecordingChannels";
import { fitSelectedRecordingModel } from "./selectedRecordingIdentification";

/** Opens data through the existing qualification/fitting owners; it cannot acquire or write runtime signals. */
export async function identifyOpenedRecording(sessions: StudioAnalysisSessions, loop: ControlLoopDefinition, json: unknown,
  fitting: BoundedAnalysisJob, current: () => boolean, post: (payload: Record<string, unknown>) => Promise<void>, requestedSelection?: unknown): Promise<void> {
  if (json !== undefined && (typeof json !== "string" || Buffer.byteLength(json, "utf8") > MAX_RECORDING_IMPORT_BYTES)) throw new Error("Choose a Control Studio recording of at most 32 MiB.");
  let decoded: unknown;
  if (typeof json === "string") {
    try { decoded = JSON.parse(json); } catch { throw new Error("Choose a valid Control Studio recording JSON file."); }
  }
  const file = decoded === undefined ? undefined : decodeWorkflowResult(decoded);
  if (json !== undefined && !file) throw new Error("The recording file has invalid data or source metadata.");
  sessions.engineer?.assertModelReplacementAllowed();
  const retained = sessions.engineer?.snapshotStudy();
  const sourceCampaigns = file ? file.engineerStudy?.campaigns
    : retained?.recordingCatalog?.kind === "archive" ? retained.campaigns.filter(campaign => campaign.id.startsWith("opened-recording:")).slice(-1) : undefined;
  const archived = sourceCampaigns?.filter(campaign => campaign.status === "completed")
    .flatMap(campaign => campaign.runs).filter(result => result.request.simulation_override === undefined
      && result.run.evidence === "ST-executed" && result.evaluation.status === "Pass") ?? [];
  if (!file && !archived.length) throw new Error("Open the recording again to identify different signals.");
  const catalog = openedRecordingCatalog(file ?? { units: { sp: loop.signals.processVariable.unit, pv: loop.signals.processVariable.unit, mv: loop.signals.manipulatedVariable.unit } }, loop, archived, requestedSelection);
  sessions.engineer?.retainRecordingCatalog(catalog);
  if (archived.length) sessions.engineer?.retainOpenedRecordings(archived);
  const publish = post;
  post = payload => publish({ ...payload, recordingCatalog: catalog });
  await post({ status: "running", reason: "Checking recording…" });
  if (!catalog.selection.inputs.length || !catalog.selection.outputs.length) {
    sessions.model.restore(undefined); sessions.engineer?.modelChanged();
    await post({ status: "recording-needed", reason: "Choose the recording's plant inputs and measured outputs.", stageEvidence: sessions.engineer?.currentStageEvidence() });
    return;
  }
  if (archived.length && sessions.engineer) {
    const signals = [...catalog.selection.inputs, ...catalog.selection.outputs].map(id => ({ symbol: id, unit: catalog.channels.find(channel => channel.id === id)!.unit }));
    if (archived.some(result => signals.some(signal => !result.run.channels.some(channel => channel.id === signal.symbol && channel.unit === signal.unit)))) {
      throw new Error("The recording does not contain the selected plant signals and units.");
    }
    const latest = archived[archived.length - 1];
    const eligible = sessions.engineer.snapshotStudy()?.campaigns.filter(campaign => campaign.status === "completed")
      .flatMap(campaign => campaign.runs).filter(result => result.request.simulation_override === undefined
        && result.projectRevision === latest.projectRevision && result.run.conditions.modelRevision === latest.run.conditions.modelRevision
        && signals.every(signal => result.run.channels.some(channel => channel.id === signal.symbol && channel.unit === signal.unit))) ?? [];
    const records = [...new Map(eligible.map(result => [result.run.id, result])).values()].slice(-2);
    sessions.model.restore(undefined);
    sessions.engineer.modelChanged();
    if (records.length < 2) {
      await post({ status: "recording-needed", reason: "Open an independent recording to check the plant model.", stageEvidence: sessions.engineer.currentStageEvidence() });
      return;
    }
    const configured = configuredRecordingSelection(loop);
    const sameSelection = JSON.stringify(catalog.selection) === JSON.stringify(configured);
    if (!sameSelection) {
      const fitted = await fitSelectedRecordingModel(loop, records, catalog.selection, fitting, () => !current());
      if (!current()) return;
      sessions.identification.restore(undefined);
      sessions.model.selectModel(fitted.model);
      sessions.engineer.retainModelComparison(fitted.comparison);
      await post(sessions.engineer.modelChanged());
      return;
    }
    const fitted = await fitRecordedModel(loop, records, fitting, () => !current());
    if (!current()) return;
    sessions.identification.restore(undefined);
    if ("accepted" in fitted) sessions.acceptIdentification(fitted.accepted, loop);
    else {
      sessions.model.selectModel(fitted.model);
      if (fitted.fit) sessions.engineer.retainModelComparison(generalModelFitComparison(fitted.model, fitted.fit, records, loop));
    }
    await post(sessions.engineer.modelChanged());
    return;
  }
  if (catalog.selection.inputs.join() !== "appliedMv" || catalog.selection.outputs.join() !== "pv") {
    throw new Error("This legacy recording contains one applied input and one measured output.");
  }
  if (loop.channels && (loop.channels.inputs.length > 1 || loop.channels.outputs.length > 1)) throw new Error("Open the synchronized multichannel acquisition archive for this signal selection.");
  const imported = readIdentificationRecordingImport(json as string, loop);
  sessions.selectIdentificationRecording(imported.recording, imported.reference);
  const input = restoreIdentificationInput(sessions.identification.snapshot()!.current);
  const checked = sessions.qualifyIdentification(input.estimation, input.excluded, input.source, input.requestedKind,
    input.estimationRecordingId, input.stepResponseSettings, input.recording, input.firstOrderStepResponseSettings);
  if (!checked.qualification.suitable) throw new Error(checked.qualification.reason ?? "This recording cannot identify a plant model.");
  const review = await sessions.fitIdentification();
  if (!current() || !review) return;
  if (review.stage === "validation_required") {
    const presentation = identifiedModelPresentation(review.selected, review.fit, loop.signals.manipulatedVariable.unit, loop.signals.processVariable.unit);
    presentation.experiment = experimentFromSamples(loop, review.estimation);
    await post({ status: "recording-needed", identificationPreview: { kind: review.selected.kind, equation: presentation.equation },
      identificationPresentation: presentation, stageEvidence: sessions.engineer?.currentStageEvidence(),
      reason: "Open an independent recording to check this model." });
    return;
  }
  sessions.acceptIdentification(sessions.identification.accept(), loop);
  await post(sessions.engineer?.modelChanged() ?? { status: "failed", reason: "Choose a configured controller before tuning." });
}
