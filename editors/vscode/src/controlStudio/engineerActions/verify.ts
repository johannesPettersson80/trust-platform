import { configuredAcquisition } from "../acquisitionSettings";
import { configuredPlantRequest, usesConfiguredPlant } from "../configuredPlantVerification";
import { controllerAdapter } from "../controllerAdapters";
import { engineerVerificationCampaign, campaignStatus } from "../engineerVerificationCampaign";
import { recordingFingerprint } from "../independentModelValidation";
import { connectedModelChannels } from "../loopChannels";
import { mainWorkflowNextActionLabel, verificationFailure, verificationFailureReason } from "../mainWorkflowFailure";
import { compareOnModel } from "../modelPredictionRuns";
import { checkIndependentModel } from "../modelValidationSession";
import { measuredProcessNoise } from "../processNoiseEvidence";
import { connectRuntimeParameters } from "../runtimeParameterConnection";
import { connectRuntimeSearch } from "../runtimeSearchConnection";
import { recordedHalfRange, restingNoiseProbeRequest, restingOperatingModel } from "../transferNoiseProbe";
import type { EngineerActionContext, EngineerActionHost } from "./host";
import { randomUUID } from "crypto";

/** Validate: checks the model when needed, then runs the actual ST of the current and tuned settings in every mandatory scenario. */
export async function validateCandidate(host: EngineerActionHost, { respond, generation, applicationMode }: EngineerActionContext): Promise<boolean> {
  let model = host.model(); const design = host.state.design; const task = host.state.task;
  if (!model || !design || !task || model.revision !== design.selectedModel.revision) throw new Error("Tune against the selected model before verification.");
  host.state.verified = undefined; host.state.initialization = undefined; host.state.review = undefined; host.state.scenarioResults.splice(0); host.state.busy = "verify";
  const verifying = (async () => { try {
    await respond({ status: "running", reason: "Preparing the simulator…" });
    const connection = await connectRuntimeSearch(host.loop(), { cancelled: () => generation !== host.state.generation });
    if (connection.source !== "simulation") throw new Error("Start the local simulator before Verify.");
    if (model.validation === "unvalidated") {
      host.state.requiredScenarios = ["plant-model"]; host.state.verifier?.dispose(); host.state.verifier = undefined;
      host.state.modelValidation = await checkIndependentModel(configuredAcquisition(host.loop(), host.state.acquisitionPreferences), model, host.state.campaigns,
        connection.request, host.liveIdentification, () => generation !== host.state.generation,
        (acquisition, owner) => { host.state.identification = acquisition; host.state.cancellationOwner = owner; },
        campaign => host.state.campaigns.push({ id: randomUUID(), ...campaign }), respond);
      if (generation !== host.state.generation) return true;
      if (host.state.modelValidation.status !== "Pass") {
        await respond({ status: "verification-failed", reason: host.state.modelValidation.rows.find(row => row.status !== "Pass")?.reason ?? "Independent model validation did not pass." });
        return true;
      }
      model = { ...model, validation: "validated", evidence: [...model.evidence, recordingFingerprint(host.state.modelValidation.record.run)] };
      if (!host.acceptFittedModel) throw new Error("The validated model has no workflow handoff.");
      host.acceptFittedModel(model);
      if (host.state.pinModel?.revision === model.revision) host.state.pinModel = model;
      if (host.state.modelComparison) host.state.modelComparison = { ...host.state.modelComparison, presentation: { ...host.state.modelComparison.presentation, uncertainty: { label: "Independent model validation passed on Validate." } } };
      await respond({ status: "running", model, activity: "controller-check", reason: "Model checked. Validating the tuned controller…" });
    }
    const parameterClient = await connectRuntimeParameters(host.loop());
    const observed = await parameterClient.client.status();
    if (generation !== host.state.generation) return true;
    const adapter = controllerAdapter(host.loop());
    const values = adapter.materialize(design.candidate, observed.values);
    const campaign = engineerVerificationCampaign(host.scenarioLoop(), model, task, values);
    host.state.requiredScenarios = [...(host.state.modelValidation ? ["plant-model"] : []), ...campaign.map(scenario => scenario.id), "controller-transfer"];
    host.state.scenarioResults.splice(0); host.state.campaignPassed = false;
    const comparison = await host.compareOnModel(connection, model, task, observed.values, values, generation);
    if (!comparison) return true;
    const { verifier, baseline, candidate, predictedBaseline, predictedCandidate } = comparison;
    await respond({ status: "running", modelPrediction: { baseline: predictedBaseline.evaluation, candidate: predictedCandidate.evaluation }, reason: "Comparing current and tuned responses on the identified model." });
    await respond({ status: "running", reason: "Executing baseline and candidate ST from the selected operating point, model initial states and declared controller initialization." });
    const baselineResult = await verifier.verify(configuredPlantRequest(host.loop(), baseline.request), baseline.requirements);
    if (generation !== host.state.generation) return true;
    host.state.recordedBaseline = baselineResult;
    const result = await verifier.verify(configuredPlantRequest(host.loop(), candidate.request), candidate.requirements);
    if (generation !== host.state.generation) return true;
    if (baselineResult.projectRevision !== result.projectRevision || result.projectRevision !== predictedCandidate.projectRevision) throw new Error("The project changed between prediction, baseline and candidate verification.");
    host.state.recordedCandidate = result;
    host.state.scenarioResults.push({ id: candidate.id, result });
    for (const scenario of campaign.slice(1)) {
      await respond({ status: "running", reason: `Executing mandatory scenario: ${scenario.id}.` });
      if (generation !== host.state.generation) return true;
      const scenarioResult = await verifier.verify(usesConfiguredPlant(host.scenarioLoop(), scenario.id) ? configuredPlantRequest(host.loop(), scenario.request) : scenario.request, scenario.requirements);
      host.state.scenarioResults.push({ id: scenario.id, result: scenarioResult });
      if (generation !== host.state.generation) return true;
      if (scenarioResult.projectRevision !== result.projectRevision) throw new Error("The program changed during the mandatory verification campaign.");
    }
    host.state.campaignPassed = campaignStatus(campaign.map(scenario => scenario.id), host.state.scenarioResults) === "Pass" && (!host.state.modelValidation || host.state.modelValidation.status === "Pass");
    host.updateFamilyEvaluation(host.state.family,
      host.state.campaignPassed ? "actual-pass" : "actual-fail",
      `Actual ST verification ${result.evaluation.status}.`);
    if (applicationMode === "auto" && host.state.campaignPassed) {
      // Hold the transfer trial's configured plant at rest and measure it: the check starts the controller where that
      // plant actually rests, and the resting noise sets the allowance when no identification recording measured it.
      const probe = restingNoiseProbeRequest(adapter.initialization(model, task, values, undefined).request);
      const pv = host.loop().signals.processVariable;
      const resting = await verifier.verify(configuredPlantRequest(host.loop(), probe), [{ id: "resting-noise", kind: "bounds",
        channel: pv.symbol, window: { start: 0, end: probe.duration_ms / 1000 }, minimum: pv.range.minimum, maximum: pv.range.maximum }]);
      if (generation !== host.state.generation) return true;
      const processNoise = measuredProcessNoise(host.loop(), model, host.state.campaigns) ?? recordedHalfRange(resting, pv.symbol);
      const restingPlant = restingOperatingModel(model, resting, connectedModelChannels(host.loop(), model).outputs.map(output => output.signal.symbol));
      const plan = adapter.initialization(restingPlant, task, values, processNoise);
      const initialization = await verifier.verify(configuredPlantRequest(host.loop(), plan.request), plan.requirements);
      if (generation !== host.state.generation) return true;
      if (initialization.projectRevision !== result.projectRevision) throw new Error("The program changed before initialization verification.");
      host.state.initialization = initialization; host.state.recordedInitialization = initialization;
    }
    host.state.verified = result; host.state.values = values;
    host.state.verifiedInputs = host.inputSignature();
    const passed = host.state.campaignPassed && (applicationMode !== "auto" || host.state.initialization?.evaluation.status === "Pass");
    const failure = passed ? undefined : host.state.campaignPassed
      ? { cause: "implementation" as const, nextAction: "review-implementation" as const }
      : verificationFailure(host.state.scenarioResults.flatMap(entry => entry.result.evaluation.requirements));
    await respond({ status: passed ? "verified" : "verification-failed", baseline: baselineResult.evaluation,
      candidate: result.evaluation, modelPrediction: { baseline: predictedBaseline.evaluation, candidate: predictedCandidate.evaluation }, scenarioResults: host.state.scenarioResults, campaignStatus: campaignStatus(campaign.map(scenario => scenario.id), host.state.scenarioResults), initialization: host.state.initialization?.evaluation, coordinates: "Configured plant with declared controller initialization",
      ...(failure ? { failure } : {}),
      reason: passed ? "Verification passed: actual ST satisfies every declared mandatory scenario requirement."
        : host.state.campaignPassed ? `Transfer verification failed: inspect the first output, mode and parameter evidence. Next: ${mainWorkflowNextActionLabel(failure!.nextAction)}.`
        : `${verificationFailureReason(host.state.scenarioResults.flatMap(entry => entry.result.evaluation.requirements))} Next: ${mainWorkflowNextActionLabel(failure!.nextAction)}.` });
  } finally {
    const owner = host.state.verifier ?? host.state.cancellationOwner;
    const attempt = owner?.snapshot(); host.retainAttempt(attempt);
    if (attempt?.status === "cancellation-unconfirmed") host.state.cancellationOwner = owner;
    else if (!host.state.cancelling) { host.state.busy = undefined; host.state.cancellationOwner = undefined; }
  } })();
  host.state.pendingVerification = verifying;
  try { await verifying; } finally { if (host.state.pendingVerification === verifying) host.state.pendingVerification = undefined; }
  return true;
}
