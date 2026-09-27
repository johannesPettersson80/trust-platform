import type { EngineerRequirements } from "../engineerVerificationPlan";
import { appendModelValidationRows } from "./modelValidationRows";
import type { IndependentModelValidation } from "../independentModelValidation";
import type { ControlLoopDefinition } from "../loopDefinition";
import { groupSettingSymbols, ownedParameterLabel } from "../controllerGroupPresentation";
import type { RunEvaluation, RunRequirement, RunRequirementResult } from "../runValidation";
import { displayEngineeringUnit, formatEngineeringNumber } from "../chartContract";

export function renderTunedController(root: HTMLElement, loop: ControlLoopDefinition, payload: Record<string, unknown>): void {
  if (payload.status !== "candidate" || !payload.values || typeof payload.values !== "object") return;
  const values = payload.values as Record<string, number>;
  const area = root.querySelector<HTMLElement>("[data-studio-tuning-result]")!;
  const family = String(payload.family ?? loop.controller.type);
  const title = document.createElement("p"); title.textContent = `${family} controller · ${loop.controller.type}`;
  const list = document.createElement("dl"); list.className = "studio-setting-values";
  const law = loop.controller.law;
  const groupSymbols = groupSettingSymbols(loop);
  const primary = law?.kind === "pid" ? new Map([
    [law.proportionalGainSymbol, "Proportional gain (Kp)"],
    ...(["PI", "PID"].includes(family) ? [[law.integralGainSymbol, "Integral gain (Ki)"]] : []),
    ...(["PD", "PID"].includes(family) ? [[law.derivativeGainSymbol, "Derivative gain (Kd)"]] : []),
  ] as [string, string][]) : law?.kind === "proportional" ? new Map([[law.gainSymbol, "Proportional gain (Kp)"]]) : undefined;
  for (const parameter of loop.parameterBinding?.parameters ?? []) {
    if (!Number.isFinite(values[parameter.symbol]) || primary && !primary.has(parameter.symbol) || groupSymbols && !groupSymbols.has(parameter.symbol)) continue;
    const label = document.createElement("dt"); label.textContent = ownedParameterLabel(loop, parameter.symbol, primary?.get(parameter.symbol) ?? readable(parameter.convention || parameter.symbol));
    const value = document.createElement("dd"); value.textContent = `${formatEngineeringNumber(values[parameter.symbol])} ${displayEngineeringUnit(parameter.unit)}`;
    value.dataset.studioParameter = parameter.symbol;
    list.append(label, value);
  }
  area.replaceChildren(title, list);
  const design = payload.design as { horizon?: { steps: number; policy: string; gainSequence: number[][][]; riccatiSequence: number[][][] } } | undefined;
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
    const matrix = (value: number[][] | undefined) => value ? value.map(row => row.map(formatEngineeringNumber).join(", ")).join("; ") : "—";
    horizon.riccatiSequence.forEach((cost, stage) => { const row = body.insertRow(); row.dataset.studioRiccatiStage = String(stage);
      for (const value of [String(stage), matrix(horizon.gainSequence[stage]), matrix(cost)]) row.insertCell().textContent = value;
    });
    details.append(table); area.append(policy, details);
  }
  area.hidden = false;
}

export function renderValidationResults(root: HTMLElement, payload: Record<string, unknown>): void {
  if (["edited", "model-ready", "candidate"].includes(String(payload.status))) {
    root.querySelector<HTMLTableElement>("[data-studio-validation-result]")!.hidden = true; return;
  }
  if (!["verified", "verification-failed", "restored", "failed", "cancelled"].includes(String(payload.status))) return;
  if (!payload.scenarioResults && !payload.candidate && !payload.requiredScenarios && !payload.modelValidation) return;
  const scenarios = payload.scenarioResults as { id: string; result: { evaluation: RunEvaluation } }[] | undefined;
  const evaluations = scenarios?.length ? scenarios.map(scenario => ({ name: scenario.id, evaluation: scenario.result.evaluation }))
    : payload.candidate ? [{ name: "", evaluation: payload.candidate as RunEvaluation }] : [];
  if (payload.initialization && typeof payload.initialization === "object") evaluations.push({ name: "Controller transfer", evaluation: payload.initialization as RunEvaluation });
  const table = root.querySelector<HTMLTableElement>("[data-studio-validation-result]")!;
  const body = table.tBodies[0]; body.replaceChildren();
  for (const { name, evaluation } of evaluations) {
    if (!evaluation.requirements || !evaluation.specification) continue;
    for (const result of evaluation.requirements) {
      const requirement = evaluation.specification.find(item => item.id === result.id);
      const row = document.createElement("tr"); row.dataset.studioRequirement = result.id;
      const fixedParameter = result.id.startsWith("fixed-parameter:");
      const label = fixedParameter ? "Parameter unchanged" : requirement?.kind === "rate" && requirement.direction
        ? requirement.direction === "up" ? "Maximum increase rate" : "Maximum decrease rate" : names[result.kind] ?? readable(result.kind);
      const channel = requirement?.channel && (fixedParameter || result.kind === "state-bounds" ? readable(requirement.channel.split(".").pop()!) : requirement.channel);
      const criteria = (payload.task as EngineerRequirements | undefined)?.automaticCriteria;
      const field = result.kind === "error" ? "maximumError" : result.kind === "settling" ? "settlingTimeSeconds" : undefined;
      const loop = JSON.parse(root.dataset.studioLoop ?? "null") as ControlLoopDefinition | null;
      const output = loop?.channels?.outputs.find(output => {
        const signal = output.signalId === "processVariable" ? loop.signals.processVariable : loop.signals.additional?.find(signal => signal.id === output.signalId);
        return signal?.symbol === requirement?.channel;
      });
      const automatic = field && criteria?.fields.includes(loop?.channels && loop.channels.outputs.length > 1 ? `${field}:${output?.modelChannelId}` : field);
      const title = `${label}${automatic ? " · Automatic" : ""}${channel ? ` · ${channel}` : ""}${evaluations.length > 1 && name && name !== "target-response" ? ` · ${readable(name)}` : ""}`;
      row.title = requirement?.channel ?? "";
      row.dataset.resultPriority = String(result.status === "Fail" ? 0 : result.status === "Inconclusive" ? 1 : 2);
      row.dataset.requirementPriority = String(fixedParameter ? 7 : ({ settling: 0, overshoot: 1, error: 2, bounds: 3, rate: 4, "state-bounds": 6 } as Record<string, number>)[result.kind] ?? 5);
      for (const text of [title, measurement(result, requirement, evaluation), limit(result, requirement), result.status]) {
        const cell = document.createElement("td"); cell.textContent = text; row.append(cell);
      }
      if (result.status !== "Pass") row.cells[3].textContent = `${result.status} · ${result.reason}`;
      body.append(row);
    }
  }
  const completed = new Set(scenarios?.map(scenario => scenario.id) ?? []);
  if (appendModelValidationRows(body, payload.modelValidation as IndependentModelValidation | undefined)) completed.add("plant-model");
  if (payload.initialization) completed.add("controller-transfer");
  const required = Array.isArray(payload.requiredScenarios) ? payload.requiredScenarios : [];
  for (const id of required) {
    if (typeof id !== "string" || completed.has(id)) continue;
    const row = document.createElement("tr"); row.dataset.studioRequirement = id;
    row.dataset.resultPriority = "1"; row.dataset.requirementPriority = "0";
    for (const text of [readable(id), "—", "Complete required scenario", "Inconclusive · Scenario did not complete."]) {
      const cell = document.createElement("td"); cell.textContent = text; row.append(cell);
    }
    body.append(row);
  }
  const rows = Array.from(body.rows).sort((left, right) => Number(left.dataset.resultPriority) - Number(right.dataset.resultPriority)
    || Number(left.dataset.requirementPriority) - Number(right.dataset.requirementPriority));
  body.replaceChildren(...rows);
  table.hidden = body.rows.length === 0;
}
const names: Record<string, string> = { error: "Maximum steady-state error", settling: "Maximum settling time", overshoot: "Maximum overshoot",
  bounds: "Operating range", "state-bounds": "State range", rate: "Maximum rate", levels: "Allowed output levels", dwell: "Minimum dwell time",
  saturation: "Maximum saturation time", switching: "Maximum switching count" };
function measurement(result: RunRequirementResult, requirement: RunRequirement | undefined, evaluation: RunEvaluation): string {
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
  if (requirement?.kind === "bounds" || requirement?.kind === "state-bounds") return `${requirement.minimum === requirement.maximum ? "= " : ""}${formatEngineeringNumber(requirement.minimum)}${requirement.minimum === requirement.maximum ? "" : `–${formatEngineeringNumber(requirement.maximum)}`}${unitSuffix(result.unit)}`;
  if (requirement?.kind === "levels") return requirement.allowed.map(formatEngineeringNumber).join(", ");
  return result.limit === undefined ? "—" : `${requirement?.kind === "dwell" ? "≥" : "≤"} ${formatEngineeringNumber(result.limit)}${unitSuffix(result.unit)}`;
}
function unitSuffix(unit: string): string { return unit === "1" ? "" : ` ${displayEngineeringUnit(unit)}`; }
function readable(value: string): string { return value.replace(/([a-z])([A-Z])/g, "$1 $2").replace(/[-_]/g, " "); }
