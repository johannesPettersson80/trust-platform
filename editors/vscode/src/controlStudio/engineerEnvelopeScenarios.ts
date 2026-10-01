import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import { engineerVerificationPlan, type EngineerRequirements } from "./engineerVerificationPlan";
import { connectedModelChannels } from "./loopChannels";
import { withScenarioEffects, type EngineerScenarioEffect } from "./engineerScenarioEffects";

/** Nonzero reviewed envelope bounds create mandatory cases using the existing native generators. */
export function engineerEnvelopeScenarios(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  values: Readonly<Record<string, number>>) {
  const channels = connectedModelChannels(loop, model);
  const input = channels.inputs.findIndex(channel => channel.signal.symbol === loop.signals.manipulatedVariable.symbol);
  const output = channels.outputs.findIndex(channel => channel.signal.symbol === loop.signals.processVariable.symbol);
  const nominal = engineerVerificationPlan(loop, model, task, values);
  const cases: Array<{ id: string; request: typeof nominal.request; requirements: typeof nominal.requirements; coordinates: string }> = [];
  const add = (id: string, effects: EngineerScenarioEffect[], plan = nominal) => cases.push({ id, ...plan, request: withScenarioEffects(plan.request, model, effects) });
  const components = loop.controller.project?.components ?? [];
  if (components.some(component => component.kind === "low-pass" || component.kind === "notch")
    && (task.measurementNoiseBand === undefined || task.measurementNoiseBand <= 0)) {
    throw new Error("A declared measurement filter requires a nonzero reviewed measurement-noise band so target and noise response are both verified.");
  }
  if (components.some(component => component.kind === "reference-weighting")
    && (task.constantInputDisturbance === undefined || task.constantInputDisturbance === 0)) {
    throw new Error("Declared reference shaping requires a nonzero reviewed input disturbance so target and load response are both verified.");
  }
  if (task.constantInputDisturbance !== undefined && task.constantInputDisturbance !== 0) {
    if (input < 0) throw new Error("A reviewed load disturbance requires a bound process-input channel.");
    const regulationLoop = loop.channels ? { ...loop, channels: { ...loop.channels, outputs: loop.channels.outputs.map(channel => ({
      ...channel, ...(channel.requirements ? { requirements: { ...channel.requirements, referenceChange: 0 } } : {}),
    })) } } : loop;
    const regulation = engineerVerificationPlan(regulationLoop, model, { ...task, referenceChange: 0 }, values);
    add("load-rejection", [{ id: "reviewed-load", target: "process_input", channel: input, mode: "add", startSeconds: 0,
      generator: "constant", value: task.constantInputDisturbance }], regulation);
  }
  if (task.measurementNoiseBand !== undefined && task.measurementNoiseBand !== 0) {
    if (output < 0) throw new Error("A reviewed measurement-noise case requires a bound process-output channel.");
    add("measurement-noise", [{ id: "reviewed-noise", target: "measurement", channel: output,
    mode: "add", startSeconds: 0, generator: "uniform_white", minimum: -task.measurementNoiseBand, maximum: task.measurementNoiseBand }]);
  }
  if (task.gainUncertaintyFraction !== undefined && task.gainUncertaintyFraction !== 0) for (const direction of [-1, 1]) add(direction < 0 ? "lower-gain" : "upper-gain",
    channels.inputs.map((_, channel) => ({ id: `gain-${channel}`, target: "process_input", channel, mode: "multiply", startSeconds: 0,
      generator: "constant", value: 1 + direction * task.gainUncertaintyFraction! })));
  const project = loop.controller.project;
  if (project?.family === "MPC" || project?.components?.some(component => component.kind === "back-calculation")
    || loop.controller.antiWindupVerification !== undefined) {
    const hold = Math.ceil(task.settlingTimeSeconds * 1000 / nominal.request.step_ms) * nominal.request.step_ms / 1000;
    const recovery = engineerVerificationPlan(loop, model, { ...task, settlingTimeSeconds: 2 * hold,
      durationSeconds: task.durationSeconds + hold }, values);
    add("constraint-recovery", channels.inputs.map((_, channel) => ({ id: `held-actuator-${channel}`, target: "command", channel,
      mode: "replace", startSeconds: 0, endSeconds: hold, generator: "constant", value: 0 })), recovery);
  }
  return cases;
}
