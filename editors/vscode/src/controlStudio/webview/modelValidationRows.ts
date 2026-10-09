import type { IndependentModelValidation } from "../independentModelValidation";
import { formatEngineeringNumber } from "../chartContract";
import type { ControlLoopDefinition } from "../loopDefinition";
import { signalRoleLabel } from "./signalRoles";

/** Model agreement is a requirement in the existing Validate table. Its channel is named by its role; the symbol is the hover text (B9.9). */
export function appendModelValidationRows(body: HTMLTableSectionElement, report: IndependentModelValidation | undefined,
  loop: ControlLoopDefinition | null = null): boolean {
  if (!report) return false;
  for (const result of report.rows) {
    const row = body.insertRow(); row.dataset.studioRequirement = `plant-model:${result.channel}`; row.title = result.channel;
    const signal = [loop?.signals.processVariable, ...(loop?.signals.additional ?? [])].find(candidate => candidate?.symbol === result.channel);
    row.dataset.resultPriority = String(result.status === "Fail" ? 0 : result.status === "Inconclusive" ? 1 : 2);
    row.dataset.requirementPriority = "0";
    const error = result.normalizedError === undefined ? "—" : `${formatEngineeringNumber(result.normalizedError * 100)} %`;
    for (const value of [`Model agreement · ${signalRoleLabel(loop, result.channel, signal?.unit ?? "")}`, error,
      `≤ ${formatEngineeringNumber(result.limit * 100)} %`]) row.insertCell().textContent = value;
    // The verdict keeps its word; its icon and colour come from the verdict attribute, as in the requirement rows.
    const verdict = row.insertCell();
    verdict.textContent = result.status === "Pass" ? "Pass" : `${result.status} · ${result.reason}`;
    const kind = String(result.status).toLowerCase();
    if (["pass", "fail", "inconclusive"].includes(kind)) { verdict.classList.add("studio-verdict"); verdict.dataset.verdict = kind; }
  }
  return true;
}
