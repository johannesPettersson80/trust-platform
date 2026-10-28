import type { StepResponseAnalysis, StepResponseMeasurement } from "../generated/ManualIdentificationAnswer";
import { formatEngineeringNumber } from "../chartContract";
import { displayEngineeringUnit } from "../unitDisplay";

/**
 * Shows the held step's measurements (specification 35, B3.3). Every fact the rows state is the engine's: the side of the baseline the onset is
 * sought on is the measurement's own (`onsetSide`, N2.5), and the unit of the gain is the model's (`modelGainUnit`, the presentation's `units.gain`).
 * The page derives neither; a fact the engine did not state is left out of its row.
 */
export function renderStepResponse(analysis: StepResponseMeasurement, output: string, input: string, evidence: HTMLDetailsElement,
  estimation?: StepResponseAnalysis, validation = false, modelGainUnit?: string): void {
  const n = (value: number | null | undefined) => value === undefined || value === null ? "Not detected" : formatEngineeringNumber(value);
  const unit = displayEngineeringUnit(output);
  const gainUnit = modelGainUnit === undefined ? "" : displayEngineeringUnit(modelGainUnit);
  const withUnit = (value: string, unitText: string) => unitText ? `${value} ${unitText}` : value;
  const rows: Array<[string, string]> = [];
  for (const [name, window] of [["Baseline", analysis.baseline], ["Final", analysis.final]] as const) {
    rows.push([name, `${n(window.mean)} ${unit} · ${n(window.startSeconds)}–${n(window.endSeconds)} s · ${window.count} samples`],
      [`${name} range / slope`, `${n(window.range)} ${unit} / ${n(window.slopePerSecond)} ${unit}/s`]);
  }
  rows.push(["Applied change", `${n(analysis.inputChange)} ${displayEngineeringUnit(input)}`],
    ["Step / restoration", `${n(analysis.stepTimeSeconds)} / ${n(analysis.holdEndSeconds)} s`],
    ["Detectable onset", `${n(analysis.detectableOnsetSeconds)} s after step · ${n(analysis.sampleTimeSeconds)} s sampling`],
    ["Departure / 63.2% level", `${n(analysis.onsetThreshold)} / ${n(analysis.level63)} ${unit}`],
    ["Onset criterion", `${analysis.settings.onsetSamples} consecutive samples · ${n(analysis.settings.onsetThreshold)} ${unit}${analysis.onsetSide ? ` ${analysis.onsetSide} baseline` : ""}`]);
  if (estimation) {
    rows.push([validation ? "Estimation gain" : "Best gain", withUnit(n(estimation.best.gain), gainUnit)], [validation ? "Estimation delay" : "Transport delay", `${n(estimation.best.delaySeconds)} s`],
      [validation ? "Estimation time constant" : "Time constant", `${n(estimation.best.timeConstantSeconds)} s`], [validation ? "Estimation consistent grid" : "Consistent grid", `${estimation.consistency.count} candidates · ±${n(estimation.settings.measurementBound)} ${unit}`]);
    for (const [label, range, rangeUnit] of [["Gain range", estimation.consistency.gain, gainUnit], ["Delay range", estimation.consistency.delaySeconds, "s"],
      ["Time constant range", estimation.consistency.timeConstantSeconds, "s"]] as const) {
      rows.push([label, range ? withUnit(`${n(range.minimum)}–${n(range.maximum)}`, rangeUnit) : "No consistent candidates"]);
    }
    rows.push(["Range meaning", "Grid-resolved marginal ranges; not statistical confidence"]);
  }
  if (analysis.warnings.length) rows.push(["Review", analysis.warnings.join(" ")]);
  evidence.querySelector("tbody")!.replaceChildren(...rows.map(([name, value]) => {
    const row = document.createElement("tr"), heading = document.createElement("th"), cell = document.createElement("td");
    heading.scope = "row"; heading.textContent = name; cell.textContent = value; row.append(heading, cell); return row;
  }));
  evidence.querySelector("summary")!.textContent = validation ? "Validation step measurements" : "Step-response measurements";
  evidence.hidden = false;
}
