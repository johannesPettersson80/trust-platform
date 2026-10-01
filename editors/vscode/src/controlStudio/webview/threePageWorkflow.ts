declare function acquireVsCodeApi(): { postMessage(message: unknown): void };

type Page = StudioPage;
const root = document.querySelector<HTMLElement>("[data-studio-three-page]");
if (root) {
  const api = acquireVsCodeApi();
  const post = (kind: string, payload: Record<string, unknown> = {}) => api.postMessage({
    schemaVersion: 1, kind, messageId: crypto.randomUUID(),
    projectId: root.dataset.projectId, editorSessionId: root.dataset.editorSessionId,
    loopId: root.dataset.loopId, expectedLoopRevision: root.dataset.loopRevision, payload,
  });
  const persistView = () => post("control-studio.navigation.select", {
    stage: root.dataset.selectedStage ?? "model", view: {
      acquisitionMethod: root.querySelector<HTMLSelectElement>("[data-engineer-acquisition-method]")?.value ?? "automatic",
      experimentSignalIds: Array.from(root.querySelector<HTMLSelectElement>("[data-three-page-experiment-signals]")?.selectedOptions ?? [], option => option.value),
    },
  });
  root.querySelector("[data-three-page-experiment-signals]")?.addEventListener("change", persistView);
  const tabs = Array.from(root.querySelectorAll<HTMLButtonElement>("[data-engineer-stage-tab]"));
  for (const select of Array.from(root.querySelectorAll<HTMLSelectElement>("select[multiple]"))) initializeMultiSelect(select);
  const loop = JSON.parse(root.dataset.studioLoop ?? "null") as ControlLoopDefinition | null;
  const controller = loop ? new ThreePageController(root, loop, post) : undefined;
  let channelMapping = loop?.channels;
  root.querySelector<HTMLButtonElement>("[data-engineer-channels]")?.addEventListener("click", () => {
    if (!loop) return;
    editControllerChannels({ ...loop, channels: channelMapping }, next => post("control-studio.engineer.invalidate", { channelMapping: next }));
  });
  const reference = loop ? new ThreePageReference(root, loop) : undefined;
  const acquisitionMethod = root.querySelector<HTMLSelectElement>("[data-engineer-acquisition-method]")!;
  const methodIssue = () => loop ? identificationMethodApplicability(reference?.acquisitionLoop ?? loop, acquisitionMethod.value as AcquisitionSettings["method"]).reason : undefined;
  acquisitionMethod.addEventListener("change", () => {
    persistView();
    operation.synchronize();
    experiments?.setSelectedMethod(acquisitionMethod.value === "project");
    identification.setSelectedMethod(acquisitionMethod.value === "recorded-fit");
    if (["supplied-model", "project", "recorded-fit"].includes(acquisitionMethod.value)) { operation.notice("model", ""); return; }
    const issue = methodIssue();
    operation.notice("model", issue ?? "");
    if (!issue && loop && limits?.durationSeconds !== undefined) post("control-studio.engineer.invalidate", { acquisitionSettings: { ...acquisitionPreferences(loop, acquisitionMethod.value as AcquisitionSettings["method"], limits), ...(reference?.signalId ? { setpointSignalId: reference.signalId } : {}) } });
  });
  let limits = loop ? initialExperimentLimits(loop) : undefined;
  let task: Partial<EngineerRequirements> = loop?.channels?.outputs[0]?.requirements ?? {};
  let scenarioDefinitions = loop?.verificationScenarios ?? [];
  let waitingForRequirements = false;
  let waitingForLimits = false;
  root.querySelector<HTMLButtonElement>("[data-studio-requirements]")!.addEventListener("click", () => {
    if (!loop) return;
    editControlRequirements({ ...loop, channels: channelMapping }, task, next => {
      const { scenarioDefinitions: scenarios, ...requirements } = next as EngineerRequirements & { scenarioDefinitions?: typeof scenarioDefinitions };
      task = requirements;
      if (scenarios) scenarioDefinitions = scenarios;
      post("control-studio.engineer.invalidate", next as unknown as Record<string, unknown>);
      if (waitingForRequirements) { waitingForRequirements = false; start("tune"); }
    }, { scenarioDefinitions: scenarioDefinitions as unknown as Record<string, unknown>[], processSymbol: loop.signals.processVariable.symbol,
      setpointSymbol: loop.signals.setpoint?.symbol, samplePeriodMs: loop.controller.taskPeriodMs,
      durationSeconds: task.durationSeconds ?? 0, processMinimum: loop.signals.processVariable.range.minimum, processMaximum: loop.signals.processVariable.range.maximum });
  });
  root.querySelector<HTMLButtonElement>("[data-studio-limits]")!.addEventListener("click", () => {
    if (!loop || !limits) return;
    editExperimentLimits(loop, limits, next => {
      limits = next;
      const method = root.querySelector<HTMLSelectElement>("[data-engineer-acquisition-method]")!.value as AcquisitionSettings["method"];
      post("control-studio.engineer.invalidate", { acquisitionSettings: acquisitionPreferences(loop, method, next) });
      if (waitingForLimits) { waitingForLimits = false; start("find"); }
    }, waitingForLimits, () => { waitingForLimits = false; });
  });
  let recovery: ThreePageRecovery | undefined;
  const select = (page: Page, persist: boolean) => {
    root.dataset.selectedStage = page;
    for (const tab of tabs) {
      const selected = tab.dataset.engineerStageTab === page;
      tab.setAttribute("aria-selected", String(selected));
      tab.tabIndex = selected ? 0 : -1;
    }
    for (const panel of Array.from(root.querySelectorAll<HTMLElement>("[data-studio-page]"))) panel.hidden = panel.dataset.studioPage !== page;
    root.querySelector<HTMLElement>("[data-studio-application]")!.hidden = page !== "verify";
    recovery?.synchronize();
    if (persist) persistView();
    window.dispatchEvent(new CustomEvent("trust-control-studio-three-page", { detail: { page } }));
  };
  const operation = new ThreePageOperation(root, post, select, field => {
    if (field === "requirements") {
      waitingForRequirements = true;
      root.querySelector<HTMLButtonElement>("[data-studio-requirements]")!.click();
    }
    if (field === "limits") { waitingForLimits = true; root.querySelector<HTMLButtonElement>("[data-studio-limits]")!.click(); }
  });
  root.querySelector("[data-studio-reset-chart]")?.addEventListener("click", () => {
    if (experiments?.busy || startingExperiment || application.active || recovery?.active) {
      operation.notice("model", "Finish or abort the current operation before resetting the study."); return;
    }
    operation.reset();
  });
  const tuning = loop ? new ThreePageTuning(root, loop, post, () => controller?.selectedFamily ?? "automatic", () => controller?.choice ?? "automatic") : undefined;
  const application = new ThreePageApplication(root, post, () => operation.busy, message => operation.notice("verify", message));
  recovery = new ThreePageRecovery(root, post, () => operation.busy || application.active);
  const suppliedModel = new ThreePageSuppliedModel(root, post, () => operation.busy || application.active || recovery?.active === true,
    message => operation.notice("model", message));
  const experiments = loop?.experiment?.execution === "project-scripted" ? new ThreePageExperiments(root, loop, post) : undefined;
  let startingExperiment = false;
  let mimoRecordingId: string | undefined;
  const identification = new ThreePageIdentification(root, post, () => root.querySelector<HTMLButtonElement>("[data-identify-import]")!.click());
  const analysis = initializeThreePageAnalysis(root, post);
  const mimoAnalysis = initializeThreePageMimoAnalysis(root);
  const configureMimoRecording = (recording: Record<string, unknown> | undefined): void => {
    if (!loop?.channels || !recording) return;
    const evidence = recording.evidence && typeof recording.evidence === "object" ? recording.evidence
      : recording.acquisition && typeof recording.acquisition === "object" ? recording.acquisition : undefined;
    if (!evidence || typeof (evidence as Record<string, unknown>).recordingId !== "string") return;
    const recordingId = String((evidence as Record<string, unknown>).recordingId);
    mimoRecordingId = recordingId;
    mimoAnalysis.setContext({ inputIds: loop.channels.inputs.map(input => input.modelChannelId), recordings: [{ id: recordingId, label: `Retained recording · ${recordingId}` }],
      onAnalyze: (selectedRecordingId, trials) => post("control-studio.mimo.analyze", { recordingId: selectedRecordingId, trials }) });
  };
  analysis.setAvailable({ declared: loop?.analysis });
  const tuningMethod = root.querySelector<HTMLSelectElement>("[data-engineer-tuning-method]")!;
  const analysisLabels = { "root-locus": "Root locus", routh: "Routh–Hurwitz", poles: "Poles and zeros", frequency: "Frequency response", "periodic-frequency": "Measured frequency response", feedback: "Loop response", "lead-lag": "Lead/lag design", response: "Initial-state response", "state-structure": "Controllability and observability", rga: "Relative gain array", "measured-mimo": "Measured MIMO paths" };
  for (const [tool, label] of Object.entries(analysisLabels)) {
    const option = root.ownerDocument.createElement("option"); option.value = `analysis:${tool}`; option.textContent = label; tuningMethod.append(option);
  }
  const synchronizeTools = () => {
    const available = analysis.availableTools();
    for (const option of Array.from(tuningMethod.options)) if (option.value.startsWith("analysis:")) {
      const tool = option.value.slice(9);
      const measuredMimoAvailable = tool === "measured-mimo" && Boolean(mimoRecordingId && loop?.channels && loop.channels.inputs.length >= 2);
      option.hidden = option.disabled = tool === "measured-mimo" ? !measuredMimoAvailable : !available.includes(tool as ThreePageAnalysisTool);
    }
    const selected = tuningMethod.value.startsWith("analysis:");
    const measuredMimoSelected = tuningMethod.value === "analysis:measured-mimo";
    root.querySelector<HTMLElement>('[data-studio-message="tune"]')!.hidden = selected;
    mimoAnalysis.setSelected(measuredMimoSelected);
    if (selected) {
      const button = root.querySelector<HTMLButtonElement>("[data-engineer-tune]")!;
      button.textContent = analysis.actionLabel() ?? "Analysis ready"; button.disabled = !analysis.actionLabel();
      if (measuredMimoSelected) { button.textContent = mimoAnalysis.actionLabel(); button.disabled = false; }
      root.querySelector<HTMLElement>("[data-studio-tuning-result]")!.hidden = true;
    }
    root.dataset.analysisSelected = String(selected);
    experiments?.setSelectedMethod(acquisitionMethod.value === "project");
    identification.setSelectedMethod(acquisitionMethod.value === "recorded-fit");
    if (experiments?.busy || startingExperiment) {
      for (const control of Array.from(root.querySelectorAll<HTMLButtonElement | HTMLInputElement | HTMLSelectElement>("input,select,[data-engineer-tune],[data-engineer-verify],[data-engineer-apply],[data-identify-import],[data-studio-limits],[data-studio-requirements]"))) control.disabled = true;
    }
    if (startingExperiment) root.querySelector<HTMLButtonElement>("[data-engineer-find]")!.disabled = true;
    recovery?.synchronize();
  };
  tuningMethod.addEventListener("change", () => {
    analysis.setTool(tuningMethod.value.startsWith("analysis:") ? tuningMethod.value.slice(9) as ThreePageAnalysisTool : undefined);
    operation.synchronize(); synchronizeTools();
  });
  synchronizeTools();
  window.addEventListener("trust-control-studio-identification-open-recording", () => root.querySelector<HTMLButtonElement>("[data-identify-import]")!.click());
  window.addEventListener("trust-control-studio-identification-method-required", () => { acquisitionMethod.value = "recorded-fit"; synchronizeTools(); });
  window.addEventListener("trust-control-studio-identification-presentation", event => {
    const { presentation, model } = (event as CustomEvent<{ presentation?: { equation: string }; model?: { kind: string } }>).detail;
    if (!presentation) return;
    root.querySelector<HTMLElement>("[data-studio-model-type]")!.textContent = model?.kind ? modelKindLabel(model.kind) : "Plant model";
    root.querySelector<HTMLElement>("[data-studio-model-equation]")!.textContent = presentation.equation;
    root.querySelector<HTMLElement>("[data-studio-model-result]")!.hidden = false;
    window.dispatchEvent(new CustomEvent("trust-control-studio-engineer", { detail: { identificationPresentation: presentation } }));
  });
  let recordingJson: string | undefined;
  const identifyRecording = () => {
    if ((!recordingJson && !bindings?.recordingSelection) || operation.busy || application.active || recovery?.active) return;
    operation.start("recording", { json: recordingJson, ...(bindings?.recordingSelection ? { recordingSelection: bindings.recordingSelection } : {}),
      task, family: controller?.choice, applicationMode: "auto", tuningPreferences: tuning?.preferences(), pinnedValues: tuning?.pinnedValues() });
  };
  const bindings = loop && controller ? new ThreePageBindings(root, loop, controller, post, (page, message) => operation.notice(page, message), () => { operation.synchronize(); application.synchronize(); recovery?.synchronize(); }, identifyRecording) : undefined;
  bindRecordingOpen(root, () => operation.busy || application.active || recovery?.active === true || bindings?.canRun === false,
    json => { if (identification.isSelected) identification.openRecording(json); else { recordingJson = json; identifyRecording(); } },
    message => operation.notice("model", message));
  const start = (from: StudioStart) => {
    if (recovery?.active) return;
    if (application.active) { application.abort(); return; }
    if (from === "tune" && !operation.busy && tuningMethod.value === "analysis:measured-mimo") { mimoAnalysis.run(); return; }
    if (from === "tune" && !operation.busy && analysis.choice()) { analysis.run(); synchronizeTools(); return; }
    if (from === "find" && !operation.busy && suppliedModel.selected) { suppliedModel.open(); return; }
    if (from === "find" && !operation.busy && identification.isSelected) { identification.primaryAction(); return; }
    if (experiments?.isSelected && (experiments.busy || from === "find")) {
      if (startingExperiment) return;
      if (!experiments.busy && !experiments.hasReviewedPlan) {
        startingExperiment = true;
        operation.notice("model", "Starting simulator…");
        root.querySelector<HTMLButtonElement>("[data-engineer-find]")!.disabled = true;
        post("control-studio.runtime.start", { runtimeId: loop!.runtimeId, runtimeMode: "simulate" });
      } else experiments.primaryAction();
      return;
    }
    if (from === "find" && !operation.busy) { recordingJson = undefined; bindings?.useLiveSignals(); }
    if (bindings?.canRun === false) { bindings.explain(); return; }
    if (!loop || !limits) return;
    if (!operation.busy && from === "find" && methodIssue()) { operation.notice("model", methodIssue()!); return; }
    if (!operation.busy && from === "find" && limits.durationSeconds === undefined) {
      waitingForLimits = true; root.querySelector<HTMLButtonElement>("[data-studio-limits]")!.click(); return;
    }
    const method = root.querySelector<HTMLSelectElement>("[data-engineer-acquisition-method]")!.value as AcquisitionSettings["method"];
    const requestedTuningMethod = root.querySelector<HTMLSelectElement>("[data-engineer-tuning-method]")!.value;
    const currentTask = { ...task };
    if (requestedTuningMethod === "SIMC" || requestedTuningMethod === "AMIGO") currentTask.tuningMethod = requestedTuningMethod;
    else delete currentTask.tuningMethod;
    try {
      operation.start(from, { ...(from === "find" ? { acquisitionSettings: { ...acquisitionPreferences(loop, method, limits), ...(reference?.signalId ? { setpointSignalId: reference.signalId } : {}) } } : {}), task: currentTask, family: requestedTuningMethod === "manual" ? controller?.selectedFamily : controller?.choice, applicationMode: "auto",
        tuningPreferences: tuning?.preferences(from === "tune" && !operation.busy), pinnedValues: tuning?.pinnedValues() });
    } catch (error) { operation.notice("tune", error instanceof Error ? error.message : String(error)); }
  };
  for (const action of ["find", "tune", "verify"] as const) root.querySelector<HTMLButtonElement>(`[data-engineer-${action}]`)!.addEventListener("click", () => start(action));
  for (const [index, tab] of tabs.entries()) {
    tab.addEventListener("click", () => { operation.navigated(); select(tab.dataset.engineerStageTab as Page, true); });
    tab.addEventListener("keydown", event => {
      const next = event.key === "ArrowRight" ? (index + 1) % tabs.length : event.key === "ArrowLeft" ? (index + tabs.length - 1) % tabs.length : event.key === "Home" ? 0 : event.key === "End" ? tabs.length - 1 : undefined;
      if (next === undefined) return;
      event.preventDefault(); tabs[next].focus(); tabs[next].click();
    });
  }
  select("model", false);
  bindThreePageHostState(root, (stage, payload) => {
    analysis.receive(stage, payload);
    if (stage !== "engineer" && stage !== "flow" && stage !== "bindings") {
      suppliedModel.receive(stage, payload);
      if (stage === "identify") identification.receive(stage, payload);
      if (stage === "parameters") experiments?.receive("parameters", payload);
      if (stage === "mimo") {
        const result = decodeMimoAnalysisResult(payload);
        if (result) mimoAnalysis.receive(result);
      }
      synchronizeTools();
      return;
    }
    if (stage === "bindings") { bindings?.receive(payload); return; }
    if (payload.recordingCatalog || payload.status === "model-ready") bindings?.receive(payload);
    if (payload.channelMapping) channelMapping = payload.channelMapping as typeof channelMapping;
    recovery?.receive(payload);
    controller?.receive(payload);
    renderRelayCycle(root, loop, payload);
    tuning?.receive(payload);
    renderValidationResults(root, payload);
    if (payload.task && typeof payload.task === "object") task = payload.task as EngineerRequirements;
    if (Array.isArray(payload.scenarioDefinitions)) scenarioDefinitions = payload.scenarioDefinitions;
    if (loop && payload.acquisitionSettings && typeof payload.acquisitionSettings === "object") {
      const saved = payload.acquisitionSettings as AcquisitionSettings;
      limits = { ...initialExperimentLimits(loop), durationSeconds: saved.durationSeconds,
        ...(saved.inputRanges ? { inputRanges: saved.inputRanges } : {}), ...(saved.outputRanges ? { outputRanges: saved.outputRanges } : {}),
        ...(saved.inputRange ? { inputMinimum: saved.inputRange.minimum, inputMaximum: saved.inputRange.maximum } : {}),
        ...(saved.outputRange ? { outputMinimum: saved.outputRange.minimum, outputMaximum: saved.outputRange.maximum } : {}) };
      root.querySelector<HTMLSelectElement>("[data-engineer-acquisition-method]")!.value = saved.method === "configured" ? "automatic" : saved.method;
      reference?.restore(saved.setpointSignalId);
    }
    operation.receive(stage, payload);
    application.receive(payload);
    bindings?.synchronize();
    recovery?.synchronize();
    suppliedModel.receive(stage, payload);
    synchronizeTools();
  }, select, message => {
    const rawPayload = message.payload as { recording?: Record<string, unknown>; result?: Record<string, unknown> } | undefined;
    if (message.kind === "control-studio.identification-recording") {
      identification.receive("recording", message.payload);
      if (isRecord(message.payload) && message.payload.status === "imported" && isRecord(message.payload.recording)) {
        window.dispatchEvent(new CustomEvent("trust-control-studio-recording-opened", { detail: { recording: message.payload.recording } }));
      }
      if (isRecord(message.payload)) analysis.receive("recording", message.payload);
      if (message.payload && typeof rawPayload!.recording === "object") configureMimoRecording(rawPayload!.recording as Record<string, unknown>);
    }
    if (message.kind === "control-studio.experiment.result") {
      experiments?.receive(message.phase as Parameters<ThreePageExperiments["receive"]>[0], message.payload);
      if (message.phase === "terminal" && isRecord(message.payload) && message.payload.status === "completed"
        && isRecord(message.payload.recording) && Array.isArray(message.payload.recording.rawSamples)
        && message.payload.recording.rawSamples.length > 1 && !experiments?.busy) {
        identification.receive("recording", { status: "imported" });
      }
      const retained = completedExperimentRecording(message);
      if (retained) analysis.receive("recording", { recording: retained });
      if (!experiments?.busy) operation.synchronize();
    }
    if (message.kind === "control-studio.recording.evidence") experiments?.receive("recording", message.payload);
    if (message.kind === "control-studio.recording.evidence" && message.payload && typeof rawPayload!.recording === "object") {
      if (isRecord(message.payload)) analysis.receive("recording", message.payload);
      configureMimoRecording(rawPayload!.recording as Record<string, unknown>);
    }
    if (message.kind === "control-studio.experiment.result" && message.payload && typeof rawPayload!.recording === "object") configureMimoRecording(rawPayload!.recording as Record<string, unknown>);
    if (message.kind === "control-studio.workflow-result" && message.payload && typeof rawPayload!.result === "object") {
      const result = rawPayload!.result as Record<string, unknown>;
      if (["restored", "opened"].includes(String(message.phase)) && isRecord(result.identificationStudy)) {
        identification.receive("study", result.identificationStudy);
      }
      if (isRecord(result.recording) && isRecord(result.experiment)) experiments?.receive("recording", { recording: result.recording, experiment: result.experiment });
      if (result.recording && typeof result.recording === "object") analysis.receive("recording", { recording: result.recording });
      if (result.recording && typeof result.recording === "object") configureMimoRecording(result.recording as Record<string, unknown>);
    }
    if (message.kind === "control-studio.runtime.start.result" && startingExperiment) {
      startingExperiment = false;
      const result = message.result as { ok: boolean; failure?: { message: string } };
      operation.synchronize(); operation.notice("model", result.ok ? "" : result.failure?.message ?? "The simulator could not start.");
      if (result.ok) experiments?.primaryAction();
    }
    synchronizeTools();
  });
  root.dataset.ready = "true";
  post("control-studio.ready");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
import { modelKindLabel } from "../failureMessages";
import type { ControlLoopDefinition } from "../loopDefinition";
import type { AcquisitionSettings } from "../acquisitionSettings";
import { acquisitionPreferences, editExperimentLimits, initialExperimentLimits } from "./threePageLimits";
import { editControlRequirements } from "./threePageRequirements";
import type { EngineerRequirements } from "../engineerVerificationPlan";
import { bindThreePageHostState, type StudioPage } from "./threePageHostState";
import { ThreePageOperation, type StudioStart } from "./threePageOperation";
import { renderValidationResults } from "./threePageResults";
import { renderRelayCycle } from "./threePageRelayCycle";
import { ThreePageApplication } from "./threePageApplication";
import { ThreePageController } from "./threePageController";
import { bindRecordingOpen } from "./threePageRecording";
import { ThreePageTuning } from "./threePageTuning";
import { ThreePageBindings } from "./threePageBindings";

import { initializeMultiSelect } from "../../webview/multiSelect";

import { ThreePageRecovery } from "./threePageRecovery";

import { editControllerChannels } from "./threePageChannels";

import { identificationMethodApplicability } from "../identificationMethodApplicability";

import { ThreePageReference } from "./threePageReference";
import { ThreePageSuppliedModel } from "./threePageSuppliedModel";
import { ThreePageExperiments } from "./threePageExperiments";
import { ThreePageIdentification } from "./threePageIdentification";
import { initializeThreePageAnalysis, type ThreePageAnalysisTool } from "./threePageAnalysis";
import { completedExperimentRecording } from "./periodicRecording";
import { decodeMimoAnalysisResult, initializeThreePageMimoAnalysis } from "./threePageMimoAnalysis";
