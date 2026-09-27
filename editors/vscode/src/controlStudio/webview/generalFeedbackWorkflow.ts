import type { GeneralModel } from "../generalModel";
import type { SavedGeneralFeedback } from "../generalFeedbackPersistence";
import type { SavedGeneralModelAnalysis } from "../generalModelPersistence";
import { formatEngineeringNumber } from "../chartContract";
import { axis, chartTheme, engineeringLine, initEngineeringChart } from "./chartPrimitives";
import * as echarts from "echarts/core";
import { MarkPointComponent } from "echarts/components";
echarts.use([MarkPointComponent]);
export function initializeGeneralFeedbackWorkflow(post: (kind: string, payload?: Record<string, unknown>) => void): () => Record<string, string> {
  const file = document.querySelector<HTMLInputElement>("#general-controller-file");
  if (!file) return () => ({});
  const plantPicker = document.querySelector<HTMLSelectElement>("[data-feedback-plant-channel]")!;
  const controllerPicker = document.querySelector<HTMLSelectElement>("[data-feedback-controller-channel]")!;
  const status = document.querySelector<HTMLElement>("[data-general-controller-status]")!;
  const section = document.querySelector<HTMLElement>("[data-general-feedback-result]")!;
  const clear = document.querySelector<HTMLButtonElement>("[data-feedback-clear]")!;
  clear.hidden = true;
  let plant: GeneralModel | undefined; let controller: GeneralModel | undefined; let saved: SavedGeneralFeedback | undefined;
  let magnitude: echarts.ECharts | undefined; let phase: echarts.ECharts | undefined; let nyquist: echarts.ECharts | undefined;
  function populate(picker: HTMLSelectElement, model: GeneralModel): void {
    picker.replaceChildren(); picker.disabled = false;
    model.outputs.forEach(output => model.inputs.forEach(input => {
      const option = document.createElement("option"); option.value = JSON.stringify([input.id, output.id]);
      option.textContent = `${input.id} (${input.unit}) → ${output.id} (${output.unit})`; picker.append(option);
    }));
  }
  file.addEventListener("change", async () => {
    const selected = file.files?.[0]; if (!selected) return;
    if (selected.size > 256000) { status.textContent = "Choose controller JSON of at most 256 kB."; return; }
    try { post("control-studio.general-model.import-controller", { json: await selected.text() }); }
    catch (error) { status.textContent = String(error); }
  });
  clear.addEventListener("click", () => post("control-studio.general-model.clear-controller"));
  for (const picker of [plantPicker, controllerPicker]) picker.addEventListener("change", () => post("control-studio.general-model.invalidate"));
  function render(): void {
    if (!saved || section.hidden) return;
    const points = saved.result.openLoop; const theme = chartTheme();
    magnitude ??= initEngineeringChart("[data-feedback-magnitude]"); phase ??= initEngineeringChart("[data-feedback-phase]"); nyquist ??= initEngineeringChart("[data-feedback-nyquist]");
    const common = { animation: false, backgroundColor: theme.background, textStyle: { color: theme.foreground }, color: [theme.pv, theme.muted], aria: { enabled: true }, grid: { left: 72, right: 28, top: 30, bottom: 60 }, tooltip: { trigger: "axis" } };
    magnitude.setOption({ ...common, xAxis: axis("Frequency (rad/s)", "log", theme), yAxis: axis("Open-loop magnitude (dB)", "value", theme),
      series: [{ ...engineeringLine("G × C", 0, [], "solid"), data: points.map(p => [p.frequencyRadPerSecond, p.magnitude > 0 ? 20 * Math.log10(p.magnitude) : null]) }] }, true);
    phase.setOption({ ...common, xAxis: axis("Frequency (rad/s)", "log", theme), yAxis: axis("Open-loop phase (° wrapped)", "value", theme),
      series: [engineeringLine("G × C", 0, points.map(p => [p.frequencyRadPerSecond, p.phaseRadians * 180 / Math.PI]), "solid")] }, true);
    const first = points[0]; const last = points[points.length - 1];
    nyquist.setOption({ ...common, tooltip: { trigger: "item" }, xAxis: axis("Real", "value", theme), yAxis: axis("Imaginary", "value", theme), series: [
      { ...engineeringLine("Positive frequency (increasing)", 0, points.map(p => [p.real, p.imaginary]), "solid"), markPoint: { symbolSize: 18, label: { color: theme.foreground, position: "right", formatter: "{b}" }, data: [
        { name: `start ω=${formatEngineeringNumber(first.frequencyRadPerSecond)}`, coord: [first.real, first.imaginary] },
        { name: `end ω=${formatEngineeringNumber(last.frequencyRadPerSecond)}`, coord: [last.real, last.imaginary], label: { position: "left" } }] } },
      engineeringLine("Negative-frequency conjugate", 0, points.map(p => [p.real, -p.imaginary]), "dashed"),
      { name: "−1 reference", type: "line", showSymbol: true, symbol: "circle", symbolSize: 8, label: { show: true, formatter: "−1", position: "right", color: theme.foreground }, lineStyle: { width: 0 }, data: [[-1, 0]], itemStyle: { color: theme.foreground } }
    ] }, true);
    magnitude.resize(); phase.resize(); nyquist.resize();
  }
  new ResizeObserver(() => { magnitude?.resize(); phase?.resize(); nyquist?.resize(); }).observe(section);
  window.addEventListener("trust-control-studio-stage", () => requestAnimationFrame(render));
  new MutationObserver(render).observe(document.body, { attributes: true, attributeFilter: ["class", "data-vscode-theme-id"] });
  window.addEventListener("trust-control-studio-general-model", event => {
    const payload = (event as CustomEvent).detail as Record<string, unknown>;
    if (payload.status === "imported") { plant = payload.model as GeneralModel; populate(plantPicker, plant); }
    if (payload.status === "controller-imported") { controller = payload.model as GeneralModel; populate(controllerPicker, controller); status.textContent = `Attached ${controller.revision} · ${controller.validation}. Select inverse-unit channels.`; }
    if (payload.status === "controller-cleared") { controller = undefined; file.value = ""; controllerPicker.replaceChildren(); controllerPicker.disabled = true; status.textContent = "No controller attached: plant-only analysis."; }
    clear.hidden = !controller;
    if (payload.status !== "complete") { saved = undefined; section.hidden = true; return; }
    const modelAnalysis = payload.saved as SavedGeneralModelAnalysis;
    plant = modelAnalysis.model; populate(plantPicker, plant); saved = modelAnalysis.feedback;
    if (!saved) { section.hidden = true; return; }
    controller = saved.definition.controller; populate(controllerPicker, controller);
    clear.hidden = false;
    plantPicker.value = JSON.stringify([saved.definition.plantChannel.inputId, saved.definition.plantChannel.outputId]);
    controllerPicker.value = JSON.stringify([saved.definition.controllerChannel.inputId, saved.definition.controllerChannel.outputId]);
    status.textContent = `Attached ${controller.revision} · ${controller.validation}.`;
    section.hidden = false;
    document.querySelector<HTMLElement>("[data-feedback-provenance]")!.textContent = `Plant ${plant.revision} (${plant.source}, ${plant.validation}); controller ${controller.revision} (${controller.source}, ${controller.validation}). Negative feedback; other inputs fixed. Total pure delay ${plant.delaySeconds + controller.delaySeconds} s.`;
    const closed = saved.result.closedLoop;
    document.querySelector<HTMLElement>("[data-general-feedback-poles]")!.textContent = closed.status === "qualified"
      ? `Closed-loop poles: ${closed.poles.map(p => `${formatEngineeringNumber(p.real)} ${p.imaginary < 0 ? "−" : "+"} ${formatEngineeringNumber(Math.abs(p.imaginary))}j`).join(", ") || "none (static interconnection)"}. ${closed.stable ? "Strictly stable for this declared realization." : "Not strictly stable within numerical tolerance (unstable or boundary pole)."} ${closed.scope}`
      : `Closed-loop poles unavailable: ${closed.reason}`;
    const margins = saved.result.margins;
    document.querySelector<HTMLElement>("[data-feedback-margins]")!.textContent = margins.status === "estimated"
      ? `Margin estimate: gain crossover ${formatEngineeringNumber(margins.gainCrossoverRadPerSecond)} rad/s; phase margin ${formatEngineeringNumber(margins.phaseMarginDegrees)}°. Gain margin ${margins.gainMarginDb === undefined ? "unavailable" : `${formatEngineeringNumber(margins.gainMarginDb)} dB`}. ${margins.scope}`
      : `Margin estimate unavailable: ${margins.reason}`;
    document.querySelector<HTMLElement>("[data-feedback-nyquist-scope]")!.textContent = saved.result.nyquistScope;
    const table = document.querySelector<HTMLElement>("[data-feedback-table]")!; table.replaceChildren();
    for (const point of saved.result.openLoop) {
      const row = document.createElement("tr"); for (const v of [point.frequencyRadPerSecond, point.real, point.imaginary, point.magnitude, point.phaseRadians * 180 / Math.PI]) { const cell = document.createElement("td"); cell.textContent = formatEngineeringNumber(v); row.append(cell); } table.append(row);
    }
    requestAnimationFrame(render);
  });
  return (): Record<string, string> => {
    if (!plant || !controller || !plantPicker.value || !controllerPicker.value) return {};
    const [plantInputId, plantOutputId] = JSON.parse(plantPicker.value) as string[];
    const [controllerInputId, controllerOutputId] = JSON.parse(controllerPicker.value) as string[];
    return { plantInputId, plantOutputId, controllerInputId, controllerOutputId };
  };
}
