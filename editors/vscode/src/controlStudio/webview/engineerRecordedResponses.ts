import { RecordedResponseInteraction, type RecordedResponsePlot } from "./recordedResponseInteraction";
import { commissioningRecordedResponse } from "./commissioningRecordedResponse";
import type { CommissioningResult } from "../proportionalCommissioning";
import * as echarts from "echarts/core";
import { LineChart } from "echarts/charts";
import { AriaComponent, DataZoomComponent, GridComponent, LegendComponent, MarkLineComponent, TooltipComponent } from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";

import { formatEngineeringNumber } from "../chartContract";
import { axis, chartTheme, engineeringLine, formatChartValue, initEngineeringChart } from "./chartPrimitives";

echarts.use([LineChart, AriaComponent, DataZoomComponent, GridComponent, LegendComponent, MarkLineComponent, TooltipComponent, CanvasRenderer]);

type Quality = "good" | "bad" | "forced" | "missing";
type RecordedSample = { timeSeconds: number; values: Record<string, { value: number | null; quality: Quality }> };
type RecordedRun = { evidence: string; channels: readonly { id: string; unit: string }[]; samples: readonly RecordedSample[];
  channelRoles?: { setpoint: string; processVariable: string; manipulatedVariable: string };
  conditions: { modelRevision: string; programRevision: string; events?: readonly { atSeconds: number; channel: string; value: number }[] } };
export type Evaluation = { status: string; run: RecordedRun; scope?: string; eventDescription?: string; specification?: readonly {
  id?: string; kind: string; channel: string; reference?: number | { channel: string } | { value: number }; minimum?: number; maximum?: number; window: { start: number; end: number };
}[] };

export interface RecordedResponseChannels {
  readonly setpoint?: { id: string; unit: string };
  readonly processVariable?: { id: string; unit: string };
  readonly manipulatedVariable?: { id: string; unit: string };
}

/** Resolves SP/PV/MV from the frozen requirement identities, with legacy order fallback. */
export function recordedResponseChannels(run: RecordedRun, evaluation: Pick<Evaluation, "specification">): RecordedResponseChannels {
  const byId = (id: string | undefined) => id ? run.channels.find(channel => channel.id === id) : undefined;
  if (run.channelRoles) return { setpoint: byId(run.channelRoles.setpoint),
    processVariable: byId(run.channelRoles.processVariable), manipulatedVariable: byId(run.channelRoles.manipulatedVariable) };
  const requirements = evaluation.specification ?? [];
  const processId = requirements.find(requirement => ["error", "settling", "overshoot"].includes(requirement.kind))?.channel
    ?? requirements.find(requirement => requirement.id?.includes("process-bounds"))?.channel;
  const reference = requirements.find(requirement => requirement.kind === "error" && typeof requirement.reference === "object"
    && requirement.reference !== null && "channel" in requirement.reference)?.reference;
  const outputId = requirements.find(requirement => requirement.id?.includes("output-bounds") || requirement.id?.includes("requested-output-bounds")
    || requirement.id?.includes("output-rate") || ["levels", "dwell", "saturation"].includes(requirement.kind))?.channel;
  const setpoint = byId(reference && typeof reference === "object" && "channel" in reference ? reference.channel : undefined);
  const processVariable = byId(processId);
  const manipulatedVariable = byId(outputId);
  if (!setpoint && !processVariable && !manipulatedVariable && run.channels.length === 3 && requirements.length === 0) {
    return { setpoint: run.channels[0], processVariable: run.channels[1], manipulatedVariable: run.channels[2] };
  }
  return {
    setpoint,
    processVariable,
    manipulatedVariable,
  };
}

/** Renders immutable verification records in the Tune workspace, leaving the header as an action row. */
export class EngineerRecordedResponses {
  private charts: echarts.ECharts[] = [];
  private readonly host: HTMLElement;
  private readonly plots: RecordedResponsePlot[] = [];
  private readonly repaint: Array<() => void> = [];
  private interaction: RecordedResponseInteraction | undefined;
  private readonly toolbar = document.querySelector<HTMLElement>(".plot-toolbar");
  private readonly toolbarHome = this.toolbar?.parentElement;
  private verification: [Evaluation, Evaluation] | undefined;
  private commissioning: CommissioningResult | undefined;

  constructor(section: HTMLElement, private readonly retryVerification: () => void) {
    this.host = document.createElement("section");
    this.host.dataset.engineerRecordedResponses = "";
    this.host.dataset.plotView = "recorded-responses";
    this.host.hidden = true;
    this.host.dataset.engineerResponseWorkspace = "monitor";
    this.host.className = "engineer-recorded-responses";
    this.host.setAttribute("aria-label", "Actual recorded verification responses");
    // Verification is a recorded-response analysis, so it belongs at the top of the
    // existing shared monitor plot workspace, not after the long Tune form or in the header.
    const workspace = document.querySelector<HTMLElement>("[data-control-studio-monitor]");
    if (workspace) workspace.append(this.host);
    else section.append(this.host);
    window.addEventListener("trust-control-studio-plot-shown", event => {
      const selected = (event as CustomEvent<{ id: string; manualStage?: boolean }>).detail;
      // A terminal run with no returned samples has no chart, but its recovery
      // explanation is still a recorded-response result that engineers must see.
      const active = selected.id === "recorded-responses";
      const monitor = this.host.parentElement;
      if (active) {
        monitor?.setAttribute("data-engineer-recorded-response-visible", "true");
        this.host.closest<HTMLElement>(".studio")?.setAttribute("data-engineer-manual-stage", String(selected.manualStage === true));
        if (this.toolbar) monitor?.insertBefore(this.toolbar, this.host);
        requestAnimationFrame(() => this.charts.forEach(chart => chart.resize()));
      } else {
        monitor?.removeAttribute("data-engineer-recorded-response-visible");
        if (this.toolbar && this.toolbarHome) this.toolbarHome.insertBefore(this.toolbar, this.toolbarHome.firstChild);
      }
    });
    window.addEventListener("trust-control-shared-chart-action", event => {
      if (!this.host.hidden && this.interaction?.handle((event as CustomEvent<{ action: string }>).detail.action)) event.preventDefault();
    });
    new ResizeObserver(() => { if (!this.host.hidden) this.charts.forEach(chart => chart.resize()); }).observe(this.host);
    new MutationObserver(() => this.repaint.forEach(update => update())).observe(document.body, { attributes: true, attributeFilter: ["class", "style"] });
  }

  render(baseline: Evaluation, candidate: Evaluation): void {
    this.verification = [baseline, candidate];
    this.resetDisplayed();
    this.host.append(this.heading(baseline, candidate));
    this.sourceChoice("verification");
    for (const [role, evaluation] of [["Baseline", baseline], ["Candidate", candidate]] as const) {
      try { this.host.append(this.plot(role, evaluation)); }
      catch (error) {
        const failure = document.createElement("p"); failure.className = "error";
        failure.textContent = `${role} recorded response could not render: ${error instanceof Error ? error.message : String(error)}`;
        this.host.append(failure);
      }
    }
    this.installInteraction();
  }

  renderScenario(label: string, evaluation: Evaluation): void {
    this.resetDisplayed();
    const title = document.createElement("h3"); title.textContent = `${label} · ${evaluation.status}`;
    this.host.append(title);
    if (this.verification) {
      const back = document.createElement("button"); back.className = "trust-button"; back.textContent = "Controller comparison";
      back.addEventListener("click", () => this.render(...this.verification!)); this.host.append(back);
    }
    this.plot(label, evaluation).style.gridColumn = "1/-1";
    this.installInteraction();
  }

  /** Shows one retained actual-ST prefix without implying a completed comparison. */
  renderInterrupted(evaluation: Evaluation): void {
    if (evaluation.run.samples.length === 0) {
      this.renderUnavailable("The terminal isolated ST run returned no samples; recorded response evidence is unavailable.");
      return;
    }
    this.resetDisplayed();
    const heading = document.createElement("div"); heading.style.gridColumn = "1/-1";
    const title = document.createElement("h3"); title.textContent = "Interrupted actual recorded response";
    const summary = document.createElement("p");
    summary.textContent = `${evaluation.status} · ${evaluation.run.evidence} · ${evaluation.run.samples.length} received samples. This is one interrupted ST run, not a baseline/candidate comparison.`;
    const scope = document.createElement("p"); scope.className = "plot-summary";
    scope.textContent = evaluation.scope ?? "The retained prefix is displayed without inferring an unreceived terminal sample.";
    heading.append(title, summary, scope); this.host.append(heading);
    const card = this.plot("Interrupted", evaluation); card.style.gridColumn = "1/-1";
    this.installInteraction();
  }

  /** Shows an explicit unavailable state when a terminal run has no renderable evidence. */
  renderUnavailable(reason: string): void {
    this.resetDisplayed();
    const title = document.createElement("h3"); title.textContent = "Recorded response unavailable";
    const message = document.createElement("p"); message.textContent = reason;
    const safety = document.createElement("p"); safety.textContent = "Apply remains disabled until a verified response with samples is available.";
    const retry = document.createElement("button"); retry.type = "button"; retry.className = "secondary"; retry.dataset.engineerRetryVerify = "";
    retry.textContent = "Retry Verify";
    retry.addEventListener("click", () => { retry.disabled = true; this.retryVerification(); });
    this.host.append(title, message, safety, retry);
    this.installInteraction();
  }

  renderCommissioning(result: CommissioningResult): void {
    const evaluation = commissioningRecordedResponse(result);
    if (!evaluation) return;
    this.commissioning = result;
    this.resetDisplayed();
    const heading = document.createElement("div"); heading.style.gridColumn = "1/-1";
    const title = document.createElement("h3"); title.textContent = "Actual commissioning recording";
    const summary = document.createElement("p"); summary.textContent = `${result.status} · ${evaluation.run.samples.length} native samples · ${evaluation.run.evidence}.`;
    const identity = document.createElement("details"); identity.dataset.engineerRecordedIdentity = "";
    const identityLabel = document.createElement("summary"); identityLabel.textContent = "Model and program identity";
    const identityValues = document.createElement("p"); identityValues.textContent = `Selected model ${result.modelRevision}; program ${evaluation.run.conditions.programRevision}.`;
    identity.append(identityLabel, identityValues);
    heading.append(title, summary, identity); this.host.append(heading);
    this.sourceChoice("commissioning");
    const card = this.plot("Commissioning", evaluation); card.style.gridColumn = "1/-1";
    this.installInteraction();
  }

  private resetDisplayed(): void {
    this.host.parentElement?.setAttribute("data-engineer-recorded-response-visible", "true");
    this.host.hidden = false;
    this.interaction?.dispose(); this.plots.splice(0); this.repaint.splice(0);
    this.charts.forEach(chart => chart.dispose());
    this.charts = [];
    this.host.replaceChildren();
  }
  private installInteraction(): void {
    const status = document.createElement("p"); status.dataset.engineerResponseStatus = "";
    status.setAttribute("role", "status"); status.style.gridColumn = "1/-1";
    this.host.insertBefore(status, this.host.children[1] ?? null);
    this.interaction = new RecordedResponseInteraction(this.plots, status);
    // The shared workspace owns view routing. It needs a tab for both plots and
    // the explicit no-evidence state; `clear` is the only state that removes it.
    window.dispatchEvent(new CustomEvent("trust-control-recorded-responses", { detail: { available: true, hasPlots: this.plots.length > 0 } }));
  }

  private sourceChoice(selected: "verification" | "commissioning"): void {
    if (!this.verification || !this.commissioning) return;
    const label = document.createElement("label"); label.textContent = "Recording ";
    const select = document.createElement("select"); select.dataset.engineerRecordedSource = "";
    for (const [value, text] of [["verification", "Controller verification"], ["commissioning", "Auto commissioning"]]) {
      const option = document.createElement("option"); option.value = value; option.textContent = text; select.append(option);
    }
    select.value = selected;
    select.addEventListener("change", () => select.value === "verification" ? this.render(...this.verification!) : this.renderCommissioning(this.commissioning!));
    label.append(select); this.host.firstElementChild?.append(label);
  }

  clear(): void {
    this.verification = undefined; this.commissioning = undefined;
    this.interaction?.dispose(); this.interaction = undefined; this.plots.splice(0); this.repaint.splice(0);
    this.host.hidden = true;
    this.host.parentElement?.removeAttribute("data-engineer-recorded-response-visible");
    this.charts.forEach(chart => chart.dispose());
    this.charts = [];
    this.host.replaceChildren();
    window.dispatchEvent(new CustomEvent("trust-control-recorded-responses", { detail: { available: false } }));
  }

  private heading(baseline: Evaluation, candidate: Evaluation): HTMLElement {
    const wrap = document.createElement("div");
    
    wrap.style.gridColumn = "1/-1";
    const summary = document.createElement("p");
    summary.textContent = `Baseline ${baseline.status} · Candidate ${candidate.status} · ${baseline.run.evidence}, matched isolated conditions.`;
    const identity = document.createElement("details");
    const label = document.createElement("summary"); label.textContent = "Model, program and verification scope"; identity.append(label);
    const revisions = document.createElement("p"); revisions.textContent = `Model ${candidate.run.conditions.modelRevision}; program ${candidate.run.conditions.programRevision}.`; identity.append(revisions);
    identity.append(summary);
    const scope = document.createElement("p");
    scope.className = "plot-summary";
    scope.textContent = candidate.scope ?? "Only the declared observed conditions were evaluated.";
    identity.append(scope); wrap.append(identity);
    return wrap;
  }

  private plot(role: string, evaluation: Evaluation): HTMLElement {
    const run = evaluation.run;
    const card = document.createElement("section");
    card.style.minWidth = "0";
    card.dataset.engineerRecordedPlot = role.toLowerCase();
    card.setAttribute("aria-label", `${role} actual recorded response`);
    const title = document.createElement("h4");
    title.textContent = `${role} · ${run.evidence} · ${evaluation.status}`;
    card.append(title);
    const note = document.createElement("p");
    const gaps = run.samples.some(sample => run.channels.some(channel => sample.values[channel.id]?.quality !== "good" || sample.values[channel.id]?.value === null));
    note.textContent = gaps
      ? "A gap means this recorded value is missing or not good quality; the line does not interpolate across it."
      : "No gaps: every displayed recorded value has good quality.";
    const metadata = document.createElement("details");
    const metadataLabel = document.createElement("summary"); metadataLabel.textContent = "Record details";
    metadata.append(metadataLabel);
    if (gaps) card.append(note); else metadata.append(note);
    const chartHost = document.createElement("div");
    chartHost.id = `engineer-recorded-${role.toLowerCase()}`;
    chartHost.className = "engineering-plot time";
    chartHost.setAttribute("role", "img"); chartHost.tabIndex = 0;
    chartHost.setAttribute("aria-label", `${role} recorded SP, PV and MV response`);
    chartHost.style.height = "420px";
    card.append(chartHost);
    // chartPrimitives deliberately resolves by selector so it can share the same creation
    // path as the other engineering plots; attach this card before asking it to resolve.
    this.host.append(card);
    const chart = initEngineeringChart(`#${chartHost.id}`); this.charts.push(chart);
    this.plots.push({ chart, host: chartHost, role, run });
    const resolved = recordedResponseChannels(run, evaluation);
    const process = [resolved.setpoint, resolved.processVariable]; const output = resolved.manipulatedVariable;
    const processUnit = process[1]?.unit ?? process[0]?.unit ?? "engineering unit";
    const outputUnit = output?.unit ?? "engineering unit";
    const values = (channel: { id: string } | undefined) => run.samples.map(sample => point(sample, channel?.id));
    const limits = (evaluation.specification ?? []).filter(requirement => requirement.kind === "bounds" &&
      requirement.channel === output?.id && Number.isFinite(requirement.minimum) && Number.isFinite(requirement.maximum));
    const limitNote = document.createElement("p"); limitNote.dataset.engineerRecordedLimits = "";
    limitNote.textContent = limits.length ? limits.map(limit => `${output!.id}: ${formatEngineeringNumber(limit.minimum)} to ${formatEngineeringNumber(limit.maximum)} ${outputUnit} (${limit.window.start}–${limit.window.end} s).`).join(" ") : "No output bounds declared in this record.";
    const events = run.conditions.events ?? [];
    const eventNote = document.createElement("p"); eventNote.dataset.engineerRecordedEvents = "";
    eventNote.textContent = events.length ? events.map(event => `${event.atSeconds} s: ${event.channel} → ${formatEngineeringNumber(event.value)}.`).join(" ") : "No scheduled events in this record.";
    if (evaluation.eventDescription) eventNote.textContent = `${evaluation.eventDescription} ${eventNote.textContent}`;
    metadata.append(limitNote, eventNote); card.append(metadata);
    const outputValues = values(output).filter((value): value is number => value !== null);
    const boundLines = limits.flatMap(limit => [
      [{ name: `Minimum ${formatEngineeringNumber(limit.minimum)} ${outputUnit}`, coord: [limit.window.start, limit.minimum!] }, { coord: [limit.window.end, limit.minimum!] }],
      [{ name: `Maximum ${formatEngineeringNumber(limit.maximum)} ${outputUnit}`, coord: [limit.window.start, limit.maximum!] }, { coord: [limit.window.end, limit.maximum!] }],
    ]);
    const theme = chartTheme();
    const marks = { symbol: "none", silent: true, lineStyle: { type: "dashed", color: theme.muted }, label: { show: false } };
    chart.setOption({
      animation: false, backgroundColor: theme.background, color: [theme.sp, theme.pv, theme.mv],
      aria: { enabled: true, description: `${role} ${run.evidence} record. SP and PV occupy the upper ${processUnit} lane. MV occupies the separate lower ${outputUnit} lane. ${note.textContent}` },
      axisPointer: { link: [{ xAxisIndex: "all" }] },
      legend: { top: 4, formatter: (name: string) => name.split(" · ")[0], textStyle: { color: theme.muted } },
      tooltip: { trigger: "axis", confine: true, valueFormatter: formatChartValue, axisPointer: { type: "cross", label: { color: theme.foreground } } },
      grid: [{ left: 62, right: 62, top: 38, height: "36%" }, { left: 62, right: 62, top: "57%", bottom: 44 }],
      dataZoom: [{ type: "inside", xAxisIndex: [0, 1], filterMode: "none" }],
      xAxis: [axis("", "value", theme, run.samples.map(sample => sample.timeSeconds)), { ...axis("Time (s)", "value", theme, run.samples.map(sample => sample.timeSeconds)), gridIndex: 1 }],
      yAxis: [axis(`SP / PV (${processUnit})`, "value", theme, [...values(process[0]), ...values(process[1])], { fitToData: true }), { ...axis(`MV (${outputUnit})`, "value", theme, outputValues, { fitToData: true }), gridIndex: 1 }],
      series: [
        { ...engineeringLine(channelLabel("SP", process[0]), 0, points(run, process[0]), "dashed"), xAxisIndex: 0, z: 3, markLine: { ...marks, label: { show: false }, data: events.map(event => ({ name: `${event.channel} → ${formatEngineeringNumber(event.value)}`, xAxis: event.atSeconds })) } },
        { ...engineeringLine(channelLabel("PV", process[1]), 0, points(run, process[1]), "solid"), xAxisIndex: 0 },
        { ...engineeringLine(channelLabel("MV", output), 1, points(run, output), "solid"), xAxisIndex: 1, markLine: { ...marks, data: boundLines } },
      ],
    }, true);
    this.repaint.push(() => {
      const current = chartTheme();
      chart.setOption({ backgroundColor: current.background, color: [current.sp, current.pv, current.mv],
        legend: { textStyle: { color: current.muted } },
        xAxis: [axis("", "value", current, run.samples.map(sample => sample.timeSeconds)),
          { ...axis("Time (s)", "value", current, run.samples.map(sample => sample.timeSeconds)), gridIndex: 1 }],
        yAxis: [axis(`SP / PV (${processUnit})`, "value", current, [...values(process[0]), ...values(process[1])], { fitToData: true }),
          { ...axis(`MV (${outputUnit})`, "value", current, outputValues, { fitToData: true }), gridIndex: 1 }],
        series: [current.sp, current.pv, current.mv].map(color => ({ lineStyle: { color }, itemStyle: { color },
          markLine: { lineStyle: { color: current.muted }, label: { color: current.muted } } })),
      });
    });
    const valuesView = document.createElement("details");
    const valuesSummary = document.createElement("summary"); valuesSummary.textContent = "Accessible recorded values"; valuesView.append(valuesSummary);
    const tableWrap = document.createElement("div"); tableWrap.className = "chart-table"; tableWrap.dataset.engineerRecordedValues = "";
    const table = document.createElement("table");
    const caption = document.createElement("caption"); caption.textContent = `${role} recorded values; gaps are missing or non-good-quality samples.`; table.append(caption);
    const head = document.createElement("thead"); const headers = document.createElement("tr"); const timeHeader = document.createElement("th"); timeHeader.textContent = "Time (s)"; headers.append(timeHeader);
    for (const channel of run.channels) { const header = document.createElement("th"); header.textContent = `${roleLabel(channel, resolved)} · ${channel.id} (${channel.unit})`; headers.append(header); }
    head.append(headers); table.append(head);
    const body = document.createElement("tbody");
    for (const sample of run.samples) {
      const row = document.createElement("tr"); const time = document.createElement("td"); time.textContent = formatEngineeringNumber(sample.timeSeconds); row.append(time);
      for (const channel of run.channels) { const cell = document.createElement("td"); const reading = sample.values[channel.id]; cell.textContent = reading?.quality === "good" && reading.value !== null ? formatEngineeringNumber(reading.value) : `Gap (${reading?.quality ?? "missing"})`; row.append(cell); }
      body.append(row);
    }
    table.append(body); tableWrap.append(table); valuesView.append(tableWrap); card.append(valuesView);
    return card;
  }
}

function point(sample: RecordedSample, channel: string | undefined): number | null {
  const reading = channel ? sample.values[channel] : undefined;
  return reading?.quality === "good" && typeof reading.value === "number" ? reading.value : null;
}

function points(run: RecordedRun, channel: { id: string } | undefined): number[][] {
  return run.samples.map(sample => [sample.timeSeconds, point(sample, channel?.id)] as unknown as number[]);
}

function roleLabel(channel: { id: string }, resolved: RecordedResponseChannels): string {
  if (channel.id === resolved.setpoint?.id) return "SP";
  if (channel.id === resolved.processVariable?.id) return "PV";
  if (channel.id === resolved.manipulatedVariable?.id) return "MV";
  return "Observed channel";
}

function channelLabel(role: string, channel: { id: string } | undefined): string {
  return `${role} · ${channel?.id ?? "unavailable channel"}`;
}
