import type { ParameterBinding } from "../parameterBinding";
import type { ParameterStatus } from "../parameterApplication";
import { ParameterFields, parameterFieldLabel } from "./parameterFields";
import { displayEngineeringUnit } from "../unitDisplay";

export function initializeParameterWorkflow(post: (kind: string, payload?: Record<string, unknown>) => void): void {
  const section = document.querySelector<HTMLElement>("[data-parameters]");
  if (!section) return;
  const query = <T extends Element>(name: string) => section.querySelector<T>(`[data-parameters-${name}]`)!;
  const values = query<HTMLTextAreaElement>("values");
  const status = query<HTMLElement>("status");
  const area = query<HTMLElement>("review-area");
  const apply = query<HTMLButtonElement>("apply");
  const physical = query<HTMLInputElement>("physical-approved");
  let reviewId: string | undefined;
  let physicalSource = false;
  function invalidate(): void {
    reviewId = undefined; apply.disabled = true; area.hidden = true; physical.checked = false;
    query<HTMLElement>("editor").hidden = false;
    query<HTMLElement>("review-table").replaceChildren();
  }
  const edited = () => { invalidate(); query<HTMLButtonElement>("review").disabled = false; post("control-studio.parameters.invalidate"); };
  const fields = new ParameterFields(query<HTMLElement>("fields"), values, edited);
  query<HTMLButtonElement>("change").addEventListener("click", () => {
    edited(); query<HTMLButtonElement>("review").className = "primary";
    query<HTMLElement>("fields").querySelector<HTMLInputElement>("input")?.focus();
  });
  values.addEventListener("input", () => { fields.sync(); edited(); });
  query<HTMLButtonElement>("inspect").addEventListener("click", () => { invalidate(); post("control-studio.parameters.inspect"); });
  query<HTMLButtonElement>("selected").addEventListener("click", () => { invalidate(); post("control-studio.parameters.selected-analysis"); });
  query<HTMLButtonElement>("review").addEventListener("click", () => { invalidate(); post("control-studio.parameters.review", { json: values.value }); });
  query<HTMLButtonElement>("rollback").addEventListener("click", () => { invalidate(); post("control-studio.parameters.review-rollback"); });
  physical.addEventListener("change", () => { apply.disabled = !reviewId || (physicalSource && !physical.checked); });
  apply.addEventListener("click", () => {
    if (!reviewId) return;
    post("control-studio.parameters.apply", { reviewId, physicalApproved: physical.checked });
    reviewId = undefined; apply.disabled = true;
  });
  window.addEventListener("message", event => {
    const message = event.data;
    const root = document.querySelector<HTMLElement>("[data-control-studio]");
    if (!root || message?.kind !== "control-studio.engineering.result" || message.stage !== "parameters" ||
        message.projectId !== root.dataset.projectId || message.editorSessionId !== root.dataset.editorSessionId ||
        message.loopId !== root.dataset.loopId || message.expectedLoopRevision !== root.dataset.loopRevision) return;
    const payload = message.payload;
    if (!payload || typeof payload.status !== "string") return;
    if (payload.binding) fields.configure(payload.binding);
    if (payload.status === "inspected" || payload.status === "draft") {
      invalidate(); values.value = JSON.stringify(payload.values ?? payload.observed.values, null, 2);
      fields.sync();
      query<HTMLButtonElement>("inspect").className = "secondary";
      query<HTMLButtonElement>("review").disabled = false;
      query<HTMLButtonElement>("review").className = "primary";
      query<HTMLButtonElement>("rollback").disabled = !payload.observed?.previous_values;
      status.textContent = payload.reason ?? "Current values loaded. Edit a parameter, then review the change.";
    } else if (payload.status === "reviewed") {
      reviewId = payload.reviewId; physicalSource = payload.source === "physical"; physical.checked = false;
      area.hidden = false; apply.disabled = physicalSource;
      query<HTMLElement>("editor").hidden = true;
      apply.textContent = payload.rollback ? "Apply reviewed rollback" : "Apply reviewed values";
      query<HTMLElement>("physical").hidden = !physicalSource;
      const observed = payload.observed as ParameterStatus;
      const binding = payload.binding as ParameterBinding;
      query<HTMLElement>("identity").textContent = `${payload.source} · program ${observed.program_revision} · active ${observed.active_revision}`;
      query<HTMLElement>("manual").textContent = `Required Manual condition: ${binding.mode.symbol} = ${binding.mode.manual_value}; task ${binding.task_name}.`;
      const rows = query<HTMLElement>("review-table"); rows.replaceChildren();
      for (const parameter of binding.parameters) {
        const row = document.createElement("tr");
        for (const text of [parameterFieldLabel(parameter.symbol), observed.values[parameter.symbol], payload.values[parameter.symbol], displayEngineeringUnit(parameter.unit)]) {
          const cell = document.createElement("td"); cell.textContent = String(text); row.appendChild(cell);
        }
        row.cells[0].title = `${parameter.symbol} (${parameter.iec_type}) · ${parameter.convention}`;
        rows.appendChild(row);
      }
      query<HTMLButtonElement>("review").className = "secondary";
      status.textContent = payload.rollback ? "Review the previous values before restoring them." : "Review the change before applying.";
    } else {
      if (payload.status !== "applying") invalidate();
      if (payload.status === "applied" && payload.observed) {
        values.value = JSON.stringify(payload.observed.values, null, 2); fields.sync();
        query<HTMLButtonElement>("rollback").disabled = !payload.observed.previous_values;
      }
      status.textContent = fields.describe(payload.reason ?? payload.status);
    }
  });
  section.dataset.ready = "true";
}
