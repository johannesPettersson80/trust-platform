import { LineChart } from "echarts/charts";
import { AriaComponent, GridComponent, LegendComponent, TooltipComponent } from "echarts/components";
import * as echarts from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";
import { axis, chartTheme } from "./chartPrimitives";
import type { StudioPage } from "./threePageHostState";
import { displayEngineeringUnit } from "../chartContract";
import type { LiveSides, LoopDefinition, PlotSignal } from "../generated/OpenResult";
import { ENGINE_OPENED_EVENT } from "./engineShared";
import { openedOffers, setOpenedOffers } from "./openedOffers";
import { experimentInputBinding, recordedSignalResponses, type ChartSample } from "./threePageRecordedSignals";
import { responseChartSeries, responseChartTooltip } from "./threePageResponseSeries";
import { responseRowAxis, responseRowGrids } from "./responseRows";
import { ThreePageLiveSignals, type LiveSample, type SignalResponseGroup } from "./threePageLiveSignals";
import { signalRoleName, signalRoleTitle } from "./signalRoles";

type ResponseGroup = SignalResponseGroup;

/** One trace of an experiment on the engine's time base: the applied input or the measured output. A null value is a gap (B10.3). */
interface ExperimentTrace {
  id: string;
  unit: string;
  points: readonly { timeSeconds: number; value: number | null }[];
}

/**
 * What the chart reads of an identification's presentation message: the measured and predicted samples on the engine's time base,
 * the output they belong to, and the applied inputs the experiment held when the engine's plot has them (`engineIdentify.ts` repackages
 * `IdentifyResult.plot`; the presentation of a Recorded-data fit holds the samples alone).
 */
interface ChartPresentation {
  predictedResponse?: readonly { timeSeconds: number; measured: number | null; predicted: number | null }[];
  outputId?: string;
  experiment?: { inputs: readonly ExperimentTrace[]; outputs: readonly ExperimentTrace[] };
  units: { output: string };
}

/** The predicted response of a Tune answer or manual preview (`predictedPlots` of `engineTune.ts`): the candidate's and the current settings' measurement. */
interface PredictedPlots {
  timeResponse?: readonly { timeSeconds: number; candidatePv: number | null; currentPv?: number | null }[];
}

/** The `loop.monitor.snapshot` message the panel posts: the engine's sample (`MonitorResult.sample`) with the scope it belongs to (B2). */
interface LoopMonitorEnvelope {
  kind: string;
  projectId: string;
  editorSessionId: string;
  loopId: string;
  expectedLoopRevision: string;
  payload: LiveSample;
}

echarts.use([LineChart, GridComponent, LegendComponent, TooltipComponent, AriaComponent, CanvasRenderer]);
const host = document.querySelector<HTMLElement>("[data-control-studio-chart]");
if (host) {
  const root = document.querySelector<HTMLElement>("[data-studio-three-page]")!;
  const loop = JSON.parse(root.dataset.studioLoop ?? "null") as LoopDefinition | null;
  const chart = echarts.init(host, undefined, { renderer: "canvas" });
  const empty = root.querySelector<HTMLElement>("[data-studio-chart-empty]");
  const plots: Partial<Record<StudioPage, ResponseGroup[]>> = {};
  // The Validate plot's title as the engine names it; the page's own word stands until an engine plot arrives.
  let measuredTitle: string | undefined;
  let live = loop ? new ThreePageLiveSignals() : undefined;
  let recordedSource = false;
  let livePage: StudioPage = "model";
  const selected = (kind: string) => Array.from(root.querySelector<HTMLSelectElement>(`[data-studio-${kind}]`)?.selectedOptions ?? [], option => option.value);
  /** The signals the engine lists for the plots (`liveSides.plotSignals`, B9.6); none until its open answer has arrived. */
  const plotSignals = (): readonly PlotSignal[] => openedOffers().liveSides?.plotSignals ?? [];
  /** The sides the engine names the loop's roles from (`liveSides`, B3.6); none until its open answer has arrived. */
  const liveSides = (): LiveSides | null | undefined => openedOffers().liveSides;
  let page: StudioPage = "model";
  let recording: readonly ChartSample[] = [];
  let selectedSignals: readonly string[] = [];
  const render = () => {
    const theme = chartTheme();
    const groups = plots[page] ?? (page === livePage && !recordedSource ? live?.responses(selected("inputs"), selected("outputs"),
      root.querySelector<HTMLSelectElement>("[data-engineer-acquisition-method]")?.value === "project"
        ? Array.from(root.querySelector<HTMLSelectElement>("[data-three-page-experiment-signals]")?.selectedOptions ?? [], option => option.value) : undefined) : undefined) ?? [];
    // Tune has no plot before its first prediction; Identify and Validate keep their plot area and say what it waits for.
    host.closest<HTMLElement>(".studio-response")!.hidden = groups.length === 0 && page === "tune";
    if (empty) { empty.hidden = groups.length > 0; empty.textContent = groups.length ? "" : live?.emptyMessage() ?? "Start the runtime to see live values"; }
    const title = root.querySelector<HTMLElement>("[data-studio-response-title]")!;
    title.textContent = page === "tune" ? "Predicted response" : page === "verify" ? measuredTitle ?? "Measured response" : "";
    title.hidden = !groups.length || !title.textContent;
    const rows: ResponseGroup[] = groups.length ? groups : [{ name: "", unit: "", series: [] }];
    // The Apply block sits above the plot on Validate, so the plot needs no room for it.
    const applicationHeight = 24;
    const available = window.innerHeight - (host.getBoundingClientRect().top + window.scrollY) - applicationHeight;
    const minimumRowHeight = page === "model" && rows.length > 1 ? 180 : 240;
    host.style.minHeight = `${rows.length * minimumRowHeight}px`;
    host.style.height = `${Math.max(rows.length * minimumRowHeight, available)}px`;
    const height = Math.max(host.clientHeight, rows.length * minimumRowHeight);
    const times = groups.flatMap(group => group.series.flatMap(series => series.points.map(point => point[0]).filter((time): time is number => time !== null)));
    const starts = groups.flatMap(group => group.timeStart === undefined ? [] : [group.timeStart]);
    const timeRange = times.length ? [...times, ...starts].reduce(([low, high], time) => [Math.min(low, time), Math.max(high, time)], [Infinity, -Infinity]) : undefined;
    const series = responseChartSeries(theme, plotSignals(), groups);
    const labels = new Map(groups.flatMap(group => group.series.map(item => [item.name, item.label ?? item.name] as const)));
    const description = groups.length ? `Response plot: ${[...labels.values()].join(", ")}. ${groups.map(group => `${group.name} (${displayEngineeringUnit(group.unit)})`).join(", ")}. Time in seconds.` : "Response plot";
    if (!groups.length) chart.clear();
    else chart.setOption({
      animation: false, aria: { enabled: true, description },
      backgroundColor: theme.background,
      color: [theme.pv, theme.auxiliaryOrange, theme.sp, theme.auxiliaryPurple],
      legend: { show: series.length > 0, top: 0, textStyle: { color: theme.foreground }, type: "scroll", formatter: (name: string) => labels.get(name) ?? name },
      tooltip: responseChartTooltip(),
      grid: responseRowGrids(rows.length, height),
      xAxis: rows.map((group, index) => ({ ...axis("Time (s)", "value", theme),
        ...(timeRange ? { min: timeRange[0], max: timeRange[1] } : {}), show: groups.length > 0, gridIndex: index })),
      yAxis: rows.map((group, index) => {
        const values = group.series.flatMap(item => item.points.map(point => point[1]));
        // A stationary zero-percent actuator needs a readable engineering scale,
        // not millionths of a percent. Moving responses retain fit-to-data axes.
        const zeroSpan = group.zeroSpan && values.some(value => value === 0) && values.every(value => value === null || value === 0) ? group.zeroSpan : undefined;
        return responseRowAxis(group.name ? `${group.name} (${displayEngineeringUnit(group.unit)})` : "", theme,
          zeroSpan ? [-zeroSpan, zeroSpan] : values, index, groups.length > 0, height / rows.length);
      }),
      series,
    }, { notMerge: true });
    chart.resize();
    host.dataset.seriesCount = String(series.length);
    host.dataset.timeRange = JSON.stringify(timeRange ?? []);
    host.dataset.pointCount = String(groups.reduce((sum, group) => sum + group.series.reduce((count, item) => count + item.points.length, 0), 0));
    host.setAttribute("aria-label", description);
  };
  window.addEventListener("message", event => {
    const envelope = event.data as Partial<LoopMonitorEnvelope> | undefined;
    if (!live || envelope?.kind !== "loop.monitor.snapshot" || !envelope.payload) return;
    // A sample of another session, loop or revision is not this page's (B2, A8.6).
    if (envelope.projectId !== root.dataset.projectId || envelope.editorSessionId !== root.dataset.editorSessionId
      || envelope.loopId !== root.dataset.loopId || envelope.expectedLoopRevision !== root.dataset.loopRevision) return;
    live.receive(envelope.payload);
    if (page === livePage && !plots[page] && !recordedSource) render();
  });
  for (const select of root.querySelectorAll("[data-studio-inputs],[data-studio-outputs],[data-engineer-acquisition-method]")) {
    select.addEventListener("change", render);
    new MutationObserver(render).observe(select, { childList: true, subtree: true });
  }
  window.addEventListener("trust-control-studio-engineer", event => {
    const payload = (event as CustomEvent<Record<string, unknown>>).detail;
    if (payload.liveIdentification === true && loop) {
      delete plots.model; recordedSource = false; livePage = "model"; live = new ThreePageLiveSignals();
    }
    if (payload.recordingCatalog) recordedSource = true;
    // A reset drops the Tune and Validate plots with the model they were drawn for (the Identify page specification, Reset).
    if (["edited", "reset"].includes(String(payload.status))) { delete plots.tune; delete plots.verify; }
    // A new candidate or manual preview replaces the Tune plot; one without a predicted response shows none (B4.5). A payload that
    // carries its plots draws them after this, so the prediction of the settings on the page is the one shown.
    if (payload.status === "candidate" || payload.status === "preview") delete plots.tune;
    const presentation = payload.identificationPresentation as ChartPresentation | undefined;
    if (presentation?.predictedResponse?.length) {
      const output = { id: presentation.outputId ?? loop?.signals.processVariable.symbol ?? "Measured output",
        unit: presentation.units.output, points: presentation.predictedResponse };
      const experiment = presentation.experiment;
      const retained = loop && recording.length ? recordedSignalResponses(plotSignals(), recording, selectedSignals) : [];
      const retainedSeries = retained.flatMap(group => group.series);
      const measurement = plotSignals().find(signal => signal.kind === "measurement");
      plots.model = [{ name: signalRoleName(liveSides(), output.id) ?? output.id, unit: output.unit, series: [
        { signalId: plotSignals().find(signal => signal.symbol === output.id)?.id, name: "Measured", points: experiment?.outputs[0]?.points.map(point => [point.timeSeconds, point.value])
          ?? retainedSeries.find(series => series.name === output.id || (measurement !== undefined && series.signalId === measurement.id))?.points
          ?? output.points.map(point => [point.timeSeconds, point.measured]) },
        { prediction: true, name: "Model prediction", points: output.points.map(point => [point.timeSeconds, point.predicted]), dashed: true },
      ] }];
      if (experiment) { plots.model.push(...appliedInputs(experiment.inputs, liveSides())); recordedSource = true; }
      else if (retained.length) plots.model.push(...retained.map(group => ({ ...group,
        series: group.series.filter(series => selected("inputs").includes(series.name)) })).filter(group => group.series.length > 0));
    }
    const prediction = payload.plots as PredictedPlots | undefined;
    if (loop && prediction?.timeResponse?.length) {
      const points = prediction.timeResponse;
      plots.tune = [{ name: signalRoleName(liveSides(), loop.signals.processVariable.symbol) ?? loop.signals.processVariable.symbol, unit: loop.signals.processVariable.unit, series: [
        { name: "Tuned", points: points.map(point => [point.timeSeconds, point.candidatePv]) },
        ...(points.some(point => typeof point.currentPv === "number") ? [{ name: "Current", dashed: true, points: points.map(point => [point.timeSeconds, point.currentPv ?? null]) }] : []),
      ] }];
    }
    // The engine's Validate plot, named and bounded by Rust (B5.4, B9.6); a plot the engine withdraws leaves no older curve.
    if (payload.validationPlot !== undefined) {
      const measured = payload.validationPlot as { title: string; groups: ResponseGroup[] } | null;
      if (measured) { plots.verify = measured.groups; measuredTitle = measured.title; } else delete plots.verify;
    }
    render();
  });
  // The engine's open answer names the signals the plots draw and the role of each (B9.6, B9.9); the chart draws from it once the answer has
  // arrived. The full symbols of the loop's signals show on hover; the legend and axes name them by role.
  window.addEventListener(ENGINE_OPENED_EVENT, event => {
    const opened = (event as CustomEvent<{ liveSides?: LiveSides | null }>).detail;
    setOpenedOffers({ liveSides: opened.liveSides ?? null });
    host.title = signalRoleTitle(liveSides());
    render();
  });
  window.addEventListener("trust-control-studio-three-page", event => {
    page = (event as CustomEvent<{ page: StudioPage }>).detail.page;
    render();
  });
  window.addEventListener("trust-control-studio-experiment-started", () => {
    if (!loop) return;
    recording = []; delete plots.model; recordedSource = false; livePage = "model";
    live = new ThreePageLiveSignals(); render();
  });
  const showRecording = (samples: readonly ChartSample[], signals: readonly string[]) => {
    recording = samples; selectedSignals = signals; recordedSource = true;
    if (loop) plots.model = recordedSignalResponses(plotSignals(), recording, selectedSignals);
    render();
  };
  window.addEventListener("trust-control-studio-experiment-recording", event => {
    const detail = (event as CustomEvent<{ recording: { rawSamples?: readonly ChartSample[] }; signalIds: readonly string[] }>).detail;
    showRecording(detail.recording.rawSamples ?? [], detail.signalIds);
  });
  window.addEventListener("trust-control-studio-experiment-signals", event => {
    selectedSignals = (event as CustomEvent<{ signalIds: readonly string[] }>).detail.signalIds;
    if (loop && recording.length) plots.model = recordedSignalResponses(plotSignals(), recording, selectedSignals);
    render();
  });
  render();
  new ResizeObserver(() => chart.resize()).observe(host);
  window.addEventListener("resize", render);
  new MutationObserver(render).observe(document.body, { attributes: true, attributeFilter: ["class", "style", "data-vscode-theme-kind"] });
}

/** The applied inputs of an experiment, one group for each unit: the plant input it held, drawn beside the measurement and the prediction (B9.6).
 *  Its axis names it by its role in the loop, as its legend does (B9.9); plant inputs of one unit share the axis as "Output". */
function appliedInputs(traces: readonly ExperimentTrace[], sides: LiveSides | null | undefined): ResponseGroup[] {
  const groups = new Map<string, ResponseGroup>();
  for (const trace of traces) {
    const applied = experimentInputBinding(sides?.plotSignals ?? [], trace.id);
    const name = applied ? signalRoleName(sides, applied.symbol) ?? applied.symbol : `${trace.id} · Applied input`;
    const shared = groups.get(trace.unit);
    const group: ResponseGroup = shared ? { ...shared, name: "Output" } : { name, unit: trace.unit, series: [] };
    group.series.push({ signalId: applied?.id, name, points: trace.points.map(point => [point.timeSeconds, point.value]) });
    groups.set(trace.unit, group);
  }
  return [...groups.values()];
}
