import { engineerControllerScenarios } from "./engineerControllerScenarios";
import { createHash } from "crypto";
import { physicalStateModel } from "./physicalModel";
import { decodeGeneralModel } from "./generalModel";
import { engineerEnvelopeScenarios } from "./engineerEnvelopeScenarios";
import { withScenarioEffects, type EngineerScenarioEffect } from "./engineerScenarioEffects";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import { engineerVerificationPlan, type EngineerRequirements } from "./engineerVerificationPlan";
import { decodeRunRequirement, type RunRequirement } from "./runValidation";
import type { RuntimeVerificationResult } from "./verificationClient";
export { validEngineerScenarios } from "./scenarioContract";
import { validEngineerScenarios } from "./scenarioContract";
export type { EngineerScenario, EngineerScenarioResult } from "./scenarioContract";
import type { EngineerScenario, EngineerScenarioResult } from "./scenarioContract";

/** Freeze and validate the entire mandatory campaign before starting any native job. */
export function engineerVerificationCampaign(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements, values: Readonly<Record<string, number>>) {
  const nominal = engineerVerificationPlan(loop, model, task, values);
  const scenarios = loop.verificationScenarios ?? [];
  if (!validEngineerScenarios(scenarios)) throw new Error("Invalid mandatory verification scenario campaign.");
  const parameters = new Set(loop.parameterBinding?.parameters.map(parameter => parameter.symbol));
  const interaction = [...independentReferenceScenarios(loop, model, task, values), ...engineerEnvelopeScenarios(loop, model, task, values), ...engineerControllerScenarios(loop, model, task, values)];
  const reserved = new Set(["target-response", ...interaction.map(scenario => scenario.id)]);
  if (scenarios.some(scenario => reserved.has(scenario.id))) throw new Error("A configured scenario duplicates an automatic target/interaction scenario.");
  return [{ id: "target-response", ...nominal, requirements: loop.controller.onOff ? nominal.requirements.filter(requirement => requirement.kind !== "dwell") : nominal.requirements }, ...interaction, ...scenarios.map(scenario => {
    let scenarioModel = scenario.model ? decodeGeneralModel(scenario.model) : model;
    if (!scenarioModel) throw new Error(`Scenario ${scenario.id} has an invalid supplied model.`);
    if (scenario.operatingOutputs) {
      if (!scenarioModel.physical) throw new Error("Declare a physical balance or supply a separate validated model for an operating-range case.");
      const revision = `sha256:${createHash("sha256").update(JSON.stringify({ model: scenarioModel, outputs: scenario.operatingOutputs })).digest("hex")}`;
      scenarioModel = physicalStateModel({ ...scenarioModel, revision }, scenarioModel.physical, scenario.operatingOutputs);
    }
    const base = scenarioModel === model ? nominal : engineerVerificationPlan(loop, scenarioModel, task, values);
    if (Object.keys(scenario.initialValues).some(symbol => parameters.has(symbol)) || scenario.events.some(event => parameters.has(event.symbol))) {
      throw new Error(`Scenario ${scenario.id} cannot overwrite frozen controller parameters.`);
    }
    if (scenario.events.some(event => event.atMs % base.request.step_ms !== 0 || event.atMs > base.request.duration_ms)) {
      throw new Error(`Scenario ${scenario.id} events must fall on a declared task sample within the run.`);
    }
    let request = { ...base.request, initial_values: { ...base.request.initial_values, ...scenario.initialValues },
      events: scenario.events.map(event => ({ at_ms: event.atMs, symbol: event.symbol, value: event.value })) };
    if (scenario.effects) request = { ...request, ...withScenarioEffects(request, scenarioModel, scenario.effects), events: request.events };
    // Preserve process/output limits and fixed-parameter requirements for every scenario.
    const constraints = base.requirements.filter(requirement => !["settling", "error"].includes(requirement.kind));
    const requirements = [...constraints, ...scenario.requirements];
    if (requirements.length > 64 || new Set(requirements.map(requirement => requirement.id)).size !== requirements.length) {
      throw new Error(`Scenario ${scenario.id} has duplicate or excessive mandatory requirements.`);
    }
    const contract = { channels: request.observations.map(signal => ({ id: signal.symbol, unit: signal.unit })),
      conditions: { sampleTimesSeconds: Array.from({ length: request.duration_ms / request.step_ms + 1 }, (_, index) => index * request.step_ms / 1000) } };
    requirements.forEach(requirement => decodeRunRequirement(requirement, contract));
    return { id: scenario.id, request, requirements, coordinates: base.coordinates };
  })];
}
/** Excite each controlled reference separately and retain all cross-channel requirements. */
function independentReferenceScenarios(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  values: Readonly<Record<string, number>>) {
  const channels = loop.channels;
  const controlled = channels?.outputs.filter(output => output.setpointSignalId) ?? [];
  if (!channels || controlled.length < 2) return [];
  return controlled.map(active => {
    const outputs = channels.outputs.map(output => {
      if (!output.setpointSignalId) return output;
      const required = output.requirements ?? (output.signalId === "processVariable" ? task : undefined);
      if (!required) throw new Error(`Declare interaction requirements for ${output.modelChannelId}.`);
      return { ...output, requirements: { ...required,
        referenceChange: output.modelChannelId === active.modelChannelId ? required.referenceChange : 0 } };
    });
    return { id: `independent-reference:${active.modelChannelId}`,
      ...engineerVerificationPlan({ ...loop, channels: { ...channels, outputs } }, model, task, values) };
  });
}

export function campaignStatus(expected: readonly string[], results: readonly EngineerScenarioResult[]): "Pass" | "Fail" | "Inconclusive" {
  if (results.some(entry => entry.result.evaluation.status === "Fail")) return "Fail";
  if (!expected.length || results.length !== expected.length || new Set(results.map(entry => entry.id)).size !== results.length
    || expected.some(id => !results.some(entry => entry.id === id && entry.result.evaluation.status === "Pass"))
    || new Set(results.map(entry => entry.result.projectRevision)).size !== 1) return "Inconclusive";
  return "Pass";
}
