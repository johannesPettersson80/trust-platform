import type { calculateDeclaredRouth } from "../routhHurwitz";

type RouthResult = ReturnType<typeof calculateDeclaredRouth>;
export function initializeRouthWorkflow(post: (kind: string, payload?: Record<string, unknown>) => void): void {
  const section = document.querySelector<HTMLElement>("[data-routh]");
  if (!section) return;
  const input = section.querySelector<HTMLInputElement>("[data-routh-value]")!;
  const status = section.querySelector<HTMLElement>("[data-routh-status]")!;
  const output = section.querySelector<HTMLElement>("[data-routh-result]")!;
  section.querySelector("[data-routh-run]")!.addEventListener("click", () => {
    setActiveAnalysis("chart");
    output.hidden = true;
    status.textContent = "Calculating Routh–Hurwitz…";
    post("control-studio.routh.run", { selectedValue: input.value.trim() ? Number(input.value) : null });
  });
  window.addEventListener("trust-control-studio-routh", event => {
    const payload = (event as CustomEvent).detail;
    if (payload.status !== "complete") { output.hidden = true; status.textContent = String(payload.reason); return; }
    const result = payload.result as RouthResult;
    const table = result.polynomial;
    status.textContent = `${result.parameter} = ${result.selectedValue} ${result.parameterUnit}. Analysis complete.`;
    output.replaceChildren(); output.hidden = false;
    const paragraph = (text: string) => { const p = document.createElement("p"); p.textContent = text; output.append(p); };
    paragraph(`Characteristic polynomial: ${table.coefficients.map((coefficient, index) => {
      const power = table.coefficients.length - index - 1;
      return `${coefficient}${power ? ` s${power > 1 ? `^${power}` : ""}` : ""}`;
    }).join(" + ").replace(/\+ -/g, "− ")} = 0.`);
    paragraph(`Coefficients in descending powers of s: ${table.coefficients.join(", ")}.`);
    const grid = document.createElement("table"); grid.dataset.routhTable = "";
    grid.style.cssText = "width:100%;text-align:left;border-collapse:collapse";
    const head = grid.createTHead().insertRow();
    for (const text of ["Power of s", "First column", "Remaining entries"]) { const th = document.createElement("th"); th.textContent = text; head.append(th); }
    const body = grid.createTBody();
    for (const row of table.rows) {
      const tr = body.insertRow();
      for (const text of [`s^${row.power}`, String(row.entries[0]), row.entries.slice(1).join(", ") || "—"]) tr.insertCell().textContent = text;
    }
    output.append(grid);
    paragraph(`First-column signs: ${table.firstColumn.map(entry => entry.sign).join(", ")}.`);
    paragraph(table.signChanges === null ? "Right-half-plane pole count: unresolved special case; see explanation below."
      : `${table.signChanges} right-half-plane pole${table.signChanges === 1 ? "" : "s"} (${table.signChanges} sign changes).`);
    paragraph(table.interpretation);
    for (const issue of table.issues) paragraph(issue.explanation);
    const steps = document.createElement("details");
    const summary = document.createElement("summary"); summary.textContent = "How the table is calculated"; steps.append(summary);
    const formula = document.createElement("p"); formula.textContent = "First two rows: alternating coefficients, padded with zero. Each next entry is (B₀ Aⱼ₊₁ − A₀ Bⱼ₊₁) / B₀, using the two preceding rows A and B."; steps.append(formula);
    const calculations = document.createElement("pre"); calculations.style.cssText = "white-space:pre-wrap;overflow-wrap:anywhere";
    calculations.textContent = table.rows.filter(row => row.calculations.length).map(row => `s^${row.power}: ${row.calculations.map(step => `(${step.b0} × ${step.aNext} − ${step.a0} × ${step.bNext}) / ${step.b0} = ${step.result}`).join("\n")}`).join("\n");
    steps.append(calculations); output.append(steps);
    paragraph(`Analytical stable range: ${intervalText(result)}.`);
    paragraph(result.interval.derivation);
    for (const inequality of result.interval.inequalities) paragraph(inequality.replace(/\bp\b/g, result.parameter));
    paragraph(`Configured parameter domain: ${result.allowedDomain.minimum} ≤ ${result.parameter} ≤ ${result.allowedDomain.maximum} ${result.parameterUnit}. This is separate from the mathematical interval.`);
    paragraph(`${result.provenance}. ${result.scope}`);
    setActiveAnalysis("routh");
  });
  window.addEventListener("trust-control-studio-general-model", event => {
    if ((event as CustomEvent).detail?.status !== "imported") return;
    output.hidden = true;
    status.textContent = "Selected model changed. Calculate again using the new plant and existing controller.";
    setActiveAnalysis("chart");
  });
  document.addEventListener("click", event => {
    if (event.target instanceof Element
        && event.target.closest("[data-declared-root-run],[data-declared-root-select],[data-general-model-analyze]")) setActiveAnalysis("chart");
  });
  section.dataset.ready = "true";
}
function setActiveAnalysis(value: "routh" | "chart"): void {
  const studio = document.querySelector<HTMLElement>(".studio");
  if (studio) studio.dataset.analysisView = value;
}
function intervalText(result: RouthResult): string {
  const interval = result.interval;
  if (interval.status !== "qualified") return interval.status;
  if (interval.kind === "empty") return "no stable values";
  if (interval.kind === "all-real") return "all real parameter values";
  const low = interval.lower;
  const high = interval.upper;
  if (low !== undefined && Number.isFinite(low) && high !== undefined && Number.isFinite(high)) return `${low} < ${result.parameter} < ${high}`;
  if (low !== undefined && Number.isFinite(low)) return `${result.parameter} > ${low}`;
  if (high !== undefined && Number.isFinite(high)) return `${result.parameter} < ${high}`;
  return "see analytical conditions";
}
