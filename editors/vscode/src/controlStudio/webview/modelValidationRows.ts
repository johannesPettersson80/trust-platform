import type { IndependentModelValidation } from "../independentModelValidation";
import { formatEngineeringNumber } from "../chartContract";

/** Model agreement is a requirement in the existing Validate table. */
export function appendModelValidationRows(body: HTMLTableSectionElement, report: IndependentModelValidation | undefined): boolean {
  if (!report) return false;
  for (const result of report.rows) {
    const row = body.insertRow(); row.dataset.studioRequirement = `plant-model:${result.channel}`;
    row.dataset.resultPriority = String(result.status === "Fail" ? 0 : result.status === "Inconclusive" ? 1 : 2);
    row.dataset.requirementPriority = "0";
    const error = result.normalizedError === undefined ? "—" : `${formatEngineeringNumber(result.normalizedError * 100)} %`;
    for (const value of [`Model agreement · ${result.channel}`, error, `≤ ${formatEngineeringNumber(result.limit * 100)} %`,
      result.status === "Pass" ? "Pass" : `${result.status} · ${result.reason}`]) row.insertCell().textContent = value;
  }
  return true;
}
