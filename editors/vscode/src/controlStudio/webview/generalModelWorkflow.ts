import { renderGeneralStateStructure } from "./generalStateStructureView";
import { initializeGeneralFeedbackWorkflow } from "./generalFeedbackWorkflow";
import type { SavedGeneralModelAnalysis } from "../generalModelPersistence";
import { formatEngineeringNumber } from "../chartContract";
import { axis, chartTheme, engineeringLine, initEngineeringChart } from "./chartPrimitives";
import { registerActiveChartActionTarget } from "./activeChartActionRouter";
import { executeChartToolbarAction } from "./chartToolbarActions";
import * as echarts from "echarts/core";
import { LineChart } from "echarts/charts";
import { GridComponent, TooltipComponent, AriaComponent, DataZoomComponent } from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
echarts.use([LineChart, GridComponent, TooltipComponent, AriaComponent, DataZoomComponent, CanvasRenderer]);
export function initializeGeneralModelWorkflow(post: (kind: string, payload?: Record<string, unknown>) => void): void {
  const file = document.querySelector<HTMLInputElement>("#general-model-file");
  if (!file)
    return;
  const status = document.querySelector<HTMLElement>("[data-general-model-status]")!;
  const analyze = document.querySelector<HTMLButtonElement>("[data-general-model-analyze]")!;
  const cancel = document.querySelector<HTMLButtonElement>("[data-general-model-cancel]")!;
  const save = document.querySelector<HTMLButtonElement>("[data-general-model-save]")!;
  const section = document.querySelector<HTMLElement>("[data-general-model-result]")!;
  const picker = document.querySelector<HTMLSelectElement>("[data-general-model-channel]")!;
  const feedbackRequest = initializeGeneralFeedbackWorkflow(post);
  let saved: SavedGeneralModelAnalysis | undefined;
  let hasModel = false;
  let magnitude: echarts.ECharts | undefined;
  let phase: echarts.ECharts | undefined;
  const toolbarWindows = new Map<string, [number, number]>();
  registerActiveChartActionTarget({ plotIds: ["general-model"], handle: (action, plotId) => executeChartToolbarAction(
    action, plotId, [magnitude, phase].filter((chart): chart is echarts.ECharts => Boolean(chart)),
    document.querySelector<HTMLElement>("[data-general-model-magnitude]"), toolbarWindows) });
  file.addEventListener("change", async () => {
    const selected = file.files?.[0];
    if (!selected)
      return;
    if (selected.size > 256000) {
      status.textContent = "Choose a JSON file of at most 256 kB.";
      return;
    }
    status.textContent = "Importing model…";
    save.disabled = true;
    try {
      post("control-studio.general-model.import", { json: await selected.text() });
    }
    catch (error) {
      status.textContent = String(error);
    }
  });
  analyze.addEventListener("click", () => {
    const value = (id: string) => Number(document.querySelector<HTMLInputElement>(id)!.value);
    post("control-studio.general-model.analyze", { minimumFrequency: value("#general-model-min"), maximumFrequency: value("#general-model-max"), points: value("#general-model-points"), ...feedbackRequest() });
  });
  for (const selector of ["#general-model-min", "#general-model-max", "#general-model-points"]) {
    document.querySelector(selector)?.addEventListener("input", () => post("control-studio.general-model.invalidate"));
  }
  cancel.addEventListener("click", () => post("control-studio.general-model.cancel"));
  save.addEventListener("click", () => post("control-studio.result.save"));
  function render(): void {
    if (!saved)
      return;
    const channel = saved.result.channels[Number(picker.value) || 0];
    if (!channel)
      return;
    const theme = chartTheme();
    const units = `${channel.output.unit} / ${channel.input.unit}`;
    magnitude ??= initEngineeringChart("[data-general-model-magnitude]");
    phase ??= initEngineeringChart("[data-general-model-phase]");
    const options = (label: string, data: (number | null)[][]) => ({ animation: false, backgroundColor: theme.background, textStyle: { color: theme.foreground },
      aria: { enabled: true, description: `${channel.input.id} (${channel.input.unit}) to ${channel.output.id} (${channel.output.unit}): ${label}` },
      grid: { left: 75, right: 30, top: 25, bottom: 60 }, dataZoom: [{ type: "inside", filterMode: "none" }], tooltip: { trigger: "axis" }, xAxis: axis("Frequency (rad/s)", "log", theme), yAxis: axis(label, "value", theme),
      series: [{ ...engineeringLine("Supplied plant", 0, [], "solid"), data }] });
    magnitude.setOption(options(`Magnitude (dB re 1 ${units})`, channel.points.map(p => [p.frequencyRadPerSecond, p.magnitude > 0 ? 20 * Math.log10(p.magnitude) : null])), true);
    phase.setOption(options("Phase (°)", channel.points.map(p => [p.frequencyRadPerSecond, p.phaseRadians * 180 / Math.PI])), true);
    magnitude.resize();
    phase.resize();
    const table = document.querySelector<HTMLElement>("[data-general-model-table]")!;
    table.replaceChildren();
    for (const p of channel.points) {
      const row = document.createElement("tr");
      for (const value of [p.frequencyRadPerSecond, p.real, p.imaginary, p.magnitude, p.phaseRadians * 180 / Math.PI]) {
        const cell = document.createElement("td");
        cell.textContent = formatEngineeringNumber(value);
        row.append(cell);
      }
      table.append(row);
    }
  }
  picker.addEventListener("change", render);
  window.addEventListener("trust-control-studio-stage", () => requestAnimationFrame(render));
  const resize = new ResizeObserver(() => { magnitude?.resize(); phase?.resize(); });
  for (const selector of ["[data-general-model-magnitude]", "[data-general-model-phase]"]) resize.observe(document.querySelector(selector)!);
  window.addEventListener("trust-control-studio-plot-shown", event => {
    if ((event as CustomEvent<{ id: string }>).detail.id === "general-model") requestAnimationFrame(render);
  });
  new MutationObserver(() => render()).observe(document.body, { attributes: true, attributeFilter: ["class", "data-vscode-theme-id"] });
  window.addEventListener("trust-control-studio-general-model", event => {
    const payload = (event as CustomEvent).detail as Record<string, unknown>;
    const state = String(payload.status);
    if (["imported", "complete", "inspection"].includes(state)) hasModel = true;
    if (state === "inspection" && saved?.model.revision === (payload.saved as SavedGeneralModelAnalysis | undefined)?.model.revision) return;
    if (["running", "failed", "cancelled", "controller-imported", "controller-cleared", "edited"].includes(state)) {
      saved = undefined;
      section.hidden = true;
    }
    cancel.disabled = state !== "running";
    analyze.disabled = !hasModel || state === "running";
    save.disabled = state !== "complete";
    if (state === "imported") {
      const model = payload.model as SavedGeneralModelAnalysis["model"];
      saved = undefined;
      section.hidden = true;
      status.textContent = `Imported ${model.revision}: ${model.inputs.length} inputs, ${model.outputs.length} outputs · ${model.validation}.`;
    }
    else if (state === "complete" || state === "inspection") {
      saved = payload.saved as SavedGeneralModelAnalysis;
      renderGeneralStateStructure(saved);
      document.querySelector<HTMLInputElement>("#general-model-min")!.value = String(saved.grid.minimumFrequency);
      document.querySelector<HTMLInputElement>("#general-model-max")!.value = String(saved.grid.maximumFrequency);
      document.querySelector<HTMLInputElement>("#general-model-points")!.value = String(saved.grid.points);
      section.hidden = false;
      status.textContent = state === "inspection" ? "Model preview. Run Analyze to save a frequency study."
        : "Model analysis complete. Prediction only; no controller was applied.";
      document.querySelector<HTMLElement>("[data-general-model-provenance]")!.textContent = `${saved.model.source} model ${saved.model.revision} · ${saved.model.validation} · delay ${saved.model.delaySeconds} s`;
      picker.replaceChildren();
      saved.result.channels.forEach((c, i) => { const option = document.createElement("option"); option.value = String(i); option.textContent = `${c.input.id} (${c.input.unit}) → ${c.output.id} (${c.output.unit})`; picker.append(option); });
      document.querySelector<HTMLElement>("[data-general-model-poles]")!.textContent = `Model poles: ${saved.result.poles.map(p => `${formatEngineeringNumber(p.real)} ${p.imaginary < 0 ? "−" : "+"} ${formatEngineeringNumber(Math.abs(p.imaginary))}j`).join(", ")}. ${saved.result.poleScope}`;
      document.querySelector<HTMLElement>("[data-general-model-rga]")!.textContent = saved.result.relativeGainMatrix ? `DC relative gain: ${saved.result.relativeGainMatrix.map(row => row.map(v => formatEngineeringNumber(v)).join(", ")).join("; ")}` : saved.result.capabilities.relativeGain.reason ?? "Relative gain unavailable.";
      requestAnimationFrame(() => {
        render();
        window.dispatchEvent(new CustomEvent("trust-control-studio-general-model-rendered", {
          detail: { available: Boolean(saved && magnitude && phase) },
        }));
      });
    }
    else
      status.textContent = String(payload.reason ?? state);
  });
}
