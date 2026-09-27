import { displayEngineeringUnit } from "../unitDisplay";
import type { WorkflowRecordingProvenance } from "../workflowResult";
import { setIdentificationActionState } from "./identificationActions";
export { setIdentificationActionState } from "./identificationActions";

export const IDENTIFICATION_IMPORT_CONTROLS = `<div class="workspace-actions"><button class="primary" data-identify-import>Import recording…</button><input id="identify-import-file" data-identify-import-file type="file" accept=".json,application/json" hidden></div>
<p data-identify-recording-source role="status" hidden></p>
<details class="advanced" data-identify-recording-details hidden><summary>Recording source</summary><div class="chart-table"><table><tbody data-identify-recording-provenance></tbody></table></div></details>
<button class="primary" data-identify-record-validation hidden>Record validation</button>`;

export function mountIdentificationRecordingDetails(): void {
  const pane = document.querySelector<HTMLElement>('[data-workflow-pane="Identify"]');
  const summary = pane?.querySelector<HTMLElement>(".summary-panel");
  if (!pane || !summary) return;
  for (const selector of ["[data-identify-recording-source]", "[data-identify-recording-details]", "[data-identify-record-validation]"]) {
    const element = pane.querySelector<HTMLElement>(selector);
    if (element && element.parentElement !== summary) summary.append(element);
  }
}

export function initializeIdentificationRecordingImport(post: (kind: string, payload: Record<string, unknown>) => void,
  availability: (state: { recording: boolean; result: boolean; model?: boolean; candidate?: boolean }) => void,
  showRetained: () => void = () => undefined): void {
  const root = document.querySelector<HTMLElement>("[data-control-studio]");
  const input = document.querySelector<HTMLInputElement>("[data-identify-import-file]");
  const button = document.querySelector<HTMLButtonElement>("[data-identify-import]");
  if (!root || !input || !button) return;
  mountIdentificationRecordingDetails();
  setIdentificationActionState("initial");
  button.addEventListener("click", () => input.click());
  document.querySelector<HTMLButtonElement>("[data-identify-record-validation]")?.addEventListener("click", () => {
    [...document.querySelectorAll<HTMLButtonElement>("[data-control-studio-mode]")]
      .find(candidate => candidate.textContent?.trim() === "Test")?.click();
  });
  let selection = 0;
  input.addEventListener("change", async () => {
    const current = ++selection, file = input.files?.[0];
    if (!file) return;
    if (file.size > 32 * 1024 * 1024) { status("Recording file exceeds 32 MiB."); return; }
    try {
      const json = await file.text();
      if (current !== selection) return;
      status("Reading the saved recording…"); post("control-studio.identify.import", { json });
    } catch { if (current === selection) status("The recording file could not be read."); }
  });
  window.addEventListener("message", event => {
    const message = event.data;
    if (message?.projectId !== root.dataset.projectId || message.loopId !== root.dataset.loopId ||
        message.editorSessionId !== root.dataset.editorSessionId || message.expectedLoopRevision !== root.dataset.loopRevision) return;
    if (message.kind === "control-studio.identification-recording") {
      if (message.payload?.status !== "imported") { status(String(message.payload?.reason ?? "Recording import failed.")); return; }
      show(message.payload.recording.provenance, message.payload.recording.rawSamples.length);
      setIdentificationActionState("imported");
      availability({ recording: true, result: true, model: false, candidate: false });
      showRetained();
      status(message.payload.reference
        ? "Original estimation imported with first-order settings. Check the recording to compare models."
        : "Imported recording ready. Check it before fitting a model.");
      for (const selector of ["[data-identify-fit]", "[data-identify-accept]", "[data-identification-fit]", "[data-identify-model-details]", "[data-identify-qualification]", "[data-identify-details]"]) {
        const element = document.querySelector<HTMLElement>(selector); if (element) element.hidden = true;
      }
    } else if (message.kind === "control-studio.workflow-result" && ["opened", "restored"].includes(message.phase)) {
      const recording = message.payload?.result?.recording;
      show(recording?.provenance, recording?.rawSamples?.length ?? 0);
    } else if ((message.kind === "control-studio.recording.evidence" && message.payload?.state === "recording") ||
        (message.kind === "control-studio.experiment.result" && message.phase === "running")) show(undefined, 0);
  });
}

function status(text: string): void {
  const element = document.querySelector("[data-identify-state]"); if (element) element.textContent = text;
}
function show(origin: WorkflowRecordingProvenance | undefined, count: number): void {
  const summary = document.querySelector<HTMLElement>("[data-identify-recording-source]");
  const details = document.querySelector<HTMLDetailsElement>("[data-identify-recording-details]");
  const table = document.querySelector("[data-identify-recording-provenance]");
  if (!summary || !details || !table) return;
  summary.hidden = details.hidden = !origin;
  if (!origin) { table.replaceChildren(); return; }
  summary.textContent = `Imported recording · ${origin.source} · ${count} samples`;
  const rows = [["Project", origin.projectId], ["Loop", origin.loop.id], ["Loop revision", origin.loop.revision],
    ["Result", origin.resultId], ["Result revision", String(origin.resultRevision)], ["Recorded", origin.createdAt],
    ["Input / output", `${displayEngineeringUnit(origin.units.mv)} / ${displayEngineeringUnit(origin.units.pv)}`]];
  table.replaceChildren(...rows.map(([label, value]) => {
    const row = document.createElement("tr"), heading = document.createElement("th"), cell = document.createElement("td");
    heading.scope = "row"; heading.textContent = label; cell.textContent = value; cell.style.overflowWrap = "anywhere";
    row.append(heading, cell); return row;
  }));
}
