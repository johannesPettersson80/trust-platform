import type { FiniteSearchResult } from "../finiteSearchPersistence";
import { formatEngineeringNumber } from "../chartContract";

export function initializeFiniteSearchWorkflow(post: (kind: string, payload?: Record<string, unknown>) => void): void {
  const section = document.querySelector<HTMLElement>("[data-finite-search]");
  if (!section) return;
  const query = <T extends Element>(name: string) => section.querySelector<T>(`[data-search-${name}]`)!;
  const input = query<HTMLTextAreaElement>("request");
  const status = query<HTMLElement>("status");
  const start = query<HTMLButtonElement>("start");
  const cancel = query<HTMLButtonElement>("cancel");
  const save = query<HTMLButtonElement>("save");
  let reviewId: string | undefined;
  const edited = () => { reviewId = undefined; start.disabled = true; save.disabled = true; query<HTMLElement>("review-details").hidden = true; };
  input.addEventListener("input", () => { edited(); post("control-studio.search.invalidate"); });
  query<HTMLButtonElement>("inspect").addEventListener("click", () => { edited(); post("control-studio.search.inspect"); });
  query<HTMLButtonElement>("review").addEventListener("click", () => { edited(); post("control-studio.search.review", { json: input.value }); });
  start.addEventListener("click", () => { if (reviewId) post("control-studio.search.start", { reviewId }); start.disabled = true; reviewId = undefined; });
  cancel.addEventListener("click", () => post("control-studio.search.cancel"));
  save.addEventListener("click", () => post("control-studio.result.save"));
  query<HTMLElement>("trials").addEventListener("click", event => {
    const button = (event.target as Element).closest<HTMLButtonElement>("[data-search-select]");
    if (button) post("control-studio.search.select", { index: Number(button.dataset.searchSelect) });
  });
  window.addEventListener("message", event => {
    const message = event.data;
    const root = document.querySelector<HTMLElement>("[data-control-studio]");
    if (!root || message?.kind !== "control-studio.engineering.result" || message.stage !== "search" ||
        message.projectId !== root.dataset.projectId || message.editorSessionId !== root.dataset.editorSessionId ||
        message.loopId !== root.dataset.loopId || message.expectedLoopRevision !== root.dataset.loopRevision) return;
    const payload = message.payload;
    if (!payload || typeof payload.status !== "string") return;
    input.disabled = payload.status === "running" || payload.status === "cancelling";
    cancel.disabled = !input.disabled;
    if (payload.status === "ready") {
      if (payload.draft) input.value = JSON.stringify(payload.draft, null, 2);
      status.textContent = "Loaded ST and simulated plant are ready. Review the values, events and bounds.";
    } else if (payload.status === "reviewed") {
      reviewId = payload.reviewId; start.disabled = false;
      query<HTMLElement>("review-details").hidden = false;
      query<HTMLElement>("reviewed").textContent = JSON.stringify(payload.request, null, 2);
      status.textContent = `Review ready: ${payload.request.trial_count} trials, ${payload.request.duration_ms / 1000} s each, step ${payload.request.step_ms} ms. Start consumes this exact review.`;
    } else if (payload.status === "complete" || (payload.status === "running" && payload.result)) {
      const result = payload.result as FiniteSearchResult & { selectedIndex?: number };
      const complete = payload.status === "complete";
      save.disabled = !complete;
      query<HTMLElement>("result").hidden = false;
      status.textContent = complete ? `Search ${result.status}: ${result.trials.length}/${result.trial_count} trials retained.` : `Running ${result.trials.length}/${result.trial_count} trials…`;
      query<HTMLElement>("summary").textContent = `Actual ST in isolated simulation. ${result.ranking.length} qualifying candidates. ${result.reason ?? "No parameters were applied."}`;
      const rows = query<HTMLElement>("trials"); rows.replaceChildren();
      for (const trial of result.trials) {
        const row = document.createElement("tr");
        for (const text of [JSON.stringify(trial.parameters), trial.status, [trial.score === null ? "No score" : `${formatEngineeringNumber(trial.score)} ${trial.score_unit ?? ""}`, trial.reason, ...(trial.violations ?? [])].filter(Boolean).join(" · ")]) {
          const cell = document.createElement("td"); cell.textContent = String(text); row.appendChild(cell);
        }
        const cell = document.createElement("td");
        if (complete && result.ranking.includes(trial.index)) {
          const button = document.createElement("button"); button.className = "secondary"; button.dataset.searchSelect = String(trial.index);
          button.textContent = result.selectedIndex === trial.index ? "Selected" : "Select candidate";
          button.disabled = result.selectedIndex === trial.index; cell.appendChild(button);
        }
        row.appendChild(cell); rows.appendChild(row);
      }
    } else { status.textContent = payload.reason ?? payload.status; }
  });
  section.dataset.ready = "true";
}
