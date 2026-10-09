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
    // The stage renders consult the shared gate, so a Method change cannot re-enable a blocked action; its reason stays visible.
    const block = operation.stageBlock();
    if (["supplied-model", "project", "recorded-fit"].includes(acquisitionMethod.value)) { operation.notice("model", block ?? ""); return; }
    const issue = methodIssue();
    operation.notice("model", issue ?? block ?? "");
    if (!issue && loop && limits?.durationSeconds !== undefined) post("control-studio.engineer.invalidate", { acquisitionSettings: { ...acquisitionPreferences(loop, acquisitionMethod.value as AcquisitionSettings["method"], limits), ...(reference?.signalId ? { setpointSignalId: reference.signalId } : {}) } });
  });
  let limits = loop ? initialExperimentLimits(loop) : undefined;
  let task: Partial<EngineerRequirements> = loop?.channels?.outputs[0]?.requirements ?? {};
  let scenarioDefinitions = loop?.verificationScenarios ?? [];
  // The plant-mapping gate and an in-progress simulator start both block the stage actions through the shared gate.
  let bindingsBlock: string | undefined;
  let startingExperiment = false;
  let waitingForLimits = false;
  // Done saves the requirements; only the editor that a paused Tune opens resumes that start. Cancel and manual edits never do.
  const editRequirements = (resumeTune?: () => void): void => {
    if (!loop) return;
    editControlRequirements({ ...loop, channels: channelMapping }, task, next => {
      const { scenarioDefinitions: scenarios, ...requirements } = next as EngineerRequirements & { scenarioDefinitions?: typeof scenarioDefinitions };
      task = requirements;
      if (scenarios) scenarioDefinitions = scenarios;
      post("control-studio.engineer.invalidate", next as unknown as Record<string, unknown>);
    }, { scenarioDefinitions: scenarioDefinitions as unknown as Record<string, unknown>[], processSymbol: loop.signals.processVariable.symbol,
      setpointSymbol: loop.signals.setpoint?.symbol, samplePeriodMs: loop.controller.taskPeriodMs,
      durationSeconds: task.durationSeconds ?? 0, processMinimum: loop.signals.processVariable.range.minimum, processMaximum: loop.signals.processVariable.range.maximum }, resumeTune);
  };
  root.querySelector<HTMLButtonElement>("[data-studio-requirements]")!.addEventListener("click", () => editRequirements());
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
  /** Performs the typed next action of a failure on its own page (Specification 35 A11.2); it never starts work. */
  const runNextAction = (kind: string, page: Page): void => {
    switch (kind) {
      case "connect-runtime":
      case "open-loop-setup": post("control-studio.host-command", { command: kind }); return;
      case "open-limits": root.querySelector<HTMLButtonElement>("[data-studio-limits]")!.click(); return;
      case "open-requirements": root.querySelector<HTMLButtonElement>("[data-studio-requirements]")!.click(); return;
      case "open-channels": root.querySelector<HTMLButtonElement>("[data-engineer-channels]")?.click(); return;
      case "choose-method": root.querySelector<HTMLSelectElement>(page === "model" ? "[data-engineer-acquisition-method]" : "[data-engineer-tuning-method]")?.focus(); return;
      case "identify": select("model", true); root.querySelector<HTMLButtonElement>("[data-engineer-find]")?.focus(); return;
      case "tune": select("tune", true); root.querySelector<HTMLButtonElement>("[data-engineer-tune]")?.focus(); return;
      case "validate": select("verify", true); root.querySelector<HTMLButtonElement>("[data-engineer-verify]")?.focus(); return;
      case "restore": {
        const apply = root.querySelector<HTMLButtonElement>("[data-engineer-apply]");
        if (apply?.textContent === "Restore previous settings") apply.click();
        return;
      }
      default: return;
    }
  };
  const operation = new ThreePageOperation(root, post, select, field => {
    if (field === "requirements") editRequirements(() => start("tune"));
    if (field === "limits") { waitingForLimits = true; root.querySelector<HTMLButtonElement>("[data-studio-limits]")!.click(); }
  }, () => bindingsBlock ?? (startingExperiment ? "Starting simulator…" : undefined), runNextAction);
  const resetButton = root.querySelector<HTMLButtonElement>("[data-studio-reset-chart]");
  if (resetButton) operation.bindReset(resetButton, () => Boolean(experiments?.busy || startingExperiment || application.active || recovery?.active));
  const tuning = loop ? new ThreePageTuning(root, loop, post, () => controller?.selectedFamily ?? "automatic", () => controller?.choice ?? "automatic") : undefined;
  const application = new ThreePageApplication(root, post, () => operation.busy, message => operation.notice("verify", message),
    failure => operation.failure("verify", failure));
  recovery = new ThreePageRecovery(root, post, () => operation.busy || application.active);
  const suppliedModel = new ThreePageSuppliedModel(root, post, () => operation.busy || application.active || recovery?.active === true,
    message => operation.notice("model", message));
  const experiments = loop?.experiment?.execution === "project-scripted" ? new ThreePageExperiments(root, loop, post, () => operation.stageBlock()) : undefined;
  let mimoRecordingId: string | undefined;
  const identification = new ThreePageIdentification(root, post, () => root.querySelector<HTMLButtonElement>("[data-identify-import]")!.click(), () => operation.stageBlock());
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
  // The Tuning method select holds tuning methods only; the analysis tools open from Analyze… on the Tune row.
  const analysisLabels: Record<string, string> = { "root-locus": "Root locus", routh: "Routh–Hurwitz stability", poles: "Pole-zero map", frequency: "Bode (frequency response)",
    "periodic-frequency": "Measured frequency response", feedback: "Nyquist and loop response", "lead-lag": "Lead/lag design", response: "Initial-state response",
    "state-structure": "Controllability and observability", rga: "Relative gain array", "measured-mimo": "Measured MIMO paths" };
  /** The analysis tool shown in place of the tuning result, or undefined while tuning. */
  let analysisChoice: string | undefined;
  const measuredMimoAvailable = (): boolean => Boolean(mimoRecordingId && loop?.channels && loop.channels.inputs.length >= 2);
  const availableAnalysis = (): string[] => [...analysis.availableTools(), ...(measuredMimoAvailable() ? ["measured-mimo"] : [])];
  const chooseAnalysis = (tool: string): void => {
    analysisChoice = tool;
    analysis.setTool(tool === "measured-mimo" ? undefined : tool as ThreePageAnalysisTool);
    mimoAnalysis.setSelected(tool === "measured-mimo");
    synchronizeTools();
  };
  /** Close returns the page to the tuning result. */
  const closeAnalysis = (): void => {
    analysisChoice = undefined;
    analysis.setTool(undefined); mimoAnalysis.setSelected(false);
    tuning?.refresh();
    operation.synchronize(); synchronizeTools();
  };
  const analysisChooser = new ThreePageAnalysisChooser(root, { labels: analysisLabels, tools: availableAnalysis,
    choose: tool => chooseAnalysis(tool), close: () => closeAnalysis() });
  const synchronizeTools = () => {
    const tools = availableAnalysis();
    if (analysisChoice !== undefined && !tools.includes(analysisChoice)) analysisChoice = undefined;
    const selected = analysisChoice !== undefined;
    const measuredMimoSelected = analysisChoice === "measured-mimo";
    analysisChooser.synchronize(selected);
    root.querySelector<HTMLElement>('[data-studio-message="tune"]')!.hidden = selected;
    mimoAnalysis.setSelected(measuredMimoSelected);
    if (selected) {
      root.querySelector<HTMLElement>("[data-studio-tuning-result]")!.hidden = true;
      const button = root.querySelector<HTMLButtonElement>("[data-engineer-tune]")!;
      // While a run is busy the button keeps its Abort label; otherwise it names the chosen analysis and waits for the gates.
      if (!operation.busy) {
        const label = measuredMimoSelected ? mimoAnalysis.actionLabel() : analysis.actionLabel();
        button.textContent = label ?? "Analysis ready";
        button.disabled = !label || operation.stageBlock() !== undefined;
      }
    }
    root.dataset.analysisSelected = String(selected);
    experiments?.setSelectedMethod(acquisitionMethod.value === "project");
    identification.setSelectedMethod(acquisitionMethod.value === "recorded-fit");
    if (experiments?.busy || startingExperiment) {
      for (const control of [...runLockedControls(root), ...Array.from(root.querySelectorAll<HTMLButtonElement>("[data-engineer-tune],[data-engineer-verify],[data-engineer-apply]"))]) control.disabled = true;
    }
    if (startingExperiment) root.querySelector<HTMLButtonElement>("[data-engineer-find]")!.disabled = true;
    recovery?.synchronize();
  };
  tuningMethod.addEventListener("change", () => { operation.synchronize(); synchronizeTools(); });
  synchronizeTools();
  window.addEventListener("trust-control-studio-identification-open-recording", () => root.querySelector<HTMLButtonElement>("[data-identify-import]")!.click());
  window.addEventListener("trust-control-studio-identification-method-required", () => { acquisitionMethod.value = "recorded-fit"; synchronizeTools(); });
  window.addEventListener("trust-control-studio-identification-presentation", event => {
    const { presentation, model } = (event as CustomEvent<{ presentation?: IdentifiedModelPresentation; model?: { kind: string } }>).detail;
    if (!presentation) return;
    renderModelPresentation(root, presentation, model?.kind ? modelKindLabel(model.kind) : "Plant model", decodeGeneralModel(model));
    window.dispatchEvent(new CustomEvent("trust-control-studio-engineer", { detail: { identificationPresentation: presentation } }));
  });
  let recordingJson: string | undefined;
  const identifyRecording = () => {
    if ((!recordingJson && !bindings?.recordingSelection) || operation.busy || application.active || recovery?.active) return;
    operation.start("recording", { json: recordingJson, ...(bindings?.recordingSelection ? { recordingSelection: bindings.recordingSelection } : {}),
      task, family: controller?.choice, applicationMode: "auto", tuningPreferences: tuning?.preferences(), pinnedValues: tuning?.pinnedValues() });
  };
  const bindings = loop && controller ? new ThreePageBindings(root, loop, controller, post, (page, message) => operation.notice(page, message), (valid, reason) => { bindingsBlock = valid ? undefined : reason; operation.synchronize(); application.synchronize(); recovery?.synchronize(); }, identifyRecording) : undefined;
  bindRecordingOpen(root, () => operation.busy || application.active || recovery?.active === true || bindings?.canRun === false,
    json => { if (identification.isSelected) identification.openRecording(json); else { recordingJson = json; identifyRecording(); } },
    message => operation.notice("model", message));
  const start = (from: StudioStart) => {
    if (recovery?.active) return;
    if (application.active) { application.abort(); return; }
    if (from === "tune" && !operation.busy && analysisChoice === "measured-mimo") { mimoAnalysis.run(); return; }
    if (from === "tune" && !operation.busy && analysisChoice !== undefined) { analysis.run(); synchronizeTools(); return; }
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
    // A manual-tuning preview is a prediction: it draws the plot and never counts as a finished Tune, so Validate stays locked.
    // Its refusal names the reason in the Tune area. The analysis presenter never sees these messages.
    if (stage === "tune" && payload.status === "preview") {
      operation.notice("tune", typeof payload.reason === "string" ? payload.reason : "");
      synchronizeTools();
      return;
    }
    if (stage === "tune" && payload.status === "failed") {
      operation.failure("tune", studioFailureOf(payload) ?? { message: "The predicted response is unavailable.", nextAction: { kind: "none", label: "" } });
      synchronizeTools();
      return;
    }
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
      operation.synchronize();
      if (result.ok) operation.notice("model", "");
      else operation.failure("model", studioFailureOf(result as Record<string, unknown>) ?? { message: "The simulator could not start.", nextAction: { kind: "none", label: "" } });
      if (result.ok) experiments?.primaryAction();
    }
    synchronizeTools();
  }, () => operation.modelArrived());
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
import { runLockedControls } from "./runLock";

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
import { renderModelPresentation } from "./threePageModel";
import { studioFailureOf } from "./studioFailure";
import { ThreePageAnalysisChooser } from "./threePageAnalysisChooser";
import { decodeGeneralModel } from "../generalModel";
import type { IdentifiedModelPresentation } from "../identification";
