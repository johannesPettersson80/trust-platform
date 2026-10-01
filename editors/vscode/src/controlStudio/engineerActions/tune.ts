import { resolveEngineerRequirements } from "../automaticEngineerRequirements";
import { controllerAdapter } from "../controllerAdapters";
import { assertControllerRequirements } from "../controllerChannelPreferences";
import { isControllerFamily } from "../controllerFamilies";
import { decodeControllerSettingsPins } from "../controllerSettingsPins";
import { tuneControllerAdapter } from "../controllerTuning";
import type { ControllerTuningResult } from "../controllerTuning";
import { controllerTuningPreferences } from "../controllerTuningPreferences";
import { assertEngineerModelCompatibility } from "../engineerModelCompatibility";
import { type EngineerStudy } from "../engineerStudyPersistence";
import { validEngineerScenarios } from "../engineerVerificationCampaign";
import { tuneProjectController } from "../projectController";
import { relayRuleCandidate } from "../relayCycleEvidence";
import { connectRuntimeParameters } from "../runtimeParameterConnection";
import { connectRuntimeSearch } from "../runtimeSearchConnection";
import { parseStateDesignDraft } from "../stateDesignPreferences";
import { RuntimeVerificationClient } from "../verificationClient";
import type { EngineerActionContext, EngineerActionHost } from "./host";
import { record } from "./values";

/** Tune: designs a candidate for the selected model and requirements, with its predicted response. */
export async function tuneCandidate(host: EngineerActionHost, { payload, respond, generation, applicationMode }: EngineerActionContext): Promise<boolean> {
  host.state.campaignPassed = false; host.state.verifiedInputs = undefined;
  host.state.design = undefined; host.state.values = undefined; host.state.verified = undefined; host.state.initialization = undefined; host.state.review = undefined;
  const model = host.model();
  if (!model) throw new Error("Find or supply a model first.");
  assertEngineerModelCompatibility(host.loop(), model);
  if (payload.pinnedValues !== undefined) payload.settingsPins = payload.pinnedValues === null ? null
    : { loopRevision: host.loop().revision, family: isControllerFamily(payload.family) ? payload.family : host.state.family, values: payload.pinnedValues };
  if (payload.settingsPins === null) host.state.settingsPins = undefined;
  else if (payload.settingsPins !== undefined) {
    const pin = decodeControllerSettingsPins(payload.settingsPins);
    if (!pin || pin.loopRevision !== host.loop().revision) throw new Error("Pinned settings do not match the current loop revision.");
    host.state.settingsPins = pin;
  }
  if (host.state.settingsPins) {
    if (payload.family !== undefined && payload.family !== "automatic" && payload.family !== host.state.settingsPins.family) {
      throw new Error("Clear pinned settings before changing controller family.");
    }
    payload.family = host.state.settingsPins.family;
  }
  if (payload.family === undefined) payload.family = host.state.controllerChoice;
  if (host.state.gainChoice?.pin && host.compatiblePin(host.state.gainChoice.pin) && payload.gainMode !== "automatic") {
    payload.family = "P";
    payload.gainMode = "manual";
    payload.proportionalGain ??= host.state.gainChoice.pin.value;
  }
  if ((payload.family === "automatic" || isControllerFamily(payload.family))) {
    host.state.controllerChoice = payload.family as NonNullable<EngineerStudy["controllerChoice"]>;
  }
  if (payload.scenarioDefinitions !== undefined) {
    if (JSON.stringify(payload.scenarioDefinitions).length > 256000 || !validEngineerScenarios(payload.scenarioDefinitions)) throw new Error("Invalid mandatory verification scenario policy.");
    host.state.scenarioDefinitions = structuredClone(payload.scenarioDefinitions);
  }
  if (payload.tuningPreferences !== undefined) {
    const preferences = controllerTuningPreferences(payload.tuningPreferences);
    if (!preferences) throw new Error("Choose a supported tuning method and finite controller settings.");
    host.state.tuningPreferences = preferences;
  }
  if (host.state.tuningPreferences?.method === "state-design" && host.state.tuningPreferences.stateDesignDraft) {
    host.state.tuningPreferences.stateDesign = parseStateDesignDraft(host.state.tuningPreferences.stateDesignDraft,
      String(payload.family ?? host.loop().controller.stateFeedback?.family ?? host.state.family));
  }
  const task = resolveEngineerRequirements(host.requirementLoop(), model, payload, host.state.tuningPreferences, host.state.campaigns);
  assertControllerRequirements(host.loop(), task);
  host.state.task = task;
  let tuned: ControllerTuningResult;
  if (host.state.tuningPreferences?.method === "manual") {
    tuned = tuneControllerAdapter(host.loop(), model, task, { family: String(payload.family), applicationMode,
      manualValues: host.state.tuningPreferences.values ?? {}, pinnedValues: host.state.settingsPins?.values });
  } else if (host.state.tuningPreferences?.method === "relay") {
    tuned = relayRuleCandidate(host.loop(), model, host.relayCycle(), host.state.settingsPins?.values);
  } else if (host.loop().controller.project) {
    if (payload.family !== "automatic" && !controllerAdapter(host.loop()).families.includes(payload.family as typeof host.state.family)) throw new Error("Choose the controller family declared by the current ST adapter.");
    const selectedProjectFamily = payload.family === "automatic" ? undefined : payload.family as typeof host.state.family;
    host.state.busy = "verify";
    const tuning = (async () => {
      const connection = await connectRuntimeSearch(host.loop(), { cancelled: () => generation !== host.state.generation });
      if (connection.source !== "simulation") throw new Error("Native controller tuning requires isolated simulation.");
      const parameters = await connectRuntimeParameters(host.loop());
      const observed = await parameters.client.status();
      const readiness = await connection.request("control.simulation.run.status", {});
      if (!record(readiness) || readiness.ready !== true || typeof readiness.project_revision !== "string") {
        throw new Error("The loaded source project is not ready for native controller tuning.");
      }
      if (generation !== host.state.generation) throw new Error("Controller tuning cancelled.");
      host.state.verifier = new RuntimeVerificationClient({ source: "simulation", request: connection.request }, host.retainIsolatedOperation);
      host.state.cancellationOwner = host.state.verifier;
      const design = await tuneProjectController(host.scenarioLoop(), model, task, observed.values, host.state.settingsPins?.values ?? {}, readiness.project_revision, host.state.verifier,
        () => generation !== host.state.generation, result => host.state.tuningRuns.push(result), attempt => host.retainAttempt(attempt),
        reason => respond({ status: "running", reason }), selectedProjectFamily);
      return { family: design.candidate.structure, design, values: { ...design.candidate.parameters },
        reason: design.decentralized
          ? "Designed each member's PI on the group model and checked it on the actual ST. Validate before application."
          : "Native tuning retained the best qualifying declared-grid candidate. Verify the reviewed final campaign before application." };
    })();
    host.state.pendingVerification = tuning;
    try { tuned = await tuning; }
    finally {
      host.retainAttempt(host.state.verifier?.snapshot());
      if (host.state.pendingVerification === tuning) host.state.pendingVerification = undefined;
      if (!host.state.cancelling && host.state.verifier?.snapshot()?.status !== "cancellation-unconfirmed") { host.state.busy = undefined; host.state.cancellationOwner = undefined; }
    }
    if (generation !== host.state.generation) return true;
  } else tuned = tuneControllerAdapter(host.loop(), model, task, { family: String(payload.family), applicationMode,
    gainMode: payload.gainMode, proportionalGain: payload.proportionalGain,
    pinnedValues: host.state.settingsPins?.values, stateDesign: host.state.tuningPreferences?.stateDesign },
    host.state.gainChoice?.pin && host.compatiblePin(host.state.gainChoice.pin) ? host.state.gainChoice : undefined);
  host.state.family = tuned.family;
  host.state.design = tuned.design; host.state.task = task; host.state.values = tuned.values;
  if (tuned.gainChoice) {
    host.state.gainChoice = tuned.gainChoice;
    if (tuned.gainChoice.mode === "manual") host.state.pinModel = structuredClone(model);
  }
  host.state.familyEvaluations = tuned.attempts?.map(attempt => ({ family: attempt.structure,
    status: attempt.status === "Feasible" ? "model-qualified" : "unavailable", reason: attempt.reason }));
  // Laws Studio does not calculate itself run as ST on the model (Specification 35 §9).
  const prediction = host.loop().controller.onOff || host.loop().controller.stateFeedback || host.loop().controller.project
    ? await host.programLawPrediction(model, task, tuned.values, generation, respond) : undefined;
  if (generation !== host.state.generation) return true;
  await respond({ status: "candidate", ...tuned, tuningRuns: host.state.tuningRuns, ...(prediction && "unavailable" in prediction
    ? { reason: `${tuned.reason} ${prediction.unavailable}` }
    : prediction ? { modelPrediction: { baseline: prediction.predictedBaseline.evaluation, candidate: prediction.predictedCandidate.evaluation } } : {}) });
  return true;
}
