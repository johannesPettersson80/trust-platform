import type { SensorLagComparison } from "../generated/ManualIdentificationAnswer";
import { formatEngineeringNumber } from "../chartContract";
import { displayEngineeringUnit } from "../unitDisplay";

/** Shared presentation only: calculations and model selection remain host-owned. */
export function modelStructureEvidence(comparison: SensorLagComparison, units: { input?: string; output?: string } = {}) {
    const output = displayEngineeringUnit(units.output ?? "PV"), input = displayEngineeringUnit(units.input ?? "MV");
    const value = (number: number | null | undefined, unit = "") => `${formatEngineeringNumber(number, 5)}${unit ? ` ${unit}` : ""}`;
    const range = (bounds: { minimum: number; maximum: number } | null | undefined) => bounds
      ? `${value(bounds.minimum)}–${value(bounds.maximum)}` : "No consistent candidates";
    const a = comparison.candidateA, b = comparison.candidateB;
    const table = document.createElement("table");
    table.className = "studio-requirements";
    table.innerHTML = '<thead><tr><th scope="col">Evidence</th><th scope="col">A · First order</th><th scope="col">B · Additional lag</th></tr></thead><tbody></tbody>';
    const rows: Array<[string, string, string]> = [
      ["Model-error limits", a.adequate ? "Met" : "Not met", b.adequate ? "Met" : "Not met"],
      [`Peak residual (${output})`, value(a.maximumAbsoluteResidual), value(b.maximumAbsoluteResidual)],
      [`Final error (${output})`, value(a.finalError), value(b.finalError)],
      [`RMS residual (${output})`, value(a.rootMeanSquareError), value(b.rootMeanSquareError)],
      ["Consistent parameter sets", `${a.analysis.consistency.count} candidates`, b.analysis.consistency.count ? `${b.analysis.consistency.count} candidates` : "No consistent candidates"],
    ];
    appendRows(table, rows);
    const details = document.createElement("details"); details.className = "advanced";
    details.innerHTML = '<summary>Parameters and uncertainty</summary><div class="chart-table"><table class="studio-requirements"><thead><tr><th scope="col">Parameter</th><th scope="col">A · First order</th><th scope="col">B · Additional lag</th></tr></thead><tbody></tbody></table></div>';
    appendRows(details.querySelector("table")!, [
      ["Equation", (a as typeof a & { equation?: string }).equation ?? "—", (b as typeof b & { equation?: string }).equation ?? "—"],
      [`Gain (${output}/${input})`, value(a.model.gain), value(b.model.gain)],
      ["Time constants (s)", value(a.model.timeConstantSeconds), `${value(b.model.timeConstantSeconds)} / ${value(b.model.secondTimeConstantSeconds)}`],
      ["Delay (s)", value(a.model.deadTimeSeconds), value(b.model.deadTimeSeconds)],
      ["Baseline", value(a.analysis.best.baseline, output), value(b.analysis.best.baseline, output)],
      [`Baseline range (${output})`, range(a.analysis.consistency.baseline), range(b.analysis.consistency.baseline)],
      ["Gain range", range(a.analysis.consistency.gain), range(b.analysis.consistency.gain)],
      ["First time-constant range (s)", range(a.analysis.consistency.timeConstantSeconds), range(b.analysis.consistency.timeConstantSeconds)],
      ["Additional time-constant range (s)", "No added state", range(b.analysis.consistency.secondTimeConstantSeconds)],
      ["Delay range (s)", range(a.analysis.consistency.delaySeconds), range(b.analysis.consistency.delaySeconds)],
      ["Physical origin of added lag", "No added state", "Unresolved"],
    ]);
    const explanation = document.createElement("p");
    explanation.textContent = `Limits: peak ${value(comparison.requirements.maximumAbsoluteResidual, output)}, final ${value(comparison.requirements.maximumFinalError, output)}. Measurement bound: ±${value(b.analysis.settings.measurementBound, output)}. Ranges are grid marginals, not statistical confidence.`;
    const warnings = [...new Set([...a.analysis.warnings, ...b.analysis.warnings])];
    if (warnings.length) { const note = document.createElement("p"); note.textContent = warnings.join(" "); details.append(note); }
    const tableWrap = document.createElement("div"); tableWrap.className = "chart-table"; tableWrap.append(table);
    return { tableWrap, explanation, details };
}

function appendRows(table: HTMLTableElement, rows: Array<[string, string, string]>): void {
  const body = table.querySelector("tbody")!;
  for (const [label, ...values] of rows) {
    const row = document.createElement("tr"), heading = document.createElement("th"); heading.scope = "row"; heading.textContent = label; row.append(heading);
    for (const value of values) { const cell = document.createElement("td"); cell.textContent = value; row.append(cell); }
    body.append(row);
  }
}
