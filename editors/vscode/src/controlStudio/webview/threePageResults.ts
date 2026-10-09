import type { EngineerRequirements } from "../engineerVerificationPlan";
import { appendModelValidationRows } from "./modelValidationRows";
import type { IndependentModelValidation } from "../independentModelValidation";
import type { ControlLoopDefinition } from "../loopDefinition";
import { groupSettingSymbols, ownedParameterLabel, PID_GAIN_LABELS } from "../controllerGroupPresentation";
import type { RunEvaluation, RunRequirement, RunRequirementResult } from "../runValidation";
import { displayEngineeringUnit, formatEngineeringNumber } from "../chartContract";
import { formatMatrixRows, formatNumberList } from "./numberListText";
import type { CandidateRobustness, Margin } from "../candidateRobustness";
import { familyLabel } from "../controllerFamilies";
import type { FeedforwardDesign } from "../feedforwardPidDesign";
import { signalRoleLabel } from "./signalRoles";

export function renderTunedController(root: HTMLElement, loop: ControlLoopDefinition, payload: Record<string, unknown>): void {
  if (payload.status !== "candidate" || !payload.values || typeof payload.values !== "object") return;
  const values = payload.values as Record<string, number>;
  const area = root.querySelector<HTMLElement>("[data-studio-tuning-result]")!;
  const family = String(payload.family ?? loop.controller.type);
  const title = document.createElement("p"); title.textContent = `${familyLabel(family)} controller · ${loop.controller.type}`;
  const list = document.createElement("dl"); list.className = "studio-setting-values";
  const law = loop.controller.law;
  const groupSymbols = groupSettingSymbols(loop);
  const primary = law?.kind === "pid" ? new Map([
    [law.proportionalGainSymbol, PID_GAIN_LABELS.ProportionalGain],
    ...(["PI", "PID"].includes(family) ? [[law.integralGainSymbol, PID_GAIN_LABELS.IntegralGain]] : []),
    ...(["PD", "PID"].includes(family) ? [[law.derivativeGainSymbol, PID_GAIN_LABELS.DerivativeGain]] : []),
  ] as [string, string][]) : law?.kind === "proportional" ? new Map([[law.gainSymbol, PID_GAIN_LABELS.ProportionalGain]]) : seededGainLabels(loop);
  // A Keep toggle keeps the calculated value fixed for the next tuning run; it uses the pins that Tune already sends.
  const pinned = (payload.settingsPins as { values?: Record<string, number> } | undefined)?.values ?? {};
  for (const parameter of loop.parameterBinding?.parameters ?? []) {
    if (!Number.isFinite(values[parameter.symbol]) || primary && !primary.has(parameter.symbol) || groupSymbols && !groupSymbols.has(parameter.symbol)) continue;
    const label = document.createElement("dt"); label.textContent = ownedParameterLabel(loop, parameter.symbol, primary?.get(parameter.symbol) ?? readable(parameter.convention || parameter.symbol));
    const value = document.createElement("dd"); value.textContent = `${formatEngineeringNumber(values[parameter.symbol])}${unitSuffix(parameter.unit)}`;
    value.dataset.studioParameter = parameter.symbol;
    const keepLabel = document.createElement("label"); keepLabel.className = "studio-keep"; keepLabel.title = "Keep this value fixed for the next tuning run";
    const keep = document.createElement("input"); keep.type = "checkbox"; keep.dataset.studioKeep = parameter.symbol; keep.checked = Number.isFinite(pinned[parameter.symbol]);
    keepLabel.append(keep, " Keep");
    const row = document.createElement("div"); row.append(label, value, keepLabel);
    list.append(row);
  }
  area.replaceChildren(title, list);
  const margins = payload.robustness as CandidateRobustness | undefined;
  if (margins) {
    const text = (value: Margin, unit: string) => value === "unbounded" ? "unbounded" : `${formatEngineeringNumber(value)}${unit}`;
    const line = document.createElement("p"); line.dataset.studioRobustness = "";
    line.textContent = `Gain margin ${text(margins.gainMarginDb, " dB")} · phase margin ${text(margins.phaseMarginDegrees, "°")}`
      + ` · delay margin ${text(margins.delayMarginSeconds, " s")} · peak sensitivity Ms ${formatEngineeringNumber(margins.peakSensitivity)}`;
    area.append(line);
  }
  const design = payload.design as { horizon?: { steps: number; policy: string; gainSequence: number[][][]; riccatiSequence: number[][][] };
    decentralized?: { members: { inputId: string; outputId: string; pathGain: number; relativeGain: number; effectiveGain: number; timeConstantSeconds: number; delaySeconds: number }[] };
    feedforward?: FeedforwardDesign; decoupler?: { matrix: number[][]; inputIds: string[]; memberIds: string[] } } | undefined;
  if (design?.decentralized?.members.length) area.append(decentralizedTable(loop, design.decentralized.members));
  if (design?.feedforward) area.append(...feedforwardDesignView(loop, design.feedforward));
  if (design?.decoupler) area.append(decouplerTable(design.decoupler));
  if (design?.horizon) {
    const horizon = design.horizon;
    const policy = document.createElement("p"); policy.dataset.studioDesignPolicy = "finite";
    const pinned = horizon.policy === "pinned-fixed-gain";
    policy.textContent = `Finite horizon: ${horizon.steps} ${horizon.steps === 1 ? "step" : "steps"} · ${pinned ? "Pinned fixed gain" : "Repeated first gain"}`;
    const details = document.createElement("details"); const summary = document.createElement("summary"); summary.textContent = pinned ? "Nominal Riccati stages" : "Riccati stages";
    details.append(summary);
    const table = document.createElement("table"); table.className = "trust-table";
    const header = table.createTHead().insertRow();
    for (const name of ["Stage", "Gain K", "Cost P"]) { const th = document.createElement("th"); th.textContent = name; header.append(th); }
    const body = table.createTBody();
    const matrix = (value: number[][] | undefined) => value ? formatMatrixRows(value) : "—";
    horizon.riccatiSequence.forEach((cost, stage) => { const row = body.insertRow(); row.dataset.studioRiccatiStage = String(stage);
      for (const value of [String(stage), matrix(horizon.gainSequence[stage]), matrix(cost)]) row.insertCell().textContent = value;
    });
    details.append(table); area.append(policy, details);
  }
  area.hidden = false;
}

/** A program's own P/PI/PD/PID law names its gains as Control.PID does (Specification 35 §17.1); a cascade names each
 * controller's proportional and integral gain after its instance (§17.11). */
export function seededGainLabels(loop: ControlLoopDefinition): Map<string, string> | undefined {
  const seed = loop.controller.project?.seed;
  if (seed?.kind === "cascade-pid") return new Map([seed.inner, seed.outer].flatMap(gains => [
    [gains.proportionalSymbol, `${gains.instance} · ${PID_GAIN_LABELS.ProportionalGain}`],
    [gains.integralSymbol, `${gains.instance} · ${PID_GAIN_LABELS.IntegralGain}`]]));
  if (seed?.kind === "feedforward-pid") return new Map([[seed.proportionalSymbol, PID_GAIN_LABELS.ProportionalGain],
    [seed.integralSymbol, PID_GAIN_LABELS.IntegralGain], [seed.gainSymbol, "Feedforward gain"],
    ...(seed.leadSymbol && seed.lagSymbol ? [[seed.leadSymbol, "Lead time"], [seed.lagSymbol, "Lag time"]] : [])] as [string, string][]);
  if (seed?.kind === "smith-pid") return new Map([[seed.proportionalSymbol, PID_GAIN_LABELS.ProportionalGain],
    [seed.integralSymbol, PID_GAIN_LABELS.IntegralGain], [seed.gainSymbol, "Model gain K"], [seed.timeConstantSymbol, "Model time constant T"],
    [seed.delaySymbol, "Model delay θ"]]);
  if (seed?.kind === "trajectory-feedforward-pid") return new Map([[seed.proportionalSymbol, PID_GAIN_LABELS.ProportionalGain],
    [seed.integralSymbol, PID_GAIN_LABELS.IntegralGain], [seed.derivativeSymbol, PID_GAIN_LABELS.DerivativeGain],
    [seed.rateSymbol, "Rate feedforward (Kv)"], [seed.accelerationSymbol, "Acceleration feedforward (Ka)"]]);
  if (seed?.kind !== "parallel-pid") return undefined;
  return new Map([[seed.proportionalSymbol, PID_GAIN_LABELS.ProportionalGain],
    ...(seed.integralSymbol ? [[seed.integralSymbol, PID_GAIN_LABELS.IntegralGain]] : []),
    ...(seed.derivativeSymbol ? [[seed.derivativeSymbol, PID_GAIN_LABELS.DerivativeGain]] : [])] as [string, string][]);
}

/** Specification 35 §17.15: the decoupler D, one row per plant input and one column per member. */
function decouplerTable(decoupler: { matrix: number[][]; inputIds: string[]; memberIds: string[] }): HTMLTableElement {
  const table = document.createElement("table"); table.className = "trust-table"; table.dataset.studioDecoupler = "";
  const header = table.createTHead().insertRow();
  for (const name of ["Decoupler D: plant input", ...decoupler.memberIds.map(member => `from ${member}`)]) { const th = document.createElement("th"); th.textContent = name; header.append(th); }
  const body = table.createTBody();
  decoupler.matrix.forEach((row, index) => {
    const cells = body.insertRow();
    for (const text of [decoupler.inputIds[index], ...row.map(value => formatEngineeringNumber(value))]) cells.insertCell().textContent = text;
  });
  return table;
}

/** Specification 35 §17.12: both paths to the measurement and the predicted largest deviations with the PI alone and with feedforward. */
function feedforwardDesignView(loop: ControlLoopDefinition, design: FeedforwardDesign): HTMLElement[] {
  const measurement = loop.signals.processVariable, output = loop.signals.manipulatedVariable;
  const disturbance = loop.signals.additional?.find(signal => signal.role === "disturbance");
  const table = document.createElement("table"); table.className = "trust-table"; table.dataset.studioFeedforwardDesign = "";
  const header = table.createTHead().insertRow();
  for (const name of [`Path to ${measurement.symbol}`, "Gain K", "Time constant τ", "Delay θ"]) { const th = document.createElement("th"); th.textContent = name; header.append(th); }
  const body = table.createTBody();
  for (const [label, input, path] of [[`Controller output ${output.symbol}`, output.unit, design.outputPath],
    [`Disturbance ${disturbance?.symbol ?? ""}`, disturbance?.unit ?? "1", design.disturbancePath]] as const) {
    const row = body.insertRow();
    for (const text of [label, `${formatEngineeringNumber(path.gain)}${unitSuffix(measurement.unit === input ? "1" : `${measurement.unit}/${input}`)}`,
      `${formatEngineeringNumber(path.timeConstantSeconds)} s`, `${formatEngineeringNumber(path.delaySeconds)} s`]) row.insertCell().textContent = text;
  }
  const deviation = document.createElement("p"); deviation.dataset.studioFeedforwardDeviation = "";
  const unit = unitSuffix(measurement.unit);
  deviation.textContent = `Largest deviation after a ${formatEngineeringNumber(design.disturbanceStep)}${unitSuffix(disturbance?.unit ?? "1")} disturbance step, on the model: `
    + `${formatEngineeringNumber(design.largestDeviation.feedback)}${unit} with the PI alone, ${formatEngineeringNumber(design.largestDeviation.feedforward)}${unit} with feedforward`;
  return [table, deviation];
}

/** Specification 35 §17.9: each member's own path, its relative gain and the gain it has with the other loops closed. */
function decentralizedTable(loop: ControlLoopDefinition, members: readonly { inputId: string; outputId: string; pathGain: number; relativeGain: number;
  effectiveGain: number; timeConstantSeconds: number; delaySeconds: number }[]): HTMLTableElement {
  const table = document.createElement("table"); table.className = "trust-table"; table.dataset.studioDecentralizedDesign = "";
  const header = table.createTHead().insertRow();
  for (const name of ["Controller", "Own path", "Relative gain λ", "Gain with the other loops closed"]) { const th = document.createElement("th"); th.textContent = name; header.append(th); }
  const body = table.createTBody();
  for (const member of members) {
    const owner = loop.controller.group?.members.find(entry => entry.inputChannelId === member.inputId && entry.outputChannelId === member.outputId);
    const row = body.insertRow(); row.dataset.studioDecentralizedMember = owner?.instance ?? member.inputId;
    // A path gain is in the member's output unit per input unit, as is its gain with the other loops closed.
    const output = channelUnit(loop, "outputs", member.outputId), input = channelUnit(loop, "inputs", member.inputId);
    const unit = output && input ? unitSuffix(output === input ? "1" : `${output}/${input}`) : "";
    for (const text of [owner?.instance ?? `${member.inputId} → ${member.outputId}`,
      `K = ${formatEngineeringNumber(member.pathGain)}${unit}, τ = ${formatEngineeringNumber(member.timeConstantSeconds)} s, θ = ${formatEngineeringNumber(member.delaySeconds)} s`,
      formatEngineeringNumber(member.relativeGain), `${formatEngineeringNumber(member.effectiveGain)}${unit}`]) row.insertCell().textContent = text;
  }
  return table;
}

/** A reader's name for a validation scenario: a group's one-setpoint run is named after the setpoint it changes. */
function scenarioName(name: string, loop: ControlLoopDefinition | null): string {
  const alone = /^independent-reference:(.+)$/.exec(name);
  if (!alone) return readable(name);
  const signalId = loop?.channels?.outputs.find(output => output.modelChannelId === alone[1])?.signalId;
  const signal = signalId === "processVariable" ? loop?.signals.processVariable : loop?.signals.additional?.find(entry => entry.id === signalId);
  return `${signal?.symbol ?? alone[1]} setpoint alone`;
}

function channelUnit(loop: ControlLoopDefinition, kind: "inputs" | "outputs", id: string): string {
  const signalId = loop.channels?.[kind].find(channel => channel.modelChannelId === id)?.signalId;
  const signal = signalId === "manipulatedVariable" ? loop.signals.manipulatedVariable : signalId === "processVariable" ? loop.signals.processVariable
    : loop.signals.additional?.find(entry => entry.id === signalId);
  return signal?.unit ?? "";
}

/** A channel's role in the loop with its unit, for example "Measurement (°C)"; the symbol stays the row's hover text. */
function channelRoleText(loop: ControlLoopDefinition | null, symbol: string): string {
  const signal = [loop?.signals.processVariable, loop?.signals.manipulatedVariable, loop?.signals.setpoint, ...(loop?.signals.additional ?? [])]
    .find(candidate => candidate?.symbol === symbol);
  return signalRoleLabel(loop, symbol, signal?.unit ?? "");
}

/** A verdict cell keeps its word as the text; the icon and colour come from the verdict attribute in the page styles. */
function verdictCell(text: string): HTMLTableCellElement {
  const cell = document.createElement("td"); cell.textContent = text;
  const verdict = /^(Pass|Fail|Inconclusive)\b/.exec(text)?.[1]?.toLowerCase();
  if (verdict) { cell.classList.add("studio-verdict"); cell.dataset.verdict = verdict; }
  return cell;
}

export function renderValidationResults(root: HTMLElement, payload: Record<string, unknown>): void {
  if (["edited", "model-ready", "candidate"].includes(String(payload.status))) {
    root.querySelector<HTMLTableElement>("[data-studio-validation-result]")!.hidden = true; return;
  }
  if (!["verified", "verification-failed", "restored", "failed", "cancelled"].includes(String(payload.status))) return;
  if (!payload.scenarioResults && !payload.candidate && !payload.requiredScenarios && !payload.modelValidation) return;
  // An Apply outcome reports its own result in the message; it never replaces the validation evidence it did not rerun.
  if (payload.commissioning && !payload.scenarioResults) return;
  const scenarios = payload.scenarioResults as { id: string; result: { evaluation: RunEvaluation } }[] | undefined;
  const evaluations = scenarios?.length ? scenarios.map(scenario => ({ name: scenario.id, evaluation: scenario.result.evaluation }))
    : payload.candidate ? [{ name: "", evaluation: payload.candidate as RunEvaluation }] : [];
  if (payload.initialization && typeof payload.initialization === "object") evaluations.push({ name: "Controller transfer", evaluation: payload.initialization as RunEvaluation });
  const table = root.querySelector<HTMLTableElement>("[data-studio-validation-result]")!;
  const body = table.tBodies[0]; body.replaceChildren();
  let passedChecks = 0;
  for (const { name, evaluation } of evaluations) {
    if (!evaluation.requirements || !evaluation.specification) continue;
    for (const result of evaluation.requirements) {
      const requirement = evaluation.specification.find(item => item.id === result.id);
      const fixedParameter = result.id.startsWith("fixed-parameter:");
      // Generated range, rate, controller-state and unchanged-parameter guards and the transfer run share one row
      // when they pass; requirements declared by the task or its scenarios always keep their own rows.
      const generatedCheck = fixedParameter || result.id.startsWith("controller-state:") || GENERATED_CHECK.test(result.id);
      if (result.status === "Pass" && (generatedCheck || name === "Controller transfer")) { passedChecks++; continue; }
      const row = document.createElement("tr"); row.dataset.studioRequirement = result.id;
      const loop = JSON.parse(root.dataset.studioLoop ?? "null") as ControlLoopDefinition | null;
      const label = fixedParameter ? "Parameter unchanged" : requirement?.kind === "rate" && requirement.direction
        ? requirement.direction === "up" ? "Maximum increase rate" : "Maximum decrease rate"
        : requirement?.kind === "dwell" && requirement.state ? requirement.state === "high" ? "Minimum ON time" : "Minimum OFF time"
        : errorFromReference(result.kind, requirement, loop) ?? names[result.kind] ?? readable(result.kind);
      const channel = requirement?.channel && (fixedParameter || result.kind === "state-bounds" ? readable(requirement.channel.split(".").pop()!) : channelRoleText(loop, requirement.channel));
      const criteria = (payload.task as EngineerRequirements | undefined)?.automaticCriteria;
      const field = result.kind === "error" ? "maximumError" : result.kind === "settling" ? "settlingTimeSeconds" : undefined;
      const output = loop?.channels?.outputs.find(output => {
        const signal = output.signalId === "processVariable" ? loop.signals.processVariable : loop.signals.additional?.find(signal => signal.id === output.signalId);
        return signal?.symbol === requirement?.channel;
      });
      const automatic = field && criteria?.fields.includes(loop?.channels && loop.channels.outputs.length > 1 ? `${field}:${output?.modelChannelId}` : field);
      const title = `${label}${automatic ? " · Automatic" : ""}${channel ? ` · ${channel}` : ""}${evaluations.length > 1 && name && name !== "target-response" ? ` · ${scenarioName(name, loop)}` : ""}`;
      row.title = requirement?.channel ?? "";
      row.dataset.resultPriority = String(result.status === "Fail" ? 0 : result.status === "Inconclusive" ? 1 : 2);
      row.dataset.requirementPriority = String(fixedParameter ? 7 : ({ settling: 0, overshoot: 1, error: 2, bounds: 3, rate: 4, "state-bounds": 6 } as Record<string, number>)[result.kind] ?? 5);
      const status = result.kind === "activity" && result.limit === undefined && result.status === "Pass" ? "Measured" : result.status;
      for (const text of [title, measurement(result, requirement, evaluation), limit(result, requirement)]) {
        const cell = document.createElement("td"); cell.textContent = text; row.append(cell);
      }
      row.append(verdictCell(result.status !== "Pass" ? `${result.status} · ${result.reason}` : status));
      body.append(row);
    }
  }
  if (passedChecks) {
    const row = document.createElement("tr"); row.dataset.studioRequirement = "limit-and-controller-checks";
    row.dataset.resultPriority = "2"; row.dataset.requirementPriority = "8";
    row.title = "Operating ranges, output rates, controller states, unchanged parameters and the controller transfer run";
    for (const text of ["Ranges, states and transfer", `${passedChecks} ${passedChecks === 1 ? "check" : "checks"}`, "Within declared limits"]) {
      const cell = document.createElement("td"); cell.textContent = text; row.append(cell);
    }
    row.append(verdictCell("Pass"));
    body.append(row);
  }
  const completed = new Set(scenarios?.map(scenario => scenario.id) ?? []);
  if (appendModelValidationRows(body, payload.modelValidation as IndependentModelValidation | undefined,
    JSON.parse(root.dataset.studioLoop ?? "null") as ControlLoopDefinition | null)) completed.add("plant-model");
  if (payload.initialization) completed.add("controller-transfer");
  const required = Array.isArray(payload.requiredScenarios) ? payload.requiredScenarios : [];
  for (const id of required) {
    if (typeof id !== "string" || completed.has(id)) continue;
    // The transfer run only follows a passing campaign; after a failure it was skipped, not lost.
    if (id === "controller-transfer" && payload.status === "verification-failed") continue;
    const row = document.createElement("tr"); row.dataset.studioRequirement = id;
    row.dataset.resultPriority = "1"; row.dataset.requirementPriority = "0";
    for (const text of [readable(id), "—", "Complete required scenario"]) {
      const cell = document.createElement("td"); cell.textContent = text; row.append(cell);
    }
    row.append(verdictCell("Inconclusive · Scenario did not complete."));
    body.append(row);
  }
  const rows = Array.from(body.rows).sort((left, right) => Number(left.dataset.resultPriority) - Number(right.dataset.resultPriority)
    || Number(left.dataset.requirementPriority) - Number(right.dataset.requirementPriority));
  body.replaceChildren(...rows);
  table.hidden = body.rows.length === 0;
}
const GENERATED_CHECK = /(^|:)(process-bounds|output-bounds|requested-output-bounds|output-rate-(up|down))$/;
/** An error measured from a reference other than the loop's setpoint, such as a planned trajectory, is named after that reference. */
function errorFromReference(kind: string, requirement: RunRequirement | undefined, loop: ControlLoopDefinition | null): string | undefined {
  const reference = (requirement as { reference?: { channel?: unknown } } | undefined)?.reference?.channel;
  if (kind !== "error" || typeof reference !== "string" || !loop || reference === loop.signals.setpoint?.symbol) return undefined;
  const signal = loop.signals.additional?.find(candidate => candidate.symbol === reference);
  return `Maximum error from ${signal ? readable(signal.id) : reference}`;
}
const names: Record<string, string> = { error: "Maximum steady-state error", settling: "Maximum settling time", overshoot: "Maximum overshoot",
  bounds: "Operating range", "state-bounds": "State range", rate: "Maximum rate", levels: "Allowed output levels", dwell: "Minimum dwell time",
  saturation: "Maximum saturation time", switching: "Maximum switching count", activity: "Command activity" };
function measurement(result: RunRequirementResult, requirement: RunRequirement | undefined, evaluation: RunEvaluation): string {
  if (requirement?.kind === "overshoot" && result.value !== undefined) return percentOfSetpointChange(result.value, requirement);
  if (requirement && (requirement.kind === "bounds" || requirement.kind === "state-bounds")) {
    const values = evaluation.run.samples.filter(sample => sample.timeSeconds >= requirement.window.start && sample.timeSeconds <= requirement.window.end)
      .map(sample => sample.values[requirement.channel]).filter(value => value?.quality === "good" && value.value !== null).map(value => value.value!);
    if (values.length) {
      const minimum = Math.min(...values), maximum = Math.max(...values);
      return `${formatEngineeringNumber(minimum)}${minimum === maximum ? "" : `–${formatEngineeringNumber(maximum)}`}${unitSuffix(result.unit)}`;
    }
  }
  return result.value === undefined ? "—" : `${formatEngineeringNumber(result.value)}${unitSuffix(result.unit)}`;
}
function limit(result: RunRequirementResult, requirement?: RunRequirement): string {
  if (requirement?.kind === "overshoot" && result.limit !== undefined) return `≤ ${percentOfSetpointChange(result.limit, requirement)}`;
  if (requirement?.kind === "bounds" || requirement?.kind === "state-bounds") return `${requirement.minimum === requirement.maximum ? "= " : ""}${formatEngineeringNumber(requirement.minimum)}${requirement.minimum === requirement.maximum ? "" : `–${formatEngineeringNumber(requirement.maximum)}`}${unitSuffix(result.unit)}`;
  if (requirement?.kind === "levels") return formatNumberList(requirement.allowed);
  return result.limit === undefined ? "—" : `${requirement?.kind === "dwell" ? "≥" : "≤"} ${formatEngineeringNumber(result.limit)}${unitSuffix(result.unit)}`;
}
function unitSuffix(unit: string): string { return unit === "1" ? "" : ` ${displayEngineeringUnit(unit)}`; }
/** Overshoot is read in percent of its setpoint change; the evaluator keeps the output unit, so only this display converts. */
function percentOfSetpointChange(value: number, step: { initial: number; target: number }): string {
  return `${formatEngineeringNumber(value / Math.abs(step.target - step.initial) * 100)} %`;
}
/** Identifier-like words made readable; a dotted symbol path stays exactly as the program declares it. */
function readable(value: string): string {
  return value.split(" ").map(word => word.includes(".") ? word : word.replace(/([a-z])([A-Z])/g, "$1 $2").replace(/[-_]/g, " ")).join(" ");
}
