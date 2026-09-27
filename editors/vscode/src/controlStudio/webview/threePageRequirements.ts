import type { ControlLoopDefinition } from "../loopDefinition";
import type { EngineerRequirements } from "../engineerVerificationPlan";
import { displayEngineeringUnit } from "../chartContract";
import { openCompactNumberEditor, type CompactNumberField } from "../../webview/compactEditor";
import { loopSignal } from "../loopChannels";
import { mountThreePageScenarios, type ThreePageScenarioOptions } from "./threePageScenarios";

export function editControlRequirements(loop: ControlLoopDefinition, current: Partial<EngineerRequirements>,
  commit: (next: EngineerRequirements) => void, scenarios?: ThreePageScenarioOptions): void {
  const output = loop.signals.processVariable;
  const outputs = loop.channels?.outputs.map(channel => ({ channel, signal: loopSignal(loop, channel.signalId) })) ?? [];
  const multichannel = outputs.length > 1;
  const channelFields = (modelChannelId: string, signal: typeof output, values: Partial<EngineerRequirements> | undefined): CompactNumberField[] => {
    const unit = displayEngineeringUnit(signal.unit), suffix = multichannel ? `:${modelChannelId}` : "";
    const label = (key: string, text: string) => current.automaticCriteria?.fields.includes(`${key}${suffix}`) ? `${text} · Automatic` : text;
    const value = (key: keyof EngineerRequirements) => typeof values?.[key] === "number" ? values[key] as number : undefined;
    return [
      { key: `referenceChange${suffix}`, label: label("referenceChange", `Setpoint change (${unit})`), signal: signal.symbol, value: value("referenceChange") },
      { key: `maximumError${suffix}`, label: label("maximumError", `Maximum steady-state error (${unit})`), signal: signal.symbol, value: value("maximumError"), minimum: 0 },
      { key: `settlingTimeSeconds${suffix}`, label: label("settlingTimeSeconds", "Maximum settling time (s)"), signal: signal.symbol, value: value("settlingTimeSeconds"), minimum: 0 },
      { key: `settlingBand${suffix}`, label: label("settlingBand", `Settling band (±${unit})`), signal: signal.symbol, value: value("settlingBand"), minimum: 0 },
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
  fields.push({ key: "durationSeconds", label: `Validation duration (s)${current.automaticCriteria?.fields.includes("durationSeconds") ? " · Automatic" : ""}`, value: current.durationSeconds, minimum: 0, maximum: 600 });
  if (current.constantInputDisturbance !== undefined) fields.push({ key: "constantInputDisturbance", label: `Input disturbance (${displayEngineeringUnit(loop.signals.manipulatedVariable.unit)})`, value: current.constantInputDisturbance });
  if (current.measurementNoiseBand !== undefined) fields.push({ key: "measurementNoiseBand", label: `Measurement noise (±${unit})`, value: current.measurementNoiseBand, minimum: 0 });
  if (current.gainUncertaintyFraction !== undefined) fields.push({ key: "gainUncertaintyFraction", label: "Gain uncertainty (0–1)", value: current.gainUncertaintyFraction, minimum: 0, maximum: 1 });
  const validate = (values: Readonly<Record<string, number>>) => {
    const suffixes = multichannel ? outputs.map(item => `:${item.channel.modelChannelId}`) : [""];
    let slowest = 0;
    for (const suffix of suffixes) {
      if (!values[`referenceChange${suffix}`]) return { field: `referenceChange${suffix}`, message: "Enter a nonzero setpoint change." };
      if (values[`settlingBand${suffix}`] <= 0) return { field: `settlingBand${suffix}`, message: "Enter a positive settling band." };
      if (values[`maximumError${suffix}`] <= values[`settlingBand${suffix}`]) return { field: `maximumError${suffix}`, message: "The error limit must be greater than the settling band." };
      if (values[`settlingTimeSeconds${suffix}`] <= 0) return { field: `settlingTimeSeconds${suffix}`, message: "Enter a positive settling time." };
      slowest = Math.max(slowest, values[`settlingTimeSeconds${suffix}`]);
    }
    return values.durationSeconds <= slowest ? { field: "durationSeconds", message: "Allow longer than every settling time." } : undefined;
  };
  let scenarioEditor: ReturnType<typeof mountThreePageScenarios> | undefined;
  openCompactNumberEditor({ title: "Control requirements", fields, fieldAttribute: "data-studio-requirement", rowLayout: "requirements",
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
      if (!multichannel) return commit({ ...current, ...values, ...origin, ...scenarioPayload } as EngineerRequirements);
      const channelRequirements = Object.fromEntries(outputs.map(item => {
        const suffix = `:${item.channel.modelChannelId}`;
        return [item.channel.modelChannelId, { referenceChange: values[`referenceChange${suffix}`], maximumError: values[`maximumError${suffix}`], settlingBand: values[`settlingBand${suffix}`], settlingTimeSeconds: values[`settlingTimeSeconds${suffix}`] }];
      }));
      const first = channelRequirements[outputs[0].channel.modelChannelId];
      commit({ ...current, ...values, ...first, channelRequirements, ...origin, ...scenarioPayload } as EngineerRequirements);
    } });
  if (scenarios) {
    const dialog = document.querySelector<HTMLElement>("[data-trust-compact-editor]");
    if (dialog) scenarioEditor = mountThreePageScenarios(dialog, scenarios);
  }
}
