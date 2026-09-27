import type { SavedRunValidation } from "../runValidationPersistence";
import type { RunEvaluation } from "../runValidation";
import { formatEngineeringNumber } from "../chartContract";
export function initializeRunValidationWorkflow(post: (kind: string, payload?: Record<string, unknown>) => void): void {
  const currentFile = document.querySelector<HTMLInputElement>("#run-validation-current");
  if (!currentFile) return;
  const candidateFile = document.querySelector<HTMLInputElement>("#run-validation-candidate")!;
  const requirements = document.querySelector<HTMLTextAreaElement>("#run-validation-requirements")!;
  const objective = document.querySelector<HTMLInputElement>("#run-validation-objective")!;
  const status = document.querySelector<HTMLElement>("[data-run-validation-status]")!;
  const evaluate = document.querySelector<HTMLButtonElement>("[data-run-validation-evaluate]")!;
  const cancel = document.querySelector<HTMLButtonElement>("[data-run-validation-cancel]")!;
  const save = document.querySelector<HTMLButtonElement>("[data-run-validation-save]")!;
  const result = document.querySelector<HTMLElement>("[data-run-validation-result]")!;
  let hasCurrent = false;
  function showRunIdentity(role: "current" | "candidate", id?: string, evidence?: string): void {
    document.querySelector<HTMLElement>(`[data-run-validation-${role}]`)!.textContent = id
      ? `Imported ${id} · declared ${evidence} · acquisition not independently verified.`
      : `No ${role} run imported.`;
  }
  for (const [file, role] of [[currentFile, "current"], [candidateFile, "candidate"]] as const) {
    file.addEventListener("change", async () => {
      const selected = file.files?.[0]; if (!selected) return;
      if (selected.size > 2 * 1024 * 1024) { status.textContent = "Choose a run JSON file of at most 2 MiB."; return; }
      result.hidden = true; save.disabled = true;
      try { post(`control-studio.run-validation.import-${role}`, { json: await selected.text() }); }
      catch (error) { status.textContent = String(error); }
    });
  }
  evaluate.addEventListener("click", () => {
    if (requirements.value.length > 65536) { status.textContent = "Requirements exceed 64 kB."; return; }
    post("control-studio.run-validation.evaluate", { json: requirements.value, ...(objective.value.trim() ? { objective: objective.value.trim() } : {}) });
  });
  cancel.addEventListener("click", () => post("control-studio.run-validation.cancel"));
  save.addEventListener("click", () => post("control-studio.result.save"));
  for (const control of [requirements, objective]) control.addEventListener("input", () => {
    save.disabled = true; result.hidden = true; status.textContent = "Requirements changed. Evaluate again before saving.";
    post("control-studio.run-validation.invalidate");
  });
  function rows(label: string, evaluation: RunEvaluation): void {
    const table = document.querySelector<HTMLElement>("[data-run-validation-rows]")!;
    const heading = document.createElement("tr");
    const headingCell = document.createElement("th"); headingCell.colSpan = 3;
    headingCell.textContent = `${label} · imported ${evaluation.run.evidence}`;
    heading.append(headingCell); table.append(heading);
    for (const requirement of evaluation.requirements) {
      const row = document.createElement("tr");
      const measurement = `Value ${requirement.value === undefined ? "—" : formatEngineeringNumber(requirement.value)} ${requirement.unit}; limit ${requirement.limit === undefined ? "—" : formatEngineeringNumber(requirement.limit)} ${requirement.unit}`;
      for (const value of [requirement.id, requirement.status, measurement]) {
        const cell = document.createElement("td"); cell.style.whiteSpace = "normal";
        cell.style.overflowWrap = "anywhere"; cell.textContent = value; row.append(cell);
      }
      table.append(row);
      const reason = document.createElement("tr"); const cell = document.createElement("td");
      cell.colSpan = 3; cell.style.whiteSpace = "normal"; cell.textContent = requirement.reason;
      reason.append(cell); table.append(reason);
    }
  }

  window.addEventListener("trust-control-studio-run-validation", event => {
    const payload = (event as CustomEvent).detail as Record<string, unknown>;
    const state = String(payload.status);
    cancel.disabled = state !== "running"; save.disabled = state !== "complete";
    evaluate.disabled = !hasCurrent || state === "running";
    if (state === "imported") {
      if (payload.role === "current") hasCurrent = true;
      showRunIdentity(payload.role === "current" ? "current" : "candidate", String(payload.id), String(payload.evidence));
      evaluate.disabled = !hasCurrent; result.hidden = true;
      status.textContent = "Run imported. Declare requirements and evaluate.";
    } else if (state === "complete") {
      const saved = payload.saved as SavedRunValidation;
      hasCurrent = true; evaluate.disabled = false;
      showRunIdentity("current", saved.current.id, saved.current.evidence);
      showRunIdentity("candidate", saved.candidate?.id, saved.candidate?.evidence);
      requirements.value = JSON.stringify(saved.requirements, null, 2); objective.value = saved.objective ?? "";
      const comparison = saved.comparison;
      const outcome = comparison?.status ?? saved.evaluation.status;
      status.textContent = `Evaluation complete: ${outcome}. No controller was applied.`;
      document.querySelector<HTMLElement>("[data-run-validation-summary]")!.textContent =
        `Imported record evidence · ${outcome}. ${comparison?.reason ?? saved.evaluation.scope}${comparison?.preferred ? ` Qualifying selection: ${comparison.preferred}.` : ""} Acquisition was not independently verified here.`;
      document.querySelector<HTMLElement>("[data-run-validation-rows]")!.replaceChildren();
      rows("Current", saved.evaluation); if (comparison) rows("Candidate", comparison.candidate);
      result.hidden = false;
    } else { result.hidden = true; status.textContent = String(payload.reason ?? state); }
  });
}
