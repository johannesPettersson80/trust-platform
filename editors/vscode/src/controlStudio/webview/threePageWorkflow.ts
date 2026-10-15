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
  const persistView = () => {
    const stage = root.dataset.selectedStage ?? "model";
    post("control-studio.navigation.select", {
      stage, view: {
        acquisitionMethod: root.querySelector<HTMLSelectElement>("[data-engineer-acquisition-method]")?.value ?? "automatic",
        experimentSignalIds: Array.from(root.querySelector<HTMLSelectElement>("[data-three-page-experiment-signals]")?.selectedOptions ?? [], option => option.value),
      },
      // The analysis the Tune page shows is kept with that page, so a parked loop's reopened page calculates it again (B9.9); null says none is shown.
      ...(stage === "tune" ? { analysis: mimoAnalysis.shown() ?? analysis.shown() ?? null } : {}),
    });
  };
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
  /** The sections of the methods that have their own page area: the recorded-data fit's and the project test's (B3.3). */
  const showAcquisitionMethod = (): void => {
    experiments?.setSelectedMethod(acquisitionMethod.value === "project");
    identification.setSelectedMethod(acquisitionMethod.value === "recorded-fit");
  };
  acquisitionMethod.addEventListener("change", () => {
    persistView();
    operation.synchronize();
    showAcquisitionMethod();
    // The stage renders consult the shared gate, so a Method change cannot re-enable a blocked action; its reason stays visible.
    const block = operation.stageBlock();
    const method = acquisitionMethod.value as AcquisitionSettings["method"];
    // The recorded-data fit, the supplied model and the project test have no campaign to check (B3.6, B3.7, C12): no method issue applies.
    const issue = ["supplied-model", "project", "recorded-fit"].includes(method) ? undefined : methodIssue();
    operation.notice("model", issue ?? block ?? "");
    // A live method is a study input (A8.1). The opened file, the supplied model and the project test start no campaign: the page keeps
    // them as its view (`persistView`), so a reopened loop shows them again (`pageView`).
    if (!["project", "supplied-model", "recorded-fit"].includes(method) && loop && limits?.durationSeconds !== undefined) post("control-studio.engineer.invalidate", { acquisitionSettings: { ...acquisitionPreferences(loop, method, limits), ...(reference?.signalId ? { setpointSignalId: reference.signalId } : {}) } });
  });
  let limits = loop ? initialExperimentLimits(loop) : undefined;
  let task: Partial<EngineerRequirements> = loop?.channels?.outputs[0]?.requirements ?? {};
  let scenarioDefinitions = loop?.verificationScenarios ?? [];
  // The plant-mapping gate and an in-progress simulator start both block the stage actions through the shared gate.
  let bindingsBlock: string | undefined;
  let startingExperiment = false;
  /** True while the engine serves the opened loop: it answered `engineer` `opened`; a parked loop's view restore clears it. */
  let engineServesLoop = false;
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
  }, () => bindingsBlock ?? (startingExperiment ? "Starting simulator…" : undefined), runNextAction, () => application.synchronize());
  const resetButton = root.querySelector<HTMLButtonElement>("[data-studio-reset-chart]");
  if (resetButton) operation.bindReset(resetButton, () => Boolean(experiments?.busy || startingExperiment || application.active || recovery?.active));
  const tuning = loop ? new ThreePageTuning(root, loop, post, () => controller?.selectedFamily ?? "automatic", () => controller?.choice ?? "automatic",
    family => controller?.symbolsOf(family)) : undefined;
  // The tuning methods the engine gives this loop reach the Tune page's method list (`setupDiscover`'s `methods`, C4).
  if (controller) controller.methodsChanged = methods => tuning?.offerMethods(methods);
  const application = new ThreePageApplication(root, post, () => operation.busy, message => operation.notice("verify", message),
    failure => operation.failure("verify", failure),
    () => operation.stageBlock() !== undefined || experiments?.busy === true || recovery?.active === true);
  recovery = new ThreePageRecovery(root, post, () => operation.busy || application.active);
  const suppliedModel = new ThreePageSuppliedModel(root, post, () => operation.busy || application.active || recovery?.active === true,
    message => operation.notice("model", message));
  const experiments = loop?.experiment?.execution === "project-scripted" ? new ThreePageExperiments(root, loop, post, () => operation.stageBlock()) : undefined;
  let mimoRecordingId: string | undefined;
  const identification = new ThreePageIdentification(root, post, () => root.querySelector<HTMLButtonElement>("[data-identify-import]")!.click(), () => operation.stageBlock());
  // The analysis tools, their labels and defaults, and every result come from the engine (B9.9); the page draws them.
  const analysis = initializeThreePageAnalysis(root, post, () => synchronizeTools(), () => persistView());
  // The measured MIMO paths: a served loop's are the engine's `measuredPaths` (C7), which the router sends to their section by the tool.
  const mimoAnalysis = initializeThreePageMimoAnalysis(root, undefined, post, selected => { if (selected) analysisChoice = "measured-mimo"; synchronizeTools(); });
  bindEngineAnalysis(root, routeMeasuredPaths(analysis, mimoAnalysis));
  const configureMimoRecording = (recording: Record<string, unknown> | undefined): void => {
    // A served loop's measured paths take the study's last kept recording in the engine (C7): no TypeScript recording is configured.
    if (engineServesLoop || !loop?.channels || !recording) return;
    const evidence = recording.evidence && typeof recording.evidence === "object" ? recording.evidence
      : recording.acquisition && typeof recording.acquisition === "object" ? recording.acquisition : undefined;
    if (!evidence || typeof (evidence as Record<string, unknown>).recordingId !== "string") return;
    const recordingId = String((evidence as Record<string, unknown>).recordingId);
    mimoRecordingId = recordingId;
    mimoAnalysis.setContext({ inputIds: loop.channels.inputs.map(input => input.modelChannelId), recordings: [{ id: recordingId, label: `Retained recording · ${recordingId}` }],
      onAnalyze: (selectedRecordingId, trials) => post("control-studio.mimo.analyze", { recordingId: selectedRecordingId, trials }) });
  };
  const tuningMethod = root.querySelector<HTMLSelectElement>("[data-engineer-tuning-method]")!;
  // The Tuning method select holds tuning methods only; the analysis tools open from Analyze… on the Tune row. The engine's tools
  // bring their own labels (`synchronizeTools`); the measured MIMO path is the section's own, drawn from the engine's answer (C7).
  const analysisLabels: Record<string, string> = { "measured-mimo": "Measured MIMO paths" };
  /** The analysis tool shown in place of the tuning result, or undefined while tuning. */
  let analysisChoice: string | undefined;
  // A served loop offers the measured paths when the engine does (C7); a parked loop keeps its TypeScript condition (V7.17 debt).
  const measuredMimoAvailable = (): boolean => engineServesLoop ? mimoAnalysis.offered()
    : Boolean(mimoRecordingId && loop?.channels && loop.channels.inputs.length >= 2);
  // The engine's `measuredPaths` is listed once, as the measured-MIMO entry.
  const availableAnalysis = (): string[] => [...analysis.availableTools().filter(tool => tool !== "measuredPaths"), ...(measuredMimoAvailable() ? ["measured-mimo"] : [])];
  const chooseAnalysis = (tool: string): void => {
    analysisChoice = tool;
    analysis.setTool(tool === "measured-mimo" ? undefined : tool);
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
    for (const offer of analysis.offers()) analysisLabels[offer.tool] = offer.label;
    analysisLabels["measured-mimo"] = mimoAnalysis.label() ?? "Measured MIMO paths";
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
    // A served loop's Recorded-data fit shows the engine's answers; a parked loop keeps the TypeScript rules until V7.17.
    identification.setEngineServed(engineServesLoop);
    if (experiments?.busy || startingExperiment) {
      for (const control of [...runLockedControls(root), ...Array.from(root.querySelectorAll<HTMLButtonElement>("[data-engineer-tune],[data-engineer-verify]"))]) control.disabled = true;
    }
    if (startingExperiment) root.querySelector<HTMLButtonElement>("[data-engineer-find]")!.disabled = true;
    recovery?.synchronize();
    // The Apply button's gates changed with the experiment, the start or the recovery: the application writes it (threePageApplication.ts).
    application.synchronize();
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
  /**
   * The engine's answer to a recording it checks or fits (B3.6, `control-studio.identification-recording` with `checked` or
   * `recording-needed`): the file's channels fill the selectors, what the file still needs or the check it passed is the model message,
   * and a kept file can change the analyses that apply (B9.9). Its identification comes as an Identify result, not through the
   * manual identification's stages.
   */
  const showRecordingAnswer = (payload: Record<string, unknown>): void => {
    if (payload.status === "checked" || payload.status === "recording-needed") analysis.recordingsChanged();
    if (typeof payload.recordingId === "string") bindings?.receive(payload);
    const words = payload.status === "checked" ? payload.validation : payload.reason;
    operation.notice("model", typeof words === "string" ? words : "");
  };
  /** True while the page waits for the engine's answer to a recording it asked the engine to check or fit (B3.6). */
  let awaitingEngineRecording = false;
  /** Asks the engine to check or fit a recording: the page stays busy until its answer comes, as a run does (B3.6). */
  const askEngineRecording = (kind: string, payload: Record<string, unknown>): void => {
    awaitingEngineRecording = true;
    operation.importing = true;
    operation.synchronize();
    post(kind, payload);
  };
  /** A changed selection of a recording the study keeps asks the engine to fit it again with that selection (B3.6, §0.1a); the manual identification and a parked loop refit on the TypeScript route. */
  const refitRecording = (selection: RecordingSignalSelection): void => {
    if (engineServesLoop && !identification.isSelected) askEngineRecording("control-studio.recording.refit", { recordingSelection: selection });
    else identifyRecording();
  };
  const bindings = loop && controller ? new ThreePageBindings(root, loop, controller, post, (page, message) => operation.notice(page, message), (valid, reason) => { bindingsBlock = valid ? undefined : reason; operation.synchronize(); application.synchronize(); recovery?.synchronize(); }, refitRecording) : undefined;
  bindRecordingOpen(root, () => operation.busy || application.active || recovery?.active === true || bindings?.canRun === false,
    json => {
      // Recorded-data fit is the manual identification: its file goes to the TypeScript route, on a served loop and a parked one (B3.3).
      if (identification.isSelected) { awaitingEngineRecording = false; identification.openRecording(json); }
      // Any other method on a loop the engine serves: the engine checks the file and keeps it, then fits it (B3.6, §0.1a).
      else if (engineServesLoop) askEngineRecording("control-studio.recording.import", { json });
      // A parked loop keeps the TypeScript route.
      else { recordingJson = json; identifyRecording(); }
    },
    message => operation.notice("model", message));
  const start = (from: StudioStart) => {
    if (recovery?.active) return;
    if (application.active) { application.abort(); return; }
    if (from === "tune" && !operation.busy && analysisChoice === "measured-mimo") { mimoAnalysis.run(); synchronizeTools(); return; }
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
  /** The acquisition settings the study holds (B3.2), as the page shows them: the Limits, the method and the setpoint signal. */
  const restoreAcquisition = (saved: AcquisitionSettings): void => {
    if (!loop) return;
    limits = { ...initialExperimentLimits(loop), durationSeconds: saved.durationSeconds,
      ...(saved.inputRanges ? { inputRanges: saved.inputRanges } : {}), ...(saved.outputRanges ? { outputRanges: saved.outputRanges } : {}),
      ...(saved.inputRange ? { inputMinimum: saved.inputRange.minimum, inputMaximum: saved.inputRange.maximum } : {}),
      ...(saved.outputRange ? { outputMinimum: saved.outputRange.minimum, outputMaximum: saved.outputRange.maximum } : {}) };
    root.querySelector<HTMLSelectElement>("[data-engineer-acquisition-method]")!.value = saved.method === "configured" ? "automatic" : saved.method;
    // A restored method shows its own section as a chosen one does (the select's value alone fires no change).
    showAcquisitionMethod();
    reference?.restore(saved.setpointSignalId);
  };
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
    // The engine opened a served loop: it has no method for the manual parameter tool (B6.1), so the parameter controls hide at once.
    if (stage === "engineer" && payload.status === "opened") {
      engineServesLoop = true;
      // The served loop's measured paths window each declared input (C7); the engine answers the rest.
      if (loop?.channels) mimoAnalysis.setContext({ inputIds: loop.channels.inputs.map(input => input.modelChannelId) });
      experiments?.receive("parameters", { status: "unavailable" });
    }
    analysis.receive(stage, payload);
    if (stage !== "engineer" && stage !== "flow" && stage !== "bindings") {
      suppliedModel.receive(stage, payload);
      // The engine's Identify answers (a refit's operation, a recording's failure) belong to the engine path, not the manual identification (B3.3).
      if (stage === "identify" && identification.isSelected) identification.receive(stage, payload);
      if (stage === "parameters") experiments?.receive("parameters", payload);
      // A stored TypeScript result is a parked loop's only: a served loop's analysis is the engine's (C7), so an old snapshot is not shown.
      if (stage === "mimo" && !engineServesLoop) {
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
    if (loop && payload.acquisitionSettings && typeof payload.acquisitionSettings === "object") restoreAcquisition(payload.acquisitionSettings as AcquisitionSettings);
    // A running operation (a refit that starts one) ends the busy state the engine's recording answer began (B3.6).
    if (stage === "flow" && payload.status === "running") operation.importing = false;
    operation.receive(stage, payload);
    application.receive(payload);
    bindings?.synchronize();
    recovery?.synchronize();
    suppliedModel.receive(stage, payload);
    synchronizeTools();
  }, select, message => {
    const rawPayload = message.payload as { recording?: Record<string, unknown>; result?: Record<string, unknown> } | undefined;
    // The view the engine restores for a parked loop (panel.ts): the engine serves no loop until its `opened` answer says so.
    // A parked loop's measured paths take the TypeScript route: the served loop's engine context is dropped with the served state.
    if (message.kind === "control-studio.navigation.restore") { engineServesLoop = false; mimoAnalysis.setContext(undefined); }
    if (message.kind === "control-studio.identification-recording" && isRecord(message.payload)) {
      // The engine answers a file or a selection the page asked it about with `checked`, `recording-needed` or `failed`; the TypeScript
      // route answers the manual identification's file with `imported` (B3.3). Only the engine's answers go to the engine path.
      const payload = message.payload;
      const fromEngine = payload.status === "checked" || payload.status === "recording-needed" || (payload.status === "failed" && awaitingEngineRecording);
      awaitingEngineRecording = false;
      operation.importing = false;
      operation.synchronize();
      if (fromEngine) showRecordingAnswer(payload);
      else {
        identification.receive("recording", payload);
        if (payload.status === "imported" && isRecord(payload.recording)) {
          window.dispatchEvent(new CustomEvent("trust-control-studio-recording-opened", { detail: { recording: payload.recording } }));
        }
        if (typeof rawPayload!.recording === "object") configureMimoRecording(rawPayload!.recording as Record<string, unknown>);
      }
    }
    if (message.kind === "control-studio.experiment.result") {
      experiments?.receive(message.phase as Parameters<ThreePageExperiments["receive"]>[0], message.payload);
      if (message.phase === "terminal" && isRecord(message.payload) && message.payload.status === "completed"
        && isRecord(message.payload.recording) && Array.isArray(message.payload.recording.rawSamples)
        && message.payload.recording.rawSamples.length > 1 && !experiments?.busy) {
        identification.receive("recording", { status: "imported" });
      }
      if (!experiments?.busy) operation.synchronize();
    }
    if (message.kind === "control-studio.recording.evidence") experiments?.receive("recording", message.payload);
    if (message.kind === "control-studio.recording.evidence" && message.payload && typeof rawPayload!.recording === "object") {
      configureMimoRecording(rawPayload!.recording as Record<string, unknown>);
    }
    if (message.kind === "control-studio.experiment.result" && message.payload && typeof rawPayload!.recording === "object") configureMimoRecording(rawPayload!.recording as Record<string, unknown>);
    if (message.kind === "control-studio.workflow-result" && message.payload && typeof rawPayload!.result === "object") {
      const result = rawPayload!.result as Record<string, unknown>;
      if (["restored", "opened"].includes(String(message.phase)) && isRecord(result.identificationStudy)) {
        identification.receive("study", result.identificationStudy);
      }
      if (isRecord(result.recording) && isRecord(result.experiment)) experiments?.receive("recording", { recording: result.recording, experiment: result.experiment });
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
  /** The study's recorded inputs, which an opened loop answers (A8.6): they fill the editors as the engine recorded them (§0.1a `inputs`). */
  const restoreRecordedInputs = (recorded: Record<string, unknown>): void => {
    if (!loop) return;
    if (isRecord(recorded.requirements)) task = recorded.requirements as unknown as Partial<EngineerRequirements>;
    if (Array.isArray(recorded.scenarios)) scenarioDefinitions = recorded.scenarios as typeof scenarioDefinitions;
    if (isRecord(recorded.acquisition)) restoreAcquisition(recorded.acquisition as unknown as AcquisitionSettings);
    // The method the page showed last, which the study keeps as its view, wins over the recorded live method.
    if (openedMethod && Array.from(acquisitionMethod.options).some(option => option.value === openedMethod)) {
      acquisitionMethod.value = openedMethod;
      showAcquisitionMethod();
    }
    openedMethod = undefined;
    // The recording selection the study keeps shows the selectors again (B3.6): the answer on open names it, and nothing else does.
    if (isRecord(recorded.recordingSelection)) bindings?.restoreRecording(recorded.recordingSelection as unknown as RecordingSignalSelection);
    // A state law's family, draft and pins are the study's state design (C4); a structure's pins are its settings pins.
    const stateDesign = isRecord(recorded.stateDesign) ? recorded.stateDesign : undefined;
    const method = typeof recorded.method === "string" ? recorded.method : "automatic";
    const stateFamily = stateDesign && typeof stateDesign.family === "string" ? stateDesign.family : undefined;
    controller?.restoreChoice(stateFamily ?? (typeof recorded.structure === "string" ? recorded.structure : undefined));
    const statePins = stateDesign && isRecord(stateDesign.pins) ? stateDesign.pins : undefined;
    const structurePins = isRecord(recorded.pins) && isRecord(recorded.pins.values) ? recorded.pins.values : undefined;
    const pins = method === "state-space" ? statePins : structurePins;
    tuning?.restoreInputs({
      method: method as ControllerTuningPreferences["method"],
      ...(isRecord(recorded.settings) ? { settings: recorded.settings as Record<string, number> } : {}),
      ...(pins ? { pins: pins as Record<string, number> } : {}),
      ...(stateDesign && isRecord(stateDesign.draft) ? { stateDraft: stateDesign.draft as Record<string, string> } : {}),
    });
    operation.synchronize();
    synchronizeTools();
  };
  /** The method the opened loop's view names; the recorded inputs that follow the open answer apply it (A8.6). */
  let openedMethod: string | undefined;
  window.addEventListener(ENGINE_VIEW_EVENT, event => {
    const method = (event as CustomEvent<Record<string, unknown>>).detail.identifyMethod;
    openedMethod = typeof method === "string" ? method : undefined;
  });
  window.addEventListener(ENGINE_INPUTS_EVENT, event => restoreRecordedInputs((event as CustomEvent<Record<string, unknown>>).detail));
  // The requirements an engine answer holds (B4.3): the record becomes the page's requirements, which the editor opens with, and nothing else.
  window.addEventListener(ENGINE_REQUIREMENTS_EVENT, event => {
    const requirements = (event as CustomEvent<unknown>).detail;
    if (isRecord(requirements)) task = requirements as unknown as Partial<EngineerRequirements>;
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
import { runLockedControls } from "./runLock";
import { ENGINE_INPUTS_EVENT, ENGINE_VIEW_EVENT } from "./engineInputs";
import { ENGINE_REQUIREMENTS_EVENT } from "./engineRequirements";
import type { ControllerTuningPreferences } from "../controllerTuningPreferences";

import { initializeMultiSelect } from "../../webview/multiSelect";

import { ThreePageRecovery } from "./threePageRecovery";

import { editControllerChannels } from "./threePageChannels";

import { identificationMethodApplicability } from "../identificationMethodApplicability";

import { ThreePageReference } from "./threePageReference";
import { ThreePageSuppliedModel } from "./threePageSuppliedModel";
import { ThreePageExperiments } from "./threePageExperiments";
import { ThreePageIdentification } from "./threePageIdentification";
import { initializeThreePageAnalysis } from "./threePageAnalysis";
import { bindEngineAnalysis } from "./engineAnalysis";
import { decodeMimoAnalysisResult, initializeThreePageMimoAnalysis, routeMeasuredPaths } from "./threePageMimoAnalysis";
import { renderModelPresentation } from "./threePageModel";
import { studioFailureOf } from "./studioFailure";
import { ThreePageAnalysisChooser } from "./threePageAnalysisChooser";
import { decodeGeneralModel } from "../generalModel";
import type { IdentifiedModelPresentation } from "../identification";
import type { RecordingSignalSelection } from "../openedRecordingChannels";
