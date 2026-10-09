import type { ControlLoopDefinition } from "../loopDefinition";
import type { EngineerRequirements } from "../engineerVerificationPlan";
import { displayEngineeringUnit } from "../chartContract";
import { openCompactNumberEditor, type CompactEditorIssue, type CompactNumberField } from "../../webview/compactEditor";
import { loopSignal } from "../loopChannels";
import { mountThreePageScenarios, type ThreePageScenarioOptions } from "./threePageScenarios";
import { MAXIMUM_EXPERIMENT_DURATION_MS, MINIMUM_EXPERIMENT_DURATION_MS } from "../controlStudioLimits";
import { signalRoleLabel } from "./signalRoles";

/** The test design that sits under More…; an automatic value in any of these marks the section Automatic. */
const MORE_KEYS = ["referenceChange", "durationSeconds", "transferTolerance", "measurementNoiseBand", "gainUncertaintyFraction"] as const;

/** The largest declared overshoot limit, in percent of the setpoint change. This editor keeps the planner's limit without importing the planner. */
const MAXIMUM_OVERSHOOT_PERCENT = 1000;

/** Done saves the requirements, then runs `resumeTune`; only the editor that a paused Tune opened receives it. Cancel changes nothing. */
export function editControlRequirements(loop: ControlLoopDefinition, current: Partial<EngineerRequirements>,
  commit: (next: EngineerRequirements) => void, scenarios?: ThreePageScenarioOptions, resumeTune?: () => void): void {
  const output = loop.signals.processVariable;
  // Requirements belong to controlled outputs, those with a setpoint; an auxiliary measured output has none.
  const channels = loop.channels?.outputs.map(channel => ({ channel, signal: loopSignal(loop, channel.signalId) })) ?? [];
  const controlled = channels.filter(item => item.channel.setpointSignalId);
  const outputs = controlled.length ? controlled : channels;
  const multichannel = outputs.length > 1;
  const channelFields = (modelChannelId: string, signal: typeof output, values: Partial<EngineerRequirements> | undefined): CompactNumberField[] => {
    const unit = displayEngineeringUnit(signal.unit), suffix = multichannel ? `:${modelChannelId}` : "";
    const label = (key: string, text: string) => current.automaticCriteria?.fields.includes(`${key}${suffix}`) ? `${text} · Automatic` : text;
    const value = (key: keyof EngineerRequirements) => typeof values?.[key] === "number" ? values[key] as number : undefined;
    const role = signalRoleLabel(loop, signal.symbol, signal.unit);
    return [
      { key: `settlingTimeSeconds${suffix}`, label: label("settlingTimeSeconds", "Maximum settling time"), signal: signal.symbol, signalLabel: role, value: value("settlingTimeSeconds"), minimum: 0, duration: true },
      { key: `settlingBand${suffix}`, label: label("settlingBand", `Settling band (±${unit})`), signal: signal.symbol, signalLabel: role, value: value("settlingBand"), minimum: 0 },
      // Overshoot is never generated: a blank row declares no limit, so it is not labelled Automatic.
      { key: `maximumOvershootPercent${suffix}`, label: "Maximum overshoot (% of setpoint change)", signal: signal.symbol, signalLabel: role,
        value: value("maximumOvershootPercent"), minimum: 0, maximum: MAXIMUM_OVERSHOOT_PERCENT, optional: true, placeholder: "None" },
      { key: `maximumError${suffix}`, label: label("maximumError", `Maximum steady-state error (${unit})`), signal: signal.symbol, signalLabel: role, value: value("maximumError"), minimum: 0 },
      { key: `referenceChange${suffix}`, label: label("referenceChange", `Test setpoint change (${unit})`), signal: signal.symbol, signalLabel: role, value: value("referenceChange"), more: true },
    ];
  };
  const declared = (index: number): Partial<EngineerRequirements> | undefined => {
    const item = outputs[index];
    if (!item) return current;
    const channel = current.channelRequirements?.[item.channel.modelChannelId] ?? item.channel.requirements;
    if (channel) return channel;
    return index === 0 ? current : undefined;
  };
  const fields: CompactNumberField[] = multichannel
    ? outputs.flatMap((item, index) => channelFields(item.channel.modelChannelId, item.signal, declared(index)))
    : channelFields("", output, current);
  const unit = displayEngineeringUnit(output.unit);
  // A constant load added to the plant input in its own trial with the setpoint unchanged; blank declares none.
  const command = loop.signals.manipulatedVariable;
  const commandRole = signalRoleLabel(loop, command.symbol, command.unit);
  fields.push({ key: "constantInputDisturbance", label: `Input disturbance (${displayEngineeringUnit(command.unit)})`, signal: command.symbol, signalLabel: commandRole,
    value: current.constantInputDisturbance, optional: true, placeholder: "None" });
  fields.push({ key: "durationSeconds", label: `Validation duration${current.automaticCriteria?.fields.includes("durationSeconds") ? " · Automatic" : ""}`, value: current.durationSeconds,
    minimum: MINIMUM_EXPERIMENT_DURATION_MS / 1_000, maximum: MAXIMUM_EXPERIMENT_DURATION_MS / 1_000, duration: true, more: true });
  // A declared measurement filter is validated against noise, so the loop needs this band from the user.
  const filtered = loop.controller.project?.components?.some(component => component.kind === "low-pass" || component.kind === "notch") ?? false;
  if (current.measurementNoiseBand !== undefined || filtered) fields.push({ key: "measurementNoiseBand", label: `Measurement noise (±${unit})`, value: current.measurementNoiseBand, minimum: 0, more: true });
  if (current.gainUncertaintyFraction !== undefined) fields.push({ key: "gainUncertaintyFraction", label: "Gain uncertainty (0–1)", value: current.gainUncertaintyFraction, minimum: 0, maximum: 1, more: true });
  // Auto commissioning checks the controller's first Auto output against this tolerance; blank uses the automatic allowance.
  fields.push({ key: "transferTolerance", label: `Auto-entry tolerance (${displayEngineeringUnit(command.unit)})`, signal: command.symbol, signalLabel: commandRole,
    value: current.transferTolerance, minimum: 0, maximum: loop.limits.outputMaximum - loop.limits.outputMinimum, optional: true, placeholder: "Automatic", more: true });
  const moreAutomatic = current.automaticCriteria?.fields.some(key => (MORE_KEYS as readonly string[]).some(name => key === name || key.startsWith(`${name}:`))) ?? false;
  const validate = (values: Readonly<Record<string, number>>): CompactEditorIssue[] => {
    const suffixes = multichannel ? outputs.map(item => `:${item.channel.modelChannelId}`) : [""];
    const issues: CompactEditorIssue[] = [];
    let slowest = 0;
    for (const suffix of suffixes) {
      if (!values[`referenceChange${suffix}`]) issues.push({ field: `referenceChange${suffix}`, message: "Enter a nonzero setpoint change." });
      if (values[`settlingBand${suffix}`] <= 0) issues.push({ field: `settlingBand${suffix}`, message: "Enter a positive settling band." });
      if (values[`maximumError${suffix}`] <= values[`settlingBand${suffix}`]) issues.push({ field: `maximumError${suffix}`, message: "The error limit must be greater than the settling band." });
      if (values[`settlingTimeSeconds${suffix}`] <= 0) issues.push({ field: `settlingTimeSeconds${suffix}`, message: "Enter a positive settling time." });
      // The field range check keeps negative values; zero is a number, but it cannot be a limit, so it is refused here.
      if (values[`maximumOvershootPercent${suffix}`] !== undefined && values[`maximumOvershootPercent${suffix}`] <= 0) {
        issues.push({ field: `maximumOvershootPercent${suffix}`, message: "Enter an overshoot limit above 0 %, or leave it blank for no limit." });
      }
      slowest = Math.max(slowest, values[`settlingTimeSeconds${suffix}`]);
    }
    if (values.durationSeconds <= slowest) issues.push({ field: "durationSeconds", message: "Allow longer than every settling time." });
    return issues;
  };
  const done = (next: EngineerRequirements): void => { commit(next); resumeTune?.(); };
  let scenarioEditor: ReturnType<typeof mountThreePageScenarios> | undefined;
  openCompactNumberEditor({ title: "Control requirements", fields, fieldAttribute: "data-studio-requirement", rowLayout: "requirements",
    showSignals: multichannel, moreLabel: `More…${moreAutomatic || current.automaticCriteria?.fields.includes("referenceChange") ? " · Automatic" : ""}`,
    validate,
    beforeCommit: () => scenarioEditor?.validate(),
    commit: values => {
      const generated = current.automaticCriteria?.fields.filter(key => {
        const separator = key.indexOf(":");
        const name = separator < 0 ? key : key.slice(0, separator), channel = separator < 0 ? undefined : key.slice(separator + 1);
        const before = channel ? current.channelRequirements?.[channel]?.[name as keyof import("../loopChannels").ChannelRequirements] : current[name as keyof EngineerRequirements];
        return values[key] === undefined || values[key] === before;
      });
      const origin = current.automaticCriteria ? { automaticCriteria: { ...current.automaticCriteria, fields: generated! } } : {};
      const scenarioValues = scenarioEditor?.read();
      const scenarioPayload = scenarioValues ? { scenarioDefinitions: scenarioValues } : {};
      const reviewed: Partial<EngineerRequirements> = { ...current };
      if (values.transferTolerance === undefined) delete reviewed.transferTolerance;
      // A blank row removes the declared limit. For several outputs the first output's limit is spread below, as its other values are.
      if (values.maximumOvershootPercent === undefined) delete reviewed.maximumOvershootPercent;
      if (values.constantInputDisturbance === undefined) delete reviewed.constantInputDisturbance;
      if (!multichannel) return done({ ...reviewed, ...values, ...origin, ...scenarioPayload } as EngineerRequirements);
      const channelRequirements = Object.fromEntries(outputs.map(item => {
        const suffix = `:${item.channel.modelChannelId}`;
        const overshoot = values[`maximumOvershootPercent${suffix}`];
        return [item.channel.modelChannelId, { referenceChange: values[`referenceChange${suffix}`], maximumError: values[`maximumError${suffix}`], settlingBand: values[`settlingBand${suffix}`], settlingTimeSeconds: values[`settlingTimeSeconds${suffix}`],
          ...(overshoot === undefined ? {} : { maximumOvershootPercent: overshoot }) }];
      }));
      const first = channelRequirements[outputs[0].channel.modelChannelId];
      done({ ...reviewed, ...values, ...first, channelRequirements, ...origin, ...scenarioPayload } as EngineerRequirements);
    } });
  if (scenarios) {
    const dialog = document.querySelector<HTMLElement>("[data-trust-compact-editor]");
    if (dialog) scenarioEditor = mountThreePageScenarios(dialog, scenarios);
  }
}
