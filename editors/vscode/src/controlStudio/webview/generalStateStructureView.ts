import type { SavedGeneralModelAnalysis } from "../generalModelPersistence";
import { formatEngineeringNumber } from "../chartContract";
export function renderGeneralStateStructure(saved: SavedGeneralModelAnalysis): void {
  const section = document.querySelector<HTMLElement>("[data-general-model-structure]");
  if (!section) return;
  const model = saved.model; const structure = saved.result.stateStructure;
  if (model.kind === "transfer-function" || !structure) { section.hidden = true; return; }
  section.hidden = false;
  document.querySelector<HTMLElement>("[data-state-structure-order]")!.textContent =
    `State order: ${model.states.map(s => `${s.id} (${s.unit})`).join(", ")}. Input order: ${model.inputs.map(s => `${s.id} (${s.unit})`).join(", ")}. Output order: ${model.outputs.map(s => `${s.id} (${s.unit})`).join(", ")}. Domain: ${model.kind}.`;
  const rank = document.querySelector<HTMLElement>("[data-state-structure-ranks]")!;
  const table = document.querySelector<HTMLElement>("[data-state-structure-matrices]")!; table.replaceChildren();
  const scope = document.querySelector<HTMLElement>("[data-state-structure-scope]")!;
  if (structure.status === "unavailable") { rank.textContent = "Structural analysis unavailable."; scope.textContent = structure.reason; return; }
  rank.textContent = `Controllability rank ${structure.controllability.rank} of ${model.states.length}. Observability rank ${structure.observability.rank} of ${model.states.length}.`;
  for (const [name, matrix] of [["A", model.a], ["B", model.b], ["C", model.c], ["D", model.d],
    ["Controllability [B, AB, …]", structure.controllability.matrix], ["Observability [C; CA; …]", structure.observability.matrix]] as const) {
    const row = document.createElement("tr"); const title = document.createElement("th"); title.textContent = name; row.append(title);
    const values = document.createElement("td"); values.style.whiteSpace = "normal";
    values.textContent = matrix.map(values => `[${values.map(v => formatEngineeringNumber(v)).join(", ")}]`).join("; "); row.append(values); table.append(row);
  }
  scope.textContent = structure.scope;
}
