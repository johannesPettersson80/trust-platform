import type { DeclaredRootLocusResult } from "../declaredAnalysis";
import { formatEngineeringNumber } from "../chartContract";

export function renderDeclaredRootResult(result: DeclaredRootLocusResult): void {
  const host = document.querySelector<HTMLElement>("[data-declared-root-result]");
  if (!host) return;
  const summary = document.createElement("p");
  const fixed = Object.entries(result.fixedParameters).map(([name, parameter]) =>
    `${name} = ${formatEngineeringNumber(parameter.value)} ${parameter.unit}`).join(", ");
  summary.textContent = `${result.provenance}. Varying ${result.parameter} (${result.parameterUnit}); fixed: ${fixed || "none"}. ${result.approximation}`;
  const table = document.createElement("table");
  const caption = document.createElement("caption"); caption.textContent = "Calculated poles"; table.append(caption);
  const heading = document.createElement("tr");
  for (const label of ["Parameter value", "Poles (real, imaginary)", "Selection", "Candidate"]) {
    const cell = document.createElement("th"); cell.textContent = label; heading.append(cell);
  }
  table.append(heading);
  for (const branch of result.branches) {
    const row = document.createElement("tr");
    row.dataset.selected = String(branch.gain === result.selectedValue);
    for (const text of [`${result.parameter} = ${formatEngineeringNumber(branch.gain)}`,
      branch.poles.map(pole => `(${formatEngineeringNumber(pole.real)}, ${formatEngineeringNumber(pole.imaginary)})`).join("; "),
      branch.gain === result.selectedValue ? "Selected candidate" : ""]) {
      const cell = document.createElement("td"); cell.textContent = text; row.append(cell);
    }
    const action = document.createElement("td");
    const select = document.createElement("button");
    select.dataset.declaredRootSelect = String(branch.gain);
    select.textContent = `Select ${result.parameter} = ${formatEngineeringNumber(branch.gain)}`;
    select.disabled = branch.gain === result.selectedValue;
    action.append(select); row.append(action);
    table.append(row);
  }
  host.replaceChildren(summary, table);
}
