import type { WorkflowRecordingProvenance } from "../workflowResult";
import { formatLocalTimestamp } from "../timestampFormat";
import { renderRecordingActions } from "./recordingControls";
import { unavailableLoopPresentation } from "../engineerWorkflowPresentation";
import type { DeclaredRootLocusResult } from "../declaredAnalysis";
import { requiredElement } from "./chartPrimitives";
import { ControlChartWorkspace } from "./chartWorkspace";
import { renderDeclaredRootResult } from "./declaredRootResult";
import { EngineeringChartAdapter,IdentificationChartAdapter,MimoChartAdapter } from "./engineeringCharts";
import { ControlChartAdapter } from "./liveChart";
/// <reference lib="dom" />

import { LineChart } from "echarts/charts";
import {
AriaComponent,
DataZoomComponent,
GridComponent,
GraphicComponent,
LegendComponent,
MarkAreaComponent,
MarkLineComponent,
TooltipComponent,
} from "echarts/components";
import * as echarts from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";

import {
compactRevisionLabel,
controlChartSaturationEvent,
decodeMimoPlotEvidence,
displayEngineeringUnit,
formatEngineeringNumber,
restoredControlChartSamples,
type ControlChartSample,
type ControlSaturationState
} from "../chartContract";
import type { ControlPidCandidate } from "../controllerCandidate";
import type { EngineeringPlotMessageEvidence } from "../engineeringPlots";
import type { IdentifiedModelPresentation } from "../identification";
import type { AvailableLoopSample, LoopMonitorSample, LoopRecording } from "../loopMonitor";
import type { WorkflowResultSample } from "../workflowResult";
import { decodeLoopMonitorEnvelope,decodePlantMonitorEnvelope } from "../messages";
import { valueStripHistoricalState,type ValueStripHistoricalState } from "./valueStripState";
import { stableComparisonKey } from "./designComparisonPresentation";

echarts.use([
  LineChart,
  GridComponent,
  GraphicComponent,
  LegendComponent,
  MarkAreaComponent,
  MarkLineComponent,
  TooltipComponent,
  DataZoomComponent,
  AriaComponent,
  CanvasRenderer,
]);

// All Control Studio time plots go through this adapter so keyboard, export,
// theme, and navigation behavior cannot drift between workflow pages.
const host = document.querySelector<HTMLElement>("[data-control-studio-chart]");
const studio = document.querySelector<HTMLElement>("[data-control-studio]");
const adapter = host ? new ControlChartAdapter(host) : undefined;
const workspace = host && studio && adapter ? new ControlChartWorkspace(studio, adapter,
  (context, candidate) => engineeringCache.has(comparisonKey(context, candidate.parameters))) : undefined;
const mimo = workspace ? new MimoChartAdapter() : undefined;
const engineeringTime = document.querySelector<HTMLElement>("[data-engineering-time]");
const engineeringMagnitude = document.querySelector<HTMLElement>("[data-engineering-bode-magnitude]");
const engineeringPhase = document.querySelector<HTMLElement>("[data-engineering-bode-phase]");
const engineering = engineeringTime && engineeringMagnitude && engineeringPhase
  ? new EngineeringChartAdapter(engineeringTime, engineeringMagnitude, engineeringPhase) : undefined;
const identificationHost = document.querySelector<HTMLElement>("[data-identification-fit-plot]");
const identification = identificationHost ? new IdentificationChartAdapter(identificationHost) : undefined;
const engineeringCache = new Map<string, EngineeringPlotMessageEvidence>();
let currentEngineeringKey: string | undefined;
const receivedSamples: ControlChartSample[] = [];
const receivedMessages = new Map<string, string>();
let firstSimulationTime: bigint | undefined;
let firstLoopTimeMs: number | undefined;
let liveRuntimeSessionId: string | undefined;
let previousSaturation: ControlSaturationState | undefined;
let latestLoopSample: LoopMonitorSample | undefined;
let retainedLoopRecording: readonly WorkflowResultSample[] | undefined;
let retainedProvenance: WorkflowRecordingProvenance | undefined;
let recordedLoopView = false;
let savedResultVisible = false;
adapter?.setSamples([]);

window.addEventListener("trust-control-studio-plot-shown", event => {
  recordedLoopView = (event as CustomEvent<{ source?: string }>).detail.source === "recording";
  if (savedResultVisible || !adapter) return;
  if (recordedLoopView && retainedLoopRecording?.length) renderRetainedLoopRecording(retainedLoopRecording);
  else if (latestLoopSample) {
    renderLoopSample(latestLoopSample, adapter);
    workspace?.setMonitor(latestLoopSample);
  }
});

function comparisonKey(context: unknown, parameters: unknown): string {
  return stableComparisonKey({ context, parameters });
}

function invalidateEngineeringComparisons(): void {
  engineeringCache.clear();
  currentEngineeringKey = undefined;
  workspace?.invalidateComparisons();
}

window.addEventListener("trust-control-studio-general-model", event => {
  if ((event as CustomEvent<{ status?: string }>).detail.status === "imported") invalidateEngineeringComparisons();
});

function showEngineeringEvidence(evidence: EngineeringPlotMessageEvidence, editable = false): void {
  engineering?.setEvidence(evidence);
  const comparison = evidence.comparison;
  if (comparison && workspace) {
    window.dispatchEvent(new CustomEvent("trust-control-studio-comparison-context", { detail: comparison.context }));
    const key = comparisonKey(comparison.context, comparison.candidate.parameters);
    engineeringCache.set(key, evidence);
    currentEngineeringKey = key;
    if (editable) workspace.setCandidate(comparison.candidate);
  }
}

window.addEventListener("trust-control-studio-pinned-design-select", event => {
  if (!engineering) return;
  const detail = (event as CustomEvent<{ revision?: number; candidate?: ControlPidCandidate; context?: unknown }>).detail;
  const key = detail.revision === undefined ? currentEngineeringKey
    : comparisonKey(detail.context, detail.candidate?.parameters);
  const evidence = key ? engineeringCache.get(key) : undefined;
  if (evidence) engineering.setEvidence(evidence);
});

// The connected engineer workflow returns its candidate and shared prediction
// evidence on the engineer channel rather than the legacy tune channel.
window.addEventListener("trust-control-studio-engineer", event => {
  const payload = (event as CustomEvent<Record<string, unknown>>).detail;
  if (payload.identificationPresentation) {
    identification?.setEvidence(payload.identificationPresentation as IdentifiedModelPresentation);
    workspace?.setFitAvailable(true);
  } else if (payload.model && ["model-ready", "restored"].includes(String(payload.status))) workspace?.setFitAvailable(false);
  if (payload.modelAnalysis) {
    window.dispatchEvent(new CustomEvent("trust-control-studio-general-model", {
      detail: { status: "inspection", saved: payload.modelAnalysis },
    }));
  }
  if (["model-ready", "commissioned", "applied"].includes(String(payload.status))) {
    invalidateEngineeringComparisons();
  }
  if (["model-ready", "edited", "unavailable", "failed"].includes(String(payload?.status)) || payload?.status === "candidate" && !payload.plots) workspace?.setPredictedResponseAvailable(false);
  if (payload?.status === "candidate" && payload.plots) {
    const evidence = payload.plots as EngineeringPlotMessageEvidence;
    showEngineeringEvidence(evidence);
    workspace?.setPredictedResponseAvailable(true);
  }
});

window.addEventListener("message", (event) => {
  if (!studio || !adapter) return;
  const raw = event.data as Record<string, unknown> | undefined;
  if (raw && raw.projectId === studio.dataset.projectId && raw.editorSessionId === studio.dataset.editorSessionId &&
      raw.loopId === studio.dataset.loopId && raw.expectedLoopRevision === studio.dataset.loopRevision &&
      (raw.kind === "control-studio.recording.evidence" || raw.kind === "control-studio.experiment.result" || raw.kind === "control-studio.identification-recording")) {
    const payload = raw.payload as { recording?: LoopRecording; state?: string } | undefined;
    const native = payload?.recording?.rawSamples;
    if (Array.isArray(native) && native.length > 0) {
      retainedLoopRecording = native;
      retainedProvenance = payload?.recording?.provenance;
      if (raw.kind === "control-studio.identification-recording") {
        savedResultVisible = false;
        workspace?.setFitAvailable(false);
      }
      workspace?.setRecordingAvailable(true);
    } else if (payload?.state === "recording" || raw.phase === "running") {
      retainedLoopRecording = undefined;
      retainedProvenance = undefined;
      workspace?.setRecordingAvailable(false);
    }
  }
  if (raw?.kind === "control-studio.application.result" && ["applied", "rolled-back"].includes(String(raw.phase)) &&
      raw.projectId === studio.dataset.projectId && raw.editorSessionId === studio.dataset.editorSessionId &&
      raw.loopId === studio.dataset.loopId && raw.expectedLoopRevision === studio.dataset.loopRevision) {
    invalidateEngineeringComparisons();
    return;
  }
  if (raw?.kind === "control-studio.mimo.result" &&
      raw.projectId === studio.dataset.projectId && raw.editorSessionId === studio.dataset.editorSessionId &&
      raw.loopId === studio.dataset.loopId && raw.expectedLoopRevision === studio.dataset.loopRevision) {
    const payload = raw.payload as Record<string, unknown> | undefined;
    const evidence = decodeMimoPlotEvidence(payload?.evidence);
    if (evidence) {
      mimo?.setEvidence(evidence);
      workspace?.setMimoAvailable(true);
      workspace?.setModelAvailable(true);
    }
    return;
  }
  if (raw?.kind === "control-studio.workflow-result" && raw.phase === "draft" && savedResultVisible &&
      raw.projectId === studio.dataset.projectId && raw.editorSessionId === studio.dataset.editorSessionId &&
      raw.loopId === studio.dataset.loopId && raw.expectedLoopRevision === studio.dataset.loopRevision) {
    applyValueStripHistoricalState(valueStripHistoricalState(true, "", "draft"));
    setText("[data-control-studio-freshness]", "Historical · working draft");
    return;
  }
  if (raw?.kind === "control-studio.workflow-result" &&
      (raw.phase === "restored" || raw.phase === "opened") &&
      raw.projectId === studio.dataset.projectId && raw.editorSessionId === studio.dataset.editorSessionId &&
      raw.loopId === studio.dataset.loopId && raw.expectedLoopRevision === studio.dataset.loopRevision) {
    const payload = raw.payload as Record<string, unknown> | undefined;
    const result = payload?.result as Record<string, unknown> | undefined;
    const recording = result?.recording as Record<string, unknown> | undefined;
    const samples = Array.isArray(recording?.rawSamples) ? recording.rawSamples : [];
    const units = result?.units as Record<string, unknown> | undefined;
    const controller = result?.controller as Record<string, unknown> | undefined;
    const restored = restoredControlChartSamples(samples as Parameters<typeof restoredControlChartSamples>[0]);
    const last = samples[samples.length - 1] as WorkflowResultSample | undefined;
    savedResultVisible = true;
    retainedLoopRecording = samples as WorkflowResultSample[];
    retainedProvenance = recording?.provenance as WorkflowRecordingProvenance | undefined;
    adapter.setSamples(restored);
    workspace?.setRecordedSample(last ? { ...last,
      appliedMv: restored[restored.length - 1]?.mv ?? null,
      units: { sp: String(units?.sp ?? ""), pv: String(units?.pv ?? "engineering unit"), mv: String(units?.mv ?? "%") },
    } : undefined);
    invalidateEngineeringComparisons();
    engineering?.resetEvidence("No comparison is stored; run required validation for this candidate.");
    workspace?.restoreResultEvidence(restored.length > 0, Boolean(result?.model),
      result?.candidate as ControlPidCandidate | undefined);
    workspace?.restoreSession(result?.chartSession);
    adapter.setSource(String(result?.source ?? "Saved result"));
    adapter.setUnits(String(units?.pv ?? "engineering unit"), String(units?.mv ?? "%"));
    setText("[data-control-studio-source]", result?.source === "Physical" ? "Physical plant" : "Simulation");
    setText("[data-control-studio-connection]", "");
    setText("[data-control-studio-freshness]", payload?.draft === true ? "Historical · working draft" : `Historical · revision ${String(result?.resultRevision ?? "—")}${retainedProvenance ? " · imported recording" : ""}`);
    setOptionalText("[data-control-studio-controller-mode]", String(last?.controllerMode ?? "Unavailable"));
    setOptionalText("[data-control-studio-sp]", savedReading(last?.sp, units?.sp));
    setText("[data-control-studio-pv]", savedReading(last?.pv, units?.pv));
    setText("[data-control-studio-mv]", savedReading(restored[restored.length - 1]?.mv, units?.mv));
    const limits = last?.outputLimits as Record<string, unknown> | undefined;
    if (typeof limits?.minimum === "number" && typeof limits.maximum === "number") {
      adapter.setOutputLimits(limits.minimum, limits.maximum);
    }
    // Quality capitalisation, the "Output limit" name, and the unit must match the live
    // reading below exactly; a saved result is stale evidence, not a different vocabulary
    // (finding W-11).
    setText("[data-control-studio-quality]",
      `${titleCase(String(last?.quality ?? "Unavailable"))} · Output limit ${formatEngineeringNumber(limits?.minimum)}…${formatEngineeringNumber(limits?.maximum)} ${displayEngineeringUnit(String(units?.mv ?? ""))}`);
    const revision = document.querySelector<HTMLElement>("[data-control-studio-controller-revision]");
    if (revision) {
      revision.textContent = compactRevisionLabel(String(controller?.revision ?? "Unavailable"));
      revision.title = String(controller?.revision ?? "Unavailable");
    }
    // Bind this badge to the same result as the plotted samples, independently
    // of the other webview message listener's execution order.
    applyValueStripHistoricalState(valueStripHistoricalState(true, formatLocalTimestamp(String(result?.createdAt ?? "")).display,
      payload?.draft === true ? "draft" : "saved"));
    return;
  }
  if (raw?.kind === "control-studio.engineering.result" && raw.stage === "declared-root" &&
      raw.projectId === studio.dataset.projectId && raw.editorSessionId === studio.dataset.editorSessionId &&
      raw.loopId === studio.dataset.loopId && raw.expectedLoopRevision === studio.dataset.loopRevision) {
    const payload = raw.payload as { status?: string; result?: DeclaredRootLocusResult; reason?: string } | undefined;
    if (payload?.status === "complete" && payload.result) {
      workspace?.setDeclaredRootAvailable();
      engineering?.setDeclaredRootLocus(payload.result);
      renderDeclaredRootResult(payload.result);
    } else {
      setText("[data-declared-root-result]", payload?.reason ?? "The declared root locus is unavailable.");
    }
    return;
  }
  if (raw?.kind === "control-studio.engineering.result" && raw.stage === "tune" &&
      raw.projectId === studio.dataset.projectId && raw.editorSessionId === studio.dataset.editorSessionId &&
      raw.loopId === studio.dataset.loopId && raw.expectedLoopRevision === studio.dataset.loopRevision) {
    const payload = raw.payload as Record<string, unknown> | undefined;
    if (payload?.status === "complete" && payload.plots) {
      const evidence = payload.plots as EngineeringPlotMessageEvidence;
      showEngineeringEvidence(evidence, true);
      workspace?.setPredictedResponseAvailable(true);
      workspace?.setModelAvailable(true);
      workspace?.setConstraintsAvailable(true);
      if (!evidence.comparison && payload.candidate) workspace?.setCandidate(payload.candidate as unknown as ControlPidCandidate);
    } else if (payload?.status === "failed" || payload?.status === "unavailable") {
      // A failed or unavailable tune must still leave the engineering plots showing one
      // explanation instead of silently keeping (or never showing) stale evidence
      // (finding V-10).
      engineering?.showFailure(typeof payload.reason === "string" ? payload.reason : "");
      workspace?.setPredictedResponseAvailable(false);
    }
    return;
  }
  if (raw?.kind === "control-studio.engineering.result" && raw.stage === "identify" &&
      raw.projectId === studio.dataset.projectId && raw.editorSessionId === studio.dataset.editorSessionId &&
      raw.loopId === studio.dataset.loopId && raw.expectedLoopRevision === studio.dataset.loopRevision) {
    const payload = raw.payload as Record<string, unknown> | undefined;
    if (["qualified", "recording-selected", "editing"].includes(String(payload?.phase))) workspace?.setFitAvailable(false);
    if (payload?.status === "complete" && payload.presentation) {
      identification?.setEvidence(payload.presentation as unknown as IdentifiedModelPresentation);
      workspace?.setFitAvailable(true, Boolean((payload.presentation as unknown as IdentifiedModelPresentation).modelStructure));
      workspace?.setModelAvailable(true);
    } else if (payload?.status === "failed") {
      // Keeps the Fit/Residuals/Validation/Poles-and-zeros axes visible with one
      // explanation instead of an entirely blank plot after a failed identification
      // attempt (finding V-10).
      identification?.showFailure(typeof payload.reason === "string" ? payload.reason : "");
    }
    return;
  }
  if (studio.dataset.loopId && studio.dataset.loopRevision) {
    const loopEnvelope = decodeLoopMonitorEnvelope(event.data, {
      projectId: studio.dataset.projectId ?? "",
      editorSessionId: studio.dataset.editorSessionId ?? "",
      loopId: studio.dataset.loopId,
      loopRevision: studio.dataset.loopRevision,
    });
    if (loopEnvelope) {
      if (!acceptMessage(loopEnvelope.messageId, loopEnvelope)) return;
      const sample = loopEnvelope.payload;
      savedResultVisible = false;
      latestLoopSample = sample;
      if (sample.availability === "available") appendLoopSample(sample);
      window.dispatchEvent(new CustomEvent("trust-control-studio-runtime-state", { detail: {
        runtimeMode: sample.source === "Simulation" ? "simulate" : "online",
        runtimeState: sample.availability === "available" ? "running" : ["Runtime is stopped", "Runtime is not producing scans", "Runtime is unreachable"].includes(sample.reason) ? "stopped" : "connected",
      } }));
      if (!recordedLoopView || !retainedLoopRecording?.length) {
        renderLoopSample(sample, adapter);
        workspace?.setMonitor(sample);
      }
      return;
    }
  }
  const envelope = decodePlantMonitorEnvelope(event.data, {
    projectId: studio.dataset.projectId ?? "",
    editorSessionId: studio.dataset.editorSessionId ?? "",
    plantId: studio.dataset.plantId ?? "",
    plantRevision: studio.dataset.plantRevision ?? "",
  });
  if (!envelope) return;
  if (!acceptMessage(envelope.messageId, envelope)) return;
  const snapshot = envelope.payload;
  requiredElement<HTMLElement>("[data-plant-summary]").hidden = false;
  if (snapshot.availability === "unavailable") {
    const empty = unavailableLoopPresentation(snapshot.source, snapshot.reason);
    if (studio) studio.dataset.noLoopSamples = String(receivedSamples.length === 0);
    setText("[data-control-studio-connection]", empty.status);
    setText("[data-control-studio-freshness]", snapshot.reason);
    setText("[data-control-studio-quality]", empty.explanation);
    setOptionalText("[data-control-studio-sp]", "—");
    setText("[data-control-studio-pv]", "—");
    setText("[data-control-studio-mv]", "—");
    setText("[data-plant-state]", "Unavailable");
    setText("[data-plant-command]", "—");
    setText("[data-plant-measured]", "—");
    setText("[data-plant-published]", "—");
    setText("[data-plant-fault]", snapshot.reason);
    return;
  }

  const timestamp = BigInt(snapshot.simulationTimeNs);
  firstSimulationTime ??= timestamp;
  receivedSamples.push({
    elapsedSeconds: Number(timestamp - firstSimulationTime) / 1_000_000_000,
    sp: null,
    pv: first(snapshot.publishedFeedback),
    mv: first(snapshot.command),
    quality: snapshot.operatingState === "faulted" ? "bad" : "good",
    events: snapshot.currentFault
      ? [{ kind: "event", label: snapshot.currentFault }]
      : undefined,
  });
  if (receivedSamples.length > 12_000) receivedSamples.splice(0, receivedSamples.length - 12_000);
  adapter.setSamples(receivedSamples);

  setText("[data-control-studio-connection]", "Running");
  setText("[data-control-studio-freshness]", `Current · ${formatSimulationTime(snapshot.simulationTimeNs)} simulated`);
  setText("[data-control-studio-quality]", snapshot.operatingState === "faulted"
    ? `Fault · ${snapshot.currentFault ?? "Plant faulted"}`
    : "Good · Current simulation sample");
  setOptionalText("[data-control-studio-sp]", "—");
  setText("[data-control-studio-pv]", display(first(snapshot.publishedFeedback)));
  setText("[data-control-studio-mv]", display(first(snapshot.command)));
  setText("[data-plant-state]", titleCase(snapshot.operatingState));
  setText("[data-plant-command]", vector(snapshot.command));
  setText("[data-plant-measured]", vector(snapshot.measuredFeedback));
  setText("[data-plant-published]", vector(snapshot.publishedFeedback));
  setText("[data-plant-fault]", snapshot.currentFault ?? "None");
  setText("[data-plant-model]", snapshot.model);
  setText("[data-plant-applied]", vector(snapshot.appliedInput));
  setText("[data-plant-ideal]", vector(snapshot.idealOutput));
  setText("[data-plant-vector]", vector(snapshot.state));
  setText("[data-plant-time-scale]", String(snapshot.timeScale));
  setText("[data-plant-seed]", snapshot.effectiveSeed);
  setText("[data-plant-random]", `${snapshot.randomAlgorithm} v${snapshot.randomAlgorithmVersion}`);
  setText("[data-plant-effects]", list(snapshot.activeUncertainties));
  setText("[data-plant-dropouts]", list(snapshot.activeDropouts));
  setText("[data-plant-events]", String(snapshot.pendingEventCount));
  setText("[data-plant-delay]", String(snapshot.delayBufferDepth));
  setText("[data-plant-substeps]", String(snapshot.lastSubsteps));
  setText("[data-plant-runtime]", snapshot.runtimeVersion);
});

function appendLoopSample(snapshot: AvailableLoopSample): void {
  if (snapshot.runtimeSessionId !== undefined && snapshot.runtimeSessionId !== liveRuntimeSessionId) {
    liveRuntimeSessionId = snapshot.runtimeSessionId;
    receivedSamples.length = 0;
    firstLoopTimeMs = undefined;
    previousSaturation = undefined;
  }
  firstLoopTimeMs ??= snapshot.observedAtMs;
  const quality = [snapshot.setpoint, snapshot.processVariable, snapshot.manipulatedVariable]
    .filter((signal): signal is NonNullable<typeof signal> => signal !== undefined)
    .some((signal) => signal.quality === "forced") ? "bad" : "good";
  receivedSamples.push({
    elapsedSeconds: (snapshot.observedAtMs - firstLoopTimeMs) / 1_000,
    sp: snapshot.setpoint?.value ?? null,
    pv: snapshot.processVariable.value,
    mv: snapshot.manipulatedVariable.value,
    additional: snapshot.additional?.map(signal => ({ ...signal })),
    quality,
    events: (() => {
      const transition = controlChartSaturationEvent(previousSaturation, snapshot.saturation);
      previousSaturation = snapshot.saturation;
      return transition ? [transition] : undefined;
    })(),
  });
  if (receivedSamples.length > 12_000) receivedSamples.splice(0, receivedSamples.length - 12_000);
}

function renderRetainedLoopRecording(samples: readonly WorkflowResultSample[]): void {
  const last = samples[samples.length - 1];
  if (!adapter || !last) return;
  const restored = restoredControlChartSamples(samples);
  adapter.setSource(last.source);
  adapter.setUnits(last.units.pv, last.units.mv);
  adapter.setOutputLimits(last.outputLimits.minimum, last.outputLimits.maximum);
  adapter.setSamples(restored);
  workspace?.setRecordedSample(last);
  setText("[data-control-studio-source]", last.source === "Physical" ? "Physical plant" : "Simulation");
  setText("[data-control-studio-connection]", "");
  setText("[data-control-studio-freshness]", retainedProvenance ? `Historical · imported revision ${retainedProvenance.resultRevision}` : "Historical · retained recording");
  setOptionalText("[data-control-studio-controller-mode]", last.controllerMode);
  setOptionalText("[data-control-studio-sp]", savedReading(last.sp, last.units.sp));
  setText("[data-control-studio-pv]", savedReading(last.pv, last.units.pv));
  setText("[data-control-studio-mv]", savedReading(restored[restored.length - 1]?.mv, last.units.mv));
  setText("[data-control-studio-quality]", `${titleCase(last.quality)} · Output limit ${formatEngineeringNumber(last.outputLimits.minimum)}…${formatEngineeringNumber(last.outputLimits.maximum)} ${displayEngineeringUnit(last.units.mv)}`);
  applyValueStripHistoricalState({ badgeText: retainedProvenance ? "Imported recording" : "Recorded response", dimmed: true });
}

function renderLoopSample(
  snapshot: LoopMonitorSample,
  chart: ControlChartAdapter
): void {
  setText("[data-control-studio-source]", snapshot.source === "Physical" ? "Physical plant" : "Simulation");
  const studio = document.querySelector<HTMLElement>("[data-control-studio]");
  if (studio) studio.dataset.controlSource = snapshot.source.toLowerCase();
  // A live loop-monitor sample means the studio is watching the running loop again, not a
  // saved result — clear the shared historical flag and this surface's own badge/dim
  // state regardless of how it was set (finding W-16 follow-up).
  if (studio) {
    delete studio.dataset.controlStudioHistoricalResult;
  }
  applyValueStripHistoricalState(undefined);
  const physicalTopology = document.querySelector<HTMLElement>("[data-physical-topology]");
  if (physicalTopology) physicalTopology.hidden = snapshot.source !== "Physical";
  if (snapshot.source !== "Physical") {
    const reviewed = document.querySelector<HTMLInputElement>("[data-physical-topology-reviewed]");
    if (reviewed) reviewed.checked = false;
  }
  if (snapshot.availability === "unavailable") {
    setText("[data-control-studio-connection]", connectionForReason(snapshot.reason));
    setText("[data-control-studio-freshness]", `Historical · ${snapshot.reason}`);
    setOptionalText("[data-control-studio-controller-mode]", "Unavailable");
    setText("[data-control-studio-quality]", `Unavailable · ${snapshot.reason}`);
    setOptionalText("[data-control-studio-sp]", "—");
    setText("[data-control-studio-pv]", "—");
    setText("[data-control-studio-mv]", "—");
    return;
  }
  if (studio) delete studio.dataset.noLoopSamples;
  chart.setSource(snapshot.source);
  chart.setUnits(snapshot.processVariable.unit, snapshot.manipulatedVariable.unit);
  const quality = [snapshot.setpoint, snapshot.processVariable, snapshot.manipulatedVariable]
    .some(signal => signal?.quality === "forced") ? "bad" : "good";
  chart.setSamples(receivedSamples);
  setText("[data-control-studio-connection]", "Running");
  setOptionalText("[data-control-studio-controller-mode]", snapshot.controller.mode);
  const revision = document.querySelector<HTMLElement>("[data-control-studio-controller-revision]");
  if (revision) {
    revision.textContent = compactRevisionLabel(snapshot.controller.revision);
    revision.title = snapshot.controller.revision;
  }
  setText("[data-control-studio-freshness]", "Live");
  setOptionalText("[data-control-studio-sp]", snapshot.setpoint ? reading(snapshot.setpoint) : "Open loop");
  setText("[data-control-studio-pv]", reading(snapshot.processVariable));
  setText("[data-control-studio-mv]", reading(snapshot.manipulatedVariable));
  const qualityText = quality === "good" ? "Good" : "Forced signal";
  // "Output limit" (not bare "Output") names what the range actually is, and the
  // saturation word is title-cased to match the quality word instead of a raw lowercase
  // enum in the same sentence (finding W-11).
  setText("[data-control-studio-quality]",
    `${qualityText} · Output limit ${formatEngineeringNumber(snapshot.outputLimits.minimum)}…${formatEngineeringNumber(snapshot.outputLimits.maximum)} ${displayEngineeringUnit(snapshot.manipulatedVariable.unit)} · Saturation ${titleCase(snapshot.saturation)}`);
  renderRecordingActions(snapshot.recording);
}

function acceptMessage(messageId: string, envelope: unknown): boolean {
  if (receivedMessages.has(messageId)) return false;
  receivedMessages.set(messageId, JSON.stringify(envelope));
  if (receivedMessages.size > 1_024) {
    receivedMessages.delete(receivedMessages.keys().next().value as string);
  }
  return true;
}

function reading(signal: { value: number; unit: string }): string {
  const value = formatEngineeringNumber(signal.value);
  const unit = displayEngineeringUnit(signal.unit);
  return unit ? `${value} ${unit}` : value;
}

function savedReading(value: unknown, unit: unknown): string {
  const formatted = formatEngineeringNumber(value);
  const displayedUnit = displayEngineeringUnit(String(unit ?? ""));
  return formatted === "—" ? formatted : `${formatted}${displayedUnit ? ` ${displayedUnit}` : ""}`;
}

function setOptionalText(selector: string, value: string): void {
  const element = document.querySelector<HTMLElement>(selector);
  if (element) element.textContent = value;
}

function setText(selector: string, value: string): void {
  requiredElement<HTMLElement>(selector).textContent = value;
}

/** Applies (or clears) the live-value strip's "Saved result" badge and dimmed state. Owns
 * only DOM wiring; valueStripHistoricalState decides what to show. */
function applyValueStripHistoricalState(state: ValueStripHistoricalState | undefined): void {
  const container = document.querySelector<HTMLElement>(".values");
  if (!container) return;
  let badge = container.querySelector<HTMLElement>("[data-historical-result-badge]");
  if (!state) {
    if (badge) badge.hidden = true;
    container.style.opacity = "";
    return;
  }
  if (!badge) {
    badge = document.createElement("span");
    badge.dataset.historicalResultBadge = "true";
    badge.style.cssText = "margin:0 0 4px;padding:1px 6px;border-radius:3px;"
      + "font-size:11px;background:var(--vscode-badge-background);color:var(--vscode-badge-foreground);";
    container.prepend(badge);
  }
  badge.hidden = false;
  badge.textContent = state.badgeText;
  container.style.opacity = "0.72";
}

function first(values: readonly number[]): number | null {
  return values.length > 0 ? values[0] : null;
}

function display(value: number | null): string {
  return formatEngineeringNumber(value);
}

function vector(values: readonly number[]): string {
  return values.length > 0 ? values.map((value) => formatEngineeringNumber(value)).join(", ") : "—";
}

function list(values: readonly string[]): string {
  return values.length > 0 ? values.join(", ") : "None";
}

function titleCase(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

function connectionForReason(reason: string): string {
  if (reason === "Runtime is stopped") return "Stopped";
  if (reason === "Runtime is unreachable") return "Unreachable";
  if (reason === "Runtime authorization was rejected") return "Unauthorized";
  if (reason === "Plant status is unsupported") return "Unsupported";
  if (reason === "Running plant revision is stale") return "Stale";
  if (reason === "Runtime scan is stale") return "Stale";
  if (reason === "Plant no longer exists") return "Deleted";
  return "Unavailable";
}

function formatSimulationTime(nanoseconds: string): string {
  const value = BigInt(nanoseconds);
  if (value < 1_000_000n) return `${value / 1_000n} µs`;
  if (value < 1_000_000_000n) return `${value / 1_000_000n} ms`;
  const seconds = Number(value) / 1_000_000_000;
  return `${Number.isInteger(seconds) ? seconds : seconds.toFixed(3)} s`;
}
