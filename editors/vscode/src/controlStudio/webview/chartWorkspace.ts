/// <reference lib="dom" />


import {
commitDesignEdit,
controlPlotView,
controlStagePlots,
createDesignHistory,
decodeControlChartSession,
directChartActionAvailability,
displayEngineeringUnit,
formatEngineeringNumber,
parseGraphicalGain,
pidEngineeringUnits,
pinComparableDesign,
redoDesignEdit,
selectControlPlot,
undoDesignEdit,
  type ControlWorkflowStage,
  type DesignComparisonContext,
  type DesignHistory,
type PinnedDesign
} from "../chartContract";
import type { ControlPidCandidate } from "../controllerCandidate";
import type { AvailableLoopSample, LoopMonitorSample } from "../loopMonitor";
import { controlSignalRows, type RecordedSignalSample } from "./signalRows";

import { copyValue,plotLabel,prefersReducedMotion } from "./chartPrimitives";
import { engineerPlotForStage, type EngineerStage } from "./engineerPlotNavigation";
import { historyShortcutAction, renderDesignComparisonPins } from "./designComparisonPresentation";
import type { ControlChartAdapter } from "./liveChart";
type DesignSnapshot = { revision: number; candidate: ControlPidCandidate };

export class ControlChartWorkspace {
  private stage: ControlWorkflowStage = "Monitor";
  private engineerStage: EngineerStage | undefined;
  private readonly selectedPlots: Partial<Record<ControlWorkflowStage, string>> = { Monitor: "response" };
  private readonly evidence = {
    samples: false, recording: false, model: false, candidate: false,
    fit: false, structureComparison: false, suppliedModel: false, predictedResponse: false, constraints: false, mimo: false, rootLocus: false, recordedResponses: false,
  };
  private history: DesignHistory<DesignSnapshot> | undefined;
  private pins: PinnedDesign<DesignSnapshot>[] = [];
  private selectedSignals = new Set(["sp", "pv", "mv"]);
  private lastAvailableSample: AvailableLoopSample | undefined;
  private historicalSignals: { sample?: RecordedSignalSample } | undefined;
  private revision = 0;
  private manualStage = false;
  private recordedPlotsAvailable = false;
  private editTimer: number | undefined;
  private selectedPinnedRevision: number | undefined;
  private comparisonContext: DesignComparisonContext = { modelRevision: "unknown", evidenceRevision: "unknown", method: "qualified-v1" };

  constructor(
    private readonly root: HTMLElement,
    private readonly adapter: ControlChartAdapter,
    private readonly hasComparisonEvidence: (context: DesignComparisonContext, candidate: ControlPidCandidate) => boolean = () => false
  ) {
    window.addEventListener("trust-control-chart-signal-selection", event => {
      const { id, selected } = (event as CustomEvent<{ id: string; selected: boolean }>).detail;
      if (selected) this.selectedSignals.add(id); else this.selectedSignals.delete(id);
      this.renderSignals(this.lastAvailableSample);
      this.saveSessionState();
    });
    window.addEventListener("trust-control-recorded-responses", event => {
      const detail = (event as CustomEvent<{ available: boolean; hasPlots?: boolean }>).detail;
      this.evidence.recordedResponses = detail.available;
      this.recordedPlotsAvailable = detail.available && detail.hasPlots !== false;
      this.renderTabs();
    });
    window.addEventListener("trust-control-studio-engineer", event => {
      const payload = (event as CustomEvent<Record<string, unknown>>).detail;
      const stageEvidence = payload.stageEvidence as { hasModel?: boolean; hasCandidate?: boolean } | undefined;
      if (stageEvidence?.hasModel !== undefined) {
        this.evidence.model = stageEvidence.hasModel;
        if (!stageEvidence.hasModel) this.evidence.suppliedModel = false;
      }
      if (payload.model !== undefined) this.evidence.model = true;
      if (stageEvidence?.hasCandidate === true || ["candidate", "verified", "review", "commissioned", "applied"].includes(String(payload.status))) {
        this.evidence.candidate = true;
      }
      this.renderTabs();
    });
    window.addEventListener("trust-control-studio-general-model", event => {
      const payload = (event as CustomEvent<Record<string, unknown>>).detail;
      if (payload.status === "imported") this.evidence.fit = false;
      this.evidence.suppliedModel = ["complete", "inspection"].includes(String(payload.status)) &&
        this.suppliedModelHostsReady();
      this.renderTabs();
    });
    window.addEventListener("trust-control-studio-general-model-rendered", event => {
      this.evidence.suppliedModel = (event as CustomEvent<{ available?: boolean }>).detail.available === true &&
        this.suppliedModelHostsReady();
      this.renderTabs();
    });
    this.installIdentificationViews();
    this.installSuppliedModelView();
    this.installMimoViews();
    this.installGraphicalDesignHandle();
    window.addEventListener("trust-control-studio-stage", (event) => {
      const stage = (event as CustomEvent<{ stage?: ControlWorkflowStage }>).detail?.stage;
      if (stage) this.setStage(stage);
    });
    // The engineer workflow has permanent Model/Tune/Verify/Apply navigation of
    // its own. Its stage event is deliberately only a selection signal: evidence
    // still comes from the chart adapters and recorded-response renderer.
    window.addEventListener("trust-control-studio-engineer-stage", (event) => {
      const stage = (event as CustomEvent<{ stage?: EngineerStage }>).detail?.stage;
      if (stage) this.setEngineerStage(stage);
    });
    for (const action of ["undo", "redo", "pin", "expand"] as const) {
      document.querySelector<HTMLButtonElement>(`[data-chart-action="${action}"]`)?.addEventListener("click", () => {
        if (action === "undo") this.undo();
        else if (action === "redo") this.redo();
        else if (action === "pin") this.pin();
        else this.toggleExpanded();
      });
    }
    document.addEventListener("keydown", event => {
      const action = historyShortcutAction(event);
      if (!action) return;
      event.preventDefault();
      if (action === "undo") this.undo(); else this.redo();
    });
    window.addEventListener("trust-control-studio-comparison-context", event => {
      const context = (event as CustomEvent<DesignComparisonContext>).detail;
      if (!context || context.modelRevision === this.comparisonContext.modelRevision &&
          context.evidenceRevision === this.comparisonContext.evidenceRevision && context.method === this.comparisonContext.method) return;
      this.comparisonContext = { ...context };
      this.history = undefined;
      this.selectedPinnedRevision = undefined;
      this.renderDesigns();
    });
    this.renderTabs();
    this.saveSessionState();
  }

  private installIdentificationViews(): void {
    const host = document.querySelector<HTMLElement>("[data-identification-fit-plot]");
    if (!host) return;
    host.className = "analysis-plot";
    this.installAnalysisView("fit", host);
    this.installAnalysisView("residuals");
    this.installAnalysisView("validation-data");
    this.installAnalysisView("poles-zeros");
  }

  /** Reuses the general-model ECharts hosts inside the shared engineer plot surface. */
  private installSuppliedModelView(): void {
    const plotWrap = document.querySelector<HTMLElement>(".plot-wrap");
    const magnitude = document.querySelector<HTMLElement>("[data-general-model-magnitude]");
    const phase = document.querySelector<HTMLElement>("[data-general-model-phase]");
    if (!plotWrap || !magnitude || !phase) return;
    const view = document.createElement("div");
    view.className = "plot-view";
    view.dataset.plotView = "general-model";
    view.hidden = true;
    const grid = document.createElement("div");
    grid.className = "analysis-grid bode-grid";
    for (const host of [magnitude, phase]) { host.classList.add("analysis-plot"); host.style.height = "100%"; }
    grid.append(magnitude, phase);
    view.append(grid);
    plotWrap.append(view);
  }

  private suppliedModelHostsReady(): boolean {
    return Boolean(document.querySelector("[data-general-model-magnitude] canvas") &&
      document.querySelector("[data-general-model-phase] canvas"));
  }

  private installAnalysisView(id: string, existingHost?: HTMLElement): void {
    const plotWrap = document.querySelector<HTMLElement>(".plot-wrap");
    if (!plotWrap) return;
    const view = document.createElement("div");
    view.className = "plot-view";
    view.dataset.plotView = id;
    view.hidden = true;
    const host = existingHost ?? document.createElement("div");
    host.className = "analysis-plot";
    host.dataset.identificationPlot = id;
    host.tabIndex = 0;
    host.setAttribute("role", "img");
    host.setAttribute("aria-label", plotLabel(id));
    view.append(host);
    plotWrap.append(view);
  }

  private installMimoViews(): void {
    this.installMimoView("channel-responses");
    this.installMimoView("rga");
    this.installMimoView("singular-values");
    this.installMimoView("interaction");
  }

  private installMimoView(id: string): void {
    const plotWrap = document.querySelector<HTMLElement>(".plot-wrap");
    if (!plotWrap) return;
    const view = document.createElement("div");
    view.className = "plot-view";
    view.dataset.plotView = id;
    view.hidden = true;
    const host = document.createElement("div");
    host.className = id === "rga" ? "signal-list" : "analysis-plot";
    host.dataset.mimoPlot = id;
    host.tabIndex = 0;
    host.setAttribute("role", id === "rga" ? "table" : "img");
    host.setAttribute("aria-label", plotLabel(id));
    view.append(host);
    plotWrap.append(view);
  }

  private installGraphicalDesignHandle(): void {
    const handle = document.querySelector<HTMLElement>(".design-handle");
    const row = document.querySelector<HTMLElement>("[data-control-designs]");
    const range = document.querySelector<HTMLInputElement>("[data-design-handle=\"proportionalGain\"]");
    const exact = document.querySelector<HTMLInputElement>("[data-design-exact=\"proportionalGain\"]");
    const error = document.querySelector<HTMLElement>("[data-design-error]");
    if (!handle || !row || !range || !exact || !error) return;
    const textNode = handle.firstChild;
    if (textNode?.nodeType === Node.TEXT_NODE) {
      const label = document.createElement("span");
      label.className = "design-label";
      label.textContent = textNode.textContent?.trim() ?? "Proportional gain (Kp)";
      handle.replaceChild(label, textNode);
    }
    handle.hidden = true;
    row.append(handle);
    const commit = (value: number): void => {
      range.value = exact.value = String(value);
      exact.setCustomValidity("");
      error.textContent = "";
      this.editGain(value);
    };
    range.addEventListener("input", () => commit(Number(range.value)));
    exact.addEventListener("change", () => {
      const result = parseGraphicalGain(exact.value, Number(range.min), Number(range.max));
      if (!result.ok) {
        exact.setCustomValidity(result.reason);
        error.textContent = result.reason;
        return;
      }
      commit(result.value);
    });
  }

  setMonitor(sample: LoopMonitorSample | undefined): void {
    const changed = this.evidence.samples !== (sample?.availability === "available");
    this.evidence.samples = sample?.availability === "available";
    if (sample?.availability === "available") {
      this.historicalSignals = undefined;
      this.lastAvailableSample = sample;
      this.adapter.setOutputLimits(sample.outputLimits.minimum, sample.outputLimits.maximum);
    }
    this.renderSignals(sample);
    if (changed) this.renderTabs();
  }

  setRecordingAvailable(available: boolean): void {
    this.evidence.recording = available;
    this.renderTabs();
  }

  /** Replace the displayed revision; saved histories remain in their original results. */
  restoreResultEvidence(recording: boolean, model: boolean, candidate?: ControlPidCandidate): void {
    for (const key of Object.keys(this.evidence) as Array<keyof typeof this.evidence>) this.evidence[key] = false;
    this.evidence.recording = recording;
    this.evidence.model = model;
    this.history = undefined;
    this.pins = [];
    this.selectedPinnedRevision = undefined;
    this.recordedPlotsAvailable = false;
    if (candidate) this.setCandidate(candidate);
    else {
      this.renderDesigns();
      this.renderTabs();
    }
  }

  setRecordedSample(sample: RecordedSignalSample | undefined): void {
    this.historicalSignals = { sample };
    this.renderSignals(this.lastAvailableSample);
  }

  setDeclaredRootAvailable(): void {
    this.evidence.rootLocus = true;
    this.selectedPlots.Tune = "root-locus";
    this.renderTabs();
  }

  setModelAvailable(available: boolean): void {
    this.evidence.model = available;
    this.renderTabs();
  }

  setFitAvailable(available: boolean, structureComparison = false): void {
    if (available && !this.evidence.fit) this.selectedPlots.Identify = "fit";
    this.evidence.fit = available;
    this.evidence.structureComparison = available && structureComparison;
    this.renderTabs();
  }

  setConstraintsAvailable(available: boolean): void {
    this.evidence.constraints = available;
    this.renderTabs();
  }

  setMimoAvailable(available: boolean): void {
    this.evidence.mimo = available;
    this.renderTabs();
  }

  setCandidate(candidate: ControlPidCandidate): void {
    this.evidence.candidate = true;
    const boundary = this.boundary();
    const snapshot = { revision: ++this.revision, candidate: copyValue(candidate) };
    if (!this.history) this.history = createDesignHistory(snapshot, boundary);
    else if (this.history.boundary !== boundary) this.history = createDesignHistory(snapshot, boundary);
    else if (JSON.stringify(this.history.current.candidate.parameters) !== JSON.stringify(candidate.parameters)) {
      this.history = commitDesignEdit(this.history, snapshot, boundary);
    } else {
      this.revision -= 1;
      snapshot.revision = this.history.current.revision;
    }
    const range = document.querySelector<HTMLInputElement>("[data-design-handle=\"proportionalGain\"]");
    const exact = document.querySelector<HTMLInputElement>("[data-design-exact=\"proportionalGain\"]");
    if (range && exact) {
      const maximum = String(Math.max(20, candidate.parameters.proportionalGain * 4));
      range.max = exact.max = maximum;
      range.value = exact.value = String(candidate.parameters.proportionalGain);
      const processUnit = this.root.dataset.processUnit ?? "process";
      const outputUnit = this.root.dataset.outputUnit ?? "output";
      const unit = document.querySelector<HTMLElement>("[data-design-unit]");
      if (unit) unit.textContent = pidEngineeringUnits(processUnit, outputUnit).proportional;
    }
    this.renderDesigns();
    this.renderTabs();
    this.saveSessionState();
  }

  setPredictedResponseAvailable(available: boolean): void {
    this.evidence.predictedResponse = available;
    this.renderTabs();
  }

  invalidateComparisons(): void {
    this.history = undefined;
    this.comparisonContext = { modelRevision: "unavailable", evidenceRevision: "unavailable", method: "unavailable" };
    this.selectedPinnedRevision = undefined;
    this.renderDesigns();
  }

  restoreSession(value: unknown): void {
    const session = decodeControlChartSession(value, this.sessionBoundary());
    if (!session) return;
    Object.assign(this.selectedPlots, session.selectedPlots);
    this.selectedSignals = new Set(session.selectedSignals);
    this.adapter.setSelectedSignals(session.selectedSignals);
    this.renderSignals(this.lastAvailableSample);
    this.stage = session.activeStage;
    this.root.dataset.plotExpanded = String(session.expanded);
    this.adapter.restoreWindow(session.window);
    this.pins = (session.pinnedDesigns ?? []).map((pin) => ({
      candidate: { revision: pin.revision, candidate: pin.candidate as unknown as ControlPidCandidate },
      context: pin.context,
    }));
    this.revision = Math.max(this.revision, ...this.pins.map(pin => pin.candidate.revision));
    window.dispatchEvent(new CustomEvent("trust-control-studio-restore-stage", {
      detail: { stage: session.activeStage },
    }));
    this.renderDesigns();
    this.renderTabs();
    const button = document.querySelector<HTMLButtonElement>("[data-chart-action=\"expand\"]");
    button?.setAttribute("aria-pressed", String(session.expanded));
    if (button) button.textContent = session.expanded ? "Restore" : "Expand";
  }

  private setStage(stage: ControlWorkflowStage): void {
    if (this.root.dataset.guided === "true" && this.root.dataset.manualWorkspace !== "true") return;
    this.engineerStage = undefined;
    this.stage = stage;
    this.manualStage = true;
    this.renderTabs();
  }

  private setEngineerStage(stage: EngineerStage): void {
    this.engineerStage = stage;
    this.manualStage = false;
    this.renderTabs();
  }

  private renderTabs(): void {
    const host = document.querySelector<HTMLElement>("[data-control-plot-tabs]");
    if (!host) return;
    if (this.engineerStage) {
      this.renderEngineerPlot(host);
      return;
    }
    const tabs = controlStagePlots(this.stage, { ...this.evidence,
      manualParameters: this.root.dataset.manualParameters === "true",
      ...(this.stage === "Identify" ? { model: this.evidence.fit } : {}),
    });
    if (this.stage === "Tune" && !this.evidence.predictedResponse) {
      for (const tab of tabs) if (["response", "bode", "nyquist", "nichols", "sensitivity"].includes(tab.id)) { tab.available = false; tab.reason = "Tune a controller to create this prediction."; }
    }
    if (this.stage === "Tune" && this.evidence.suppliedModel) tabs.unshift({ id: "general-model", available: true });
    const selected = selectControlPlot(tabs, this.selectedPlots[this.stage]);
    if (selected) this.selectedPlots[this.stage] = selected;
    host.setAttribute("aria-label", `Plots for ${this.stage}`);
    host.replaceChildren(...tabs.map((tab) => {
      const button = document.createElement("button");
      button.type = "button";
      button.role = "tab";
      button.textContent = tab.id === "general-model" ? "Model response" : plotLabel(tab.id);
      button.dataset.plotTab = tab.id;
      button.setAttribute("aria-selected", String(tab.id === selected));
      button.setAttribute("aria-disabled", String(!tab.available));
      button.disabled = !tab.available;
      if (tab.reason) button.title = tab.reason;
      button.addEventListener("click", () => {
        if (!tab.available) this.showUnavailable(tab.reason ?? "Required evidence is unavailable.");
        else {
          this.manualStage = false;
          this.selectedPlots[this.stage] = tab.id;
          this.renderTabs();
        }
      });
      return button;
    }));
    const active = tabs.find((tab) => tab.id === selected);
    this.root.dataset.plotAvailable = String(Boolean(active?.available));
    if (active?.available) this.showPlot(selected!);
    else this.showUnavailable(active?.reason ?? "Required evidence is unavailable.");
    this.renderChartActions(Boolean(active?.available) &&
        (this.stage !== "Monitor" || selected !== "response" ||
          this.adapter.hasSamples || Boolean(this.evidence.samples || this.evidence.recording)),
      !["signals", "events"].includes(selected ?? ""));
    host.querySelector<HTMLElement>("[aria-selected=\"true\"]")
      ?.scrollIntoView({ block: "nearest", inline: "nearest" });
    const tuning = this.stage === "Tune" && this.evidence.candidate;
    for (const action of ["undo", "redo"] as const) {
      const button = document.querySelector<HTMLButtonElement>(`[data-chart-action="${action}"]`);
      if (button) button.hidden = !tuning;
    }
    const designHandle = document.querySelector<HTMLElement>(".design-handle");
    if (designHandle) {
      designHandle.hidden = !(tuning && selected !== undefined &&
        ["bode", "root-locus", "nichols"].includes(selected));
    }
    this.updateHistoryActions();
    this.saveSessionState();
  }

  private renderEngineerPlot(host: HTMLElement): void {
    const selection = engineerPlotForStage(this.engineerStage!, { ...this.evidence, model: this.evidence.fit });
    host.setAttribute("aria-label", `Plots for ${this.engineerStage}`);
    const button = document.createElement("button");
    button.type = "button";
    button.role = "tab";
    button.textContent = selection.label;
    button.dataset.plotTab = selection.id;
    button.setAttribute("aria-selected", "true");
    button.setAttribute("aria-disabled", String(!selection.available));
    button.disabled = !selection.available;
    if (selection.reason) button.title = selection.reason;
    host.replaceChildren(button);
    const chartAvailable = selection.available && (selection.id !== "recorded-responses" || this.recordedPlotsAvailable);
    this.root.dataset.plotAvailable = String(chartAvailable);
    if (selection.available) this.showPlot(selection.id);
    else this.showUnavailable(selection.reason);
    this.renderChartActions(chartAvailable);
    this.updateHistoryActions();
  }

  private renderChartActions(available: boolean, figure = true): void {
    for (const [action, enabled] of Object.entries(directChartActionAvailability(available && figure))) {
      const actionButton = document.querySelector<HTMLButtonElement>(`[data-chart-action="${action}"]`);
      if (actionButton) {
        actionButton.hidden = !figure;
        actionButton.disabled = !enabled;
      }
    }
  }

  private showPlot(id: string): void {
    const mapped = this.engineerStage ? id : controlPlotView(this.stage, id);
    this.adapter.setPlotVisible(mapped === "response");
    let found = false;
    for (const view of document.querySelectorAll<HTMLElement>("[data-plot-view]")) {
      const active = view.dataset.plotView === mapped;
      view.hidden = !active;
      found ||= active;
    }
    if (!found) this.showUnavailable(`${plotLabel(id)} is waiting for its qualified analysis result.`);
    this.adapter.resize();
    const recorded = id === "recorded-response" || (!this.engineerStage && this.stage === "Identify" && Boolean(this.evidence.recording)) || (!this.engineerStage && this.stage === "Validate" &&
      id === "signals" && this.evidence.recording && !this.evidence.candidate);
    window.dispatchEvent(new CustomEvent("trust-control-studio-plot-shown", {
      detail: { id: mapped, manualStage: this.manualStage, source: recorded ? "recording" : "live" },
    }));
  }

  private showUnavailable(reason: string): void {
    this.adapter.setPlotVisible(false);
    for (const view of document.querySelectorAll<HTMLElement>("[data-plot-view]")) {
      view.hidden = view.dataset.plotView !== "unavailable";
    }
    const message = document.querySelector<HTMLElement>("[data-plot-unavailable]");
    if (message) message.textContent = reason;
    window.dispatchEvent(new CustomEvent("trust-control-studio-plot-shown", { detail: { id: "unavailable", manualStage: this.manualStage } }));
  }

  private renderSignals(sample: LoopMonitorSample | undefined): void {
    const host = document.querySelector<HTMLElement>("[data-control-signals]");
    if (!host) return;
    const live = sample?.availability === "available" ? sample : this.lastAvailableSample;
    const signals = controlSignalRows(this.historicalSignals ? live :
      sample?.availability === "available" ? sample : undefined, this.historicalSignals,
      this.root.dataset.loopTopology === "open-loop" ? "open-loop" : "closed-loop");
    if (signals.length === 0) {
      host.textContent = this.historicalSignals ? "No recorded signal values in this result."
        : "No current signals. Start the runtime to inspect signal values.";
      return;
    }
    host.replaceChildren(...signals.map((signal) => {
      const row = document.createElement("label");
      row.className = "signal-row";
      const checked = document.createElement("input");
      checked.type = "checkbox";
      checked.checked = this.selectedSignals.has(signal.id);
      this.adapter.setSignalSelected(signal.id, checked.checked);
      checked.addEventListener("change", () => {
        if (checked.checked) this.selectedSignals.add(signal.id);
        else this.selectedSignals.delete(signal.id);
        this.adapter.setSignalSelected(signal.id, checked.checked);
        this.saveSessionState();
      });
      const name = document.createElement("span"); name.textContent = signal.name;
      const role = document.createElement("span"); role.className = "signal-secondary";
      role.textContent = `${signal.role} · ${signal.symbol}`;
      const value = document.createElement("span");
      value.textContent = `${formatEngineeringNumber(signal.value)} ${displayEngineeringUnit(signal.unit)}`;
      const quality = document.createElement("span"); quality.className = "signal-secondary"; quality.textContent = signal.quality;
      row.append(checked, name, role, value, quality);
      return row;
    }));
  }

  private editGain(value: number): void {
    if (!this.history || !Number.isFinite(value) || value <= 0) return;
    const candidate = copyValue(this.history.current.candidate);
    candidate.parameters.proportionalGain = value;
    const active = document.querySelector<HTMLElement>("[data-active-design]");
    if (active) active.textContent = `Candidate · Kp ${formatEngineeringNumber(value)} · Updating`;
    if (this.editTimer !== undefined) window.clearTimeout(this.editTimer);
    this.editTimer = window.setTimeout(() => {
      window.dispatchEvent(new CustomEvent("trust-control-studio-design-edit", {
        detail: { settings: candidate.parameters },
      }));
    }, prefersReducedMotion() ? 0 : 140);
  }

  private undo(): void {
    if (!this.history) return;
    this.history = undoDesignEdit(this.history, this.boundary());
    this.dispatchHistoryCandidate();
  }

  private redo(): void {
    if (!this.history) return;
    this.history = redoDesignEdit(this.history, this.boundary());
    this.dispatchHistoryCandidate();
  }

  private dispatchHistoryCandidate(): void {
    if (!this.history) return;
    window.dispatchEvent(new CustomEvent("trust-control-studio-design-edit", {
      detail: { settings: this.history.current.candidate.parameters },
    }));
    this.renderDesigns();
  }

  private pin(): void {
    if (!this.history) return;
    this.pins = pinComparableDesign(this.pins, this.history.current, {
      ...this.comparisonContext,
    });
    this.renderDesigns();
  }

  private renderDesigns(): void {
    const row = document.querySelector<HTMLElement>("[data-control-designs]");
    if (row) row.hidden = !this.history && this.pins.length === 0;
    const active = document.querySelector<HTMLElement>("[data-active-design]");
    if (active && this.history) {
      const value = this.history.current.candidate;
      active.textContent = `Candidate ${this.history.current.revision} · ${value.structure} · Kp ${formatEngineeringNumber(value.parameters.proportionalGain)}`;
    }
    const host = document.querySelector<HTMLElement>("[data-pinned-designs]");
    if (host) renderDesignComparisonPins(host, this.pins.map(pin => {
      const available = pin.context.modelRevision === this.comparisonContext.modelRevision &&
        pin.context.evidenceRevision === this.comparisonContext.evidenceRevision && pin.context.method === this.comparisonContext.method &&
        this.hasComparisonEvidence(pin.context, pin.candidate.candidate);
      return {
        revision: pin.candidate.revision,
        structure: pin.candidate.candidate.structure,
        proportionalGain: pin.candidate.candidate.parameters.proportionalGain,
        integralGain: pin.candidate.candidate.parameters.integralGain,
        derivativeGain: pin.candidate.candidate.parameters.derivativeGain,
        source: pin.candidate.candidate.source,
        validation: available ? "Model prediction" : "Unavailable",
        available,
        ...(available ? {} : { reason: "Model or evidence context changed." }),
      };
    }), this.selectedPinnedRevision, revision => {
      if (revision === undefined) {
        this.selectedPinnedRevision = undefined;
        window.dispatchEvent(new CustomEvent("trust-control-studio-pinned-design-select", { detail: { revision: undefined } }));
        this.renderDesigns();
        return;
      }
      const selected = this.pins.find(pin => pin.candidate.revision === revision);
      if (!selected) return;
      this.selectedPinnedRevision = revision;
      window.dispatchEvent(new CustomEvent("trust-control-studio-pinned-design-select", {
        detail: { revision, candidate: copyValue(selected.candidate.candidate), context: copyValue(selected.context) },
      }));
      this.renderDesigns();
    }, revision => {
      this.pins = this.pins.filter(pin => pin.candidate.revision !== revision);
      if (this.selectedPinnedRevision === revision) {
        this.selectedPinnedRevision = undefined;
        window.dispatchEvent(new CustomEvent("trust-control-studio-pinned-design-select", { detail: { revision: undefined } }));
      }
      this.renderDesigns();
    });
    const pin = document.querySelector<HTMLButtonElement>("[data-chart-action=\"pin\"]");
    if (pin) pin.disabled = !this.history || this.pins.length >= 3;
    this.updateHistoryActions();
    this.saveSessionState();
  }

  private updateHistoryActions(): void {
    const undo = document.querySelector<HTMLButtonElement>("[data-chart-action=\"undo\"]");
    const redo = document.querySelector<HTMLButtonElement>("[data-chart-action=\"redo\"]");
    if (undo) undo.disabled = !this.history?.past.length;
    if (redo) redo.disabled = !this.history?.future.length;
  }

  private toggleExpanded(): void {
    const expanded = this.root.dataset.plotExpanded !== "true";
    this.root.dataset.plotExpanded = String(expanded);
    const button = document.querySelector<HTMLButtonElement>("[data-chart-action=\"expand\"]");
    button?.setAttribute("aria-pressed", String(expanded));
    if (button) button.textContent = expanded ? "Restore" : "Expand";
    requestAnimationFrame(() => this.adapter.resize());
    this.saveSessionState();
  }

  private saveSessionState(): void {
    const session = {
      schemaVersion: 1,
      boundary: this.sessionBoundary(),
      activeStage: this.stage,
      selectedPlots: this.selectedPlots,
      selectedSignals: [...this.selectedSignals],
      window: this.adapter.viewWindow(),
      expanded: this.root.dataset.plotExpanded === "true",
      ...(this.history ? { candidateRevision: this.history.current.revision } : {}),
      pinnedCandidateRevisions: this.pins.map((pin) => pin.candidate.revision),
      pinnedDesigns: this.pins.map((pin) => ({
        revision: pin.candidate.revision,
        candidate: pin.candidate.candidate as unknown as Record<string, unknown>,
        context: pin.context,
      })),
    };
    this.root.dataset.controlChartSession = JSON.stringify(session);
  }

  private boundary(): string {
    return `${this.sessionBoundary()}:` +
      `${this.comparisonContext.modelRevision}:${this.comparisonContext.evidenceRevision}:${this.comparisonContext.method}`;
  }

  private sessionBoundary(): string {
    return `${this.root.dataset.loopId ?? "unbound"}:${this.root.dataset.loopRevision ?? "no-loop"}`;
  }
}
