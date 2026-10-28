declare function acquireVsCodeApi(): { postMessage(message: unknown): void };

type Page = StudioPage;
const root = document.querySelector<HTMLElement>("[data-studio-three-page]");
if (root) {
  const api = acquireVsCodeApi();
  /** The Method list's view of the method the engine holds (threePageMethod.ts); it is built once the list exists. */
  let methodList: ThreePageMethod | undefined;
  const post = (kind: string, payload: Record<string, unknown> = {}) => {
    // The engine answers each change of the study inputs, in the order the page sends them: the Method list waits for those answers.
    if (kind === "control-studio.engineer.invalidate") methodList?.posted(methodAskedBy(payload));
    api.postMessage({
      schemaVersion: 1, kind, messageId: crypto.randomUUID(),
      projectId: root.dataset.projectId, editorSessionId: root.dataset.editorSessionId,
      loopId: root.dataset.loopId, expectedLoopRevision: root.dataset.loopRevision, payload,
    });
  };
  // A chart's export request reaches the host through this page's channel: the chart bundle has no VS Code API of its own (B10.5).
  initializeChartExportBridge(post, root.ownerDocument);
  // A plant handoff opens the page without a loop: the engine's summary of the plant is shown under the target (B1.1).
  bindPlantSummary(root);
  const persistView = () => {
    const stage = root.dataset.selectedStage ?? "model";
    post("control-studio.navigation.select", {
      stage, view: {
        acquisitionMethod: root.querySelector<HTMLSelectElement>("[data-engineer-acquisition-method]")?.value ?? "automatic",
        experimentSignalIds: Array.from(root.querySelector<HTMLSelectElement>("[data-three-page-experiment-signals]")?.selectedOptions ?? [], option => option.value),
      },
      // The analysis the Tune page shows is kept with that page, so a reopened page shows it again (B9.9); null says none is shown.
      ...(stage === "tune" ? { analysis: mimoAnalysis.shown() ?? analysis.shown() ?? null } : {}),
    });
  };
  root.querySelector("[data-three-page-experiment-signals]")?.addEventListener("change", persistView);
  const tabs = Array.from(root.querySelectorAll<HTMLButtonElement>("[data-engineer-stage-tab]"));
  // The plant inputs and outputs get their dropdowns now. The project test's signals get theirs when the engine fills that section
  // (`openProjectTest`): a loop that has no project test shows no third dropdown, hidden or not.
  for (const select of Array.from(root.querySelectorAll<HTMLSelectElement>("select[multiple]:not([data-three-page-experiment-signals])"))) initializeMultiSelect(select);
  const loop = JSON.parse(root.dataset.studioLoop ?? "null") as LoopDefinition | null;
  const controller = loop ? new ThreePageController(root, loop, post) : undefined;
  // The Channels editor shows the rows the engine answered for the open loop (A8.1); the engine's answers keep them current.
  root.querySelector<HTMLButtonElement>("[data-engineer-channels]")?.addEventListener("click", () => {
    const choices = JSON.parse(root.dataset.studioChannelChoices || "null") as ChannelChoices | null;
    // The Tune area keeps the refusal of a selection until the next one is sent (the Tune page annex); the engine refuses that one again if it must.
    if (choices) editControllerChannels(choices, selection => { operation.notice("tune", ""); post("control-studio.engineer.invalidate", { channelMapping: selection }); });
  });
  const reference = loop ? new ThreePageReference(root, loop) : undefined;
  const acquisitionMethod = root.querySelector<HTMLSelectElement>("[data-engineer-acquisition-method]")!;
  /** The engine's offers of the Method list (B3.2, B3.3), by method: a method the loop cannot run carries the sentence that says why. */
  const offers = new Map<string, IdentifyMethodOffer>();
  const methodIssue = (): string | undefined => offers.get(acquisitionMethod.value)?.reason ?? undefined;
  /** The sections of the methods that have their own page area: the recorded-data fit's and the project test's (B3.3). */
  const showAcquisitionMethod = (): void => {
    experiments?.setSelectedMethod(acquisitionMethod.value === "project");
    identification.setSelectedMethod(acquisitionMethod.value === "recorded-fit");
  };
  // After the engine refuses a method, the list shows the method the study holds again; the refusal's sentence stays on the page.
  methodList = new ThreePageMethod(acquisitionMethod, () => { showAcquisitionMethod(); operation.synchronize(); }, persistView);
  acquisitionMethod.addEventListener("change", () => {
    persistView();
    operation.synchronize();
    showAcquisitionMethod();
    // The stage renders consult the shared gate, so a Method change cannot re-enable a blocked action; its reason stays visible.
    const block = operation.stageBlock();
    const method = acquisitionMethod.value as AcquisitionSettings["method"];
    operation.notice("model", methodIssue() ?? block ?? "");
    // A method the engine records (its offer's `records`) is a study input (A8.1): the page sends it with the limits as they stand, and the
    // engine records it or refuses it with the rule it breaks; a missing duration is the Limits editor's refusal (B3.2). The opened file,
    // the supplied model and the project test start no campaign: the page keeps them as its view (`persistView`), so a reopened loop
    // shows them again (`pageView`).
    if (offers.get(method)?.records && loop && limits) post("control-studio.engineer.invalidate", { acquisitionSettings: acquisitionOf(method, limits) });
  });
  /** The Limits editor's values: the engine's limits of the open answer, which hold the study's own record (B3.2, A8.6), then what the engineer committed. */
  let limits: StudioExperimentLimits | undefined;
  /** The acquisition settings the page records for a method and the limits it holds (B3.2): they name the setpoint chosen for a setpoint step. */
  const acquisitionOf = (method: AcquisitionSettings["method"], held: StudioExperimentLimits): AcquisitionSettings =>
    ({ ...acquisitionPreferences(method, held), ...(reference?.signalId ? { setpointSignalId: reference.signalId } : {}) });
  /** The requirements the page holds: what the engine last answered, or the change the dialog posted since. The dialog's rows start from the engine's offers (B4.3). */
  let task: HeldRequirements = {};
  /** The requirements an engine answer holds become the page's: the record the Requirements editor opens with (B4.3). */
  const acceptRequirements = (requirements: HeldRequirements): void => {
    task = requirements;
  };
  // The declared scenarios are the loop file's list of JSON objects, which the engine checked.
  let scenarioDefinitions: Record<string, unknown>[] = loop?.verificationScenarios ?? [];
  // The plant-mapping gate and an in-progress simulator start both block the stage actions through the shared gate.
  let bindingsBlock: string | undefined;
  let startingExperiment = false;
  /** Starts the local simulator, the next action the engine names when the project test finds it stopped (A2.8); its answer continues the test. */
  const startSimulator = (): void => {
    if (startingExperiment || !loop) return;
    startingExperiment = true;
    operation.notice("model", "Starting simulator…");
    root.querySelector<HTMLButtonElement>("[data-engineer-find]")!.disabled = true;
    post("control-studio.runtime.start", { runtimeId: loop.runtimeId, runtimeMode: "simulate" });
  };
  /** True once the engine serves the opened loop: it answered `engineer` `opened`. A loop the engine refuses never sets it. */
  let engineServesLoop = false;
  /** The Limits editor a paused run opened (B8, `needs: limits`): the run starts again when Done commits. It holds the engine's sentence and field for what is missing. */
  let waitingForLimits: { issue?: { field: string; message: string } } | undefined;
  /** The sentence and the field with which the last pause asked for the limits (`needsMessage`, `needsField`); the engine's words, not the page's. */
  let limitsAsked: { field: string; message: string } | undefined;
  /** The noise band the engine says a declared measurement filter needs (B4.3); the requirements editor shows its field then. */
  let measurementNoiseBandRequired = false;
  // Done saves the requirements; only the editor that a paused Tune opens resumes that start. Cancel and manual edits never do.
  // A refusal the engine gives a requirement value reopens the editor with the refusal under its field (showRefusedRequirement).
  const editRequirements = (resumeTune?: () => void, refusal?: RequirementEngineNotes["refusal"]): void => {
    if (!loop) return;
    editControlRequirements(loop, task, (next: RequirementsChange) => {
      // The dialog re-shows the rows it posted when the engine refuses them, so the page holds them until the engine answers.
      const { scenarioDefinitions: scenarios, ...rows } = next;
      task = rows;
      if (scenarios) scenarioDefinitions = scenarios;
      post("control-studio.engineer.invalidate", next as unknown as Record<string, unknown>);
    }, { scenarioDefinitions }, resumeTune, { measurementNoiseBandRequired, refusal });
  };
  /** The engine refuses a requirement value with its field and names the requirements as the fix (B4.3, `open-requirements`); the editor reopens with that sentence under the field. */
  const showRefusedRequirement = (failure: unknown): void => {
    if (!isRecord(failure) || typeof failure.field !== "string" || !isRecord(failure.nextAction) || failure.nextAction.kind !== "open-requirements") return;
    // An editor that is open keeps its unsaved values; the page's message shows the refusal meanwhile.
    if (document.querySelector("[data-trust-compact-editor]")) return;
    editRequirements(undefined, { field: failure.field, message: typeof failure.message === "string" ? failure.message : "" });
  };
  root.querySelector<HTMLButtonElement>("[data-studio-requirements]")!.addEventListener("click", () => editRequirements());
  /** Opens Limits; the engine's refusal of a limit value reopens it with that sentence under its field (showRefusedLimit). */
  const editLimits = (refusal?: { field: string; message: string }): void => {
    if (!loop || !limits) return;
    editExperimentLimits(limits, next => {
      limits = next;
      const method = root.querySelector<HTMLSelectElement>("[data-engineer-acquisition-method]")!.value as AcquisitionSettings["method"];
      post("control-studio.engineer.invalidate", { acquisitionSettings: acquisitionOf(method, next) });
      if (waitingForLimits) { waitingForLimits = undefined; start("find"); }
    }, () => { waitingForLimits = undefined; }, refusal ?? waitingForLimits?.issue);
  };
  /** The engine refuses a limit value with its field and names Limits as the fix (B3.2); the editor reopens with that sentence under the field. */
  const showRefusedLimit = (failure: unknown): void => {
    if (!isRecord(failure) || typeof failure.field !== "string" || !isRecord(failure.nextAction) || failure.nextAction.kind !== "open-limits") return;
    // An editor that is open keeps its unsaved values; the page's message shows the refusal meanwhile.
    if (document.querySelector("[data-trust-compact-editor]")) return;
    editLimits({ field: failure.field, message: typeof failure.message === "string" ? failure.message : "" });
  };
  root.querySelector<HTMLButtonElement>("[data-studio-limits]")!.addEventListener("click", () => editLimits());
  const select = (page: Page, persist: boolean) => {
    root.dataset.selectedStage = page;
    for (const tab of tabs) {
      const selected = tab.dataset.engineerStageTab === page;
      tab.setAttribute("aria-selected", String(selected));
      tab.tabIndex = selected ? 0 : -1;
    }
    for (const panel of Array.from(root.querySelectorAll<HTMLElement>("[data-studio-page]"))) panel.hidden = panel.dataset.studioPage !== page;
    root.querySelector<HTMLElement>("[data-studio-application]")!.hidden = page !== "verify";
    if (persist) persistView();
    window.dispatchEvent(new CustomEvent("trust-control-studio-three-page", { detail: { page } }));
  };
  /** Performs the typed next action of a failure on its own page (Specification 35 A11.2); it never starts work. */
  const runNextAction = (kind: string, page: Page): void => {
    switch (kind) {
      case "connect-runtime":
      case "open-loop-setup": post("control-studio.host-command", { command: kind }); return;
      // The project test's review found the local simulator stopped (A2.8): the page asks the extension to start it, and the start's
      // answer continues the test.
      case "start-simulator": startSimulator(); return;
      case "open-limits": root.querySelector<HTMLButtonElement>("[data-studio-limits]")!.click(); return;
      case "open-requirements": root.querySelector<HTMLButtonElement>("[data-studio-requirements]")!.click(); return;
      case "open-channels": root.querySelector<HTMLButtonElement>("[data-engineer-channels]")?.click(); return;
      // The physical target's approval is given in the Apply review on the Validate page (specification 35, B9.3); the page only shows it.
      case "approve": select("verify", true); root.querySelector<HTMLElement>("[data-engineer-apply]")?.focus(); return;
      case "choose-method": root.querySelector<HTMLSelectElement>(page === "model" ? "[data-engineer-acquisition-method]" : "[data-engineer-tuning-method]")?.focus(); return;
      case "identify": select("model", true); root.querySelector<HTMLButtonElement>("[data-engineer-find]")?.focus(); return;
      case "tune": select("tune", true); root.querySelector<HTMLButtonElement>("[data-engineer-tune]")?.focus(); return;
      case "validate": select("verify", true); root.querySelector<HTMLButtonElement>("[data-engineer-verify]")?.focus(); return;
      // Restore previous settings asks the engine for the Restore review, which the Apply area on the Validate page shows and confirms (B6.4).
      case "restore": select("verify", true); application.restore(); return;
      // Resume asks the engine for the reattach of the interrupted operation it names (A8.6, B3.9): the engine follows, completes or ends
      // that operation, and starts no new one.
      case "resume": select("model", true); post("control-studio.identify.resume"); return;
      default: return;
    }
  };
  // The operation and the identification refer to each other in their arguments, so each carries its type for the compiler to read.
  const operation: ThreePageOperation = new ThreePageOperation(root, post, select, field => {
    if (field === "requirements") editRequirements(() => start("tune"));
    if (field === "limits") { waitingForLimits = { issue: limitsAsked }; root.querySelector<HTMLButtonElement>("[data-studio-limits]")!.click(); }
  }, () => bindingsBlock ?? (startingExperiment ? "Starting simulator…" : undefined), runNextAction, () => application.synchronize(),
  // The Identify button names the method the page shows: the project test's words, the recorded-data fit's, or Identify plant.
  () => experiments?.isSelected ? experiments.primaryActionLabel : identification.isSelected ? identification.primaryActionLabel : "Identify plant");
  const resetButton = root.querySelector<HTMLButtonElement>("[data-studio-reset-chart]");
  if (resetButton) operation.bindReset(resetButton);
  const tuning = loop ? new ThreePageTuning(root, loop, post, () => controller?.selectedFamily ?? "automatic", () => controller?.chosenFamily,
    family => controller?.symbolsOf(family)) : undefined;
  // The tuning methods the engine gives this loop reach the Tune page's method list (`setupDiscover`'s `methods`, C4).
  if (controller) controller.methodsChanged = (methods, titles) => tuning?.offerMethods(methods, titles);
  const application = new ThreePageApplication(root, post, () => operation.busy, message => operation.notice("verify", message),
    failure => operation.failure("verify", failure),
    () => operation.stageBlock() !== undefined || experiments?.busy === true);
  const suppliedModel = new ThreePageSuppliedModel(root, post, () => operation.busy || application.active,
    message => operation.notice("model", message));
  /** The project test exists once the engine's open answer names its form (B6.1, C12); a loop the engine does not name one for has none. */
  let experiments: ThreePageExperiments | undefined;
  const identification: ThreePageIdentification = new ThreePageIdentification(root, post, () => operation.stageBlock());
  // The analysis tools, their labels and defaults, and every result come from the engine (B9.9); the page draws them.
  const analysis = initializeThreePageAnalysis(root, post, () => synchronizeTools(), () => persistView());
  // The measured MIMO paths: a served loop's are the engine's `measuredPaths` (C7), which the router sends to their section by the tool.
  const mimoAnalysis = initializeThreePageMimoAnalysis(root, undefined, post, selected => { if (selected) analysisChoice = "measured-mimo"; synchronizeTools(); });
  bindEngineAnalysis(root, routeMeasuredPaths(analysis, mimoAnalysis));
  const tuningMethod = root.querySelector<HTMLSelectElement>("[data-engineer-tuning-method]")!;
  // The Tuning method select holds tuning methods only; the analysis tools open from Analyze… on the Tune row. The engine's tools
  // bring their own labels (`synchronizeTools`); the measured MIMO path is the section's own, drawn from the engine's answer (C7).
  const analysisLabels: Record<string, string> = { "measured-mimo": "Measured MIMO paths" };
  /** The analysis tool shown in place of the tuning result, or undefined while tuning. */
  let analysisChoice: string | undefined;
  // A served loop offers the measured paths when the engine does (C7); a loop the engine does not serve offers none.
  const measuredMimoAvailable = (): boolean => engineServesLoop && mimoAnalysis.offered();
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
    // A tool the analysis restored when the panel reopened (A8.6, B9.9) is the shown one, as if the engineer had chosen it from Analyze….
    if (analysisChoice === undefined) analysisChoice = analysis.shown()?.tool;
    for (const offer of analysis.offers()) analysisLabels[offer.tool] = offer.label;
    analysisLabels["measured-mimo"] = mimoAnalysis.label() ?? "Measured MIMO paths";
    const tools = availableAnalysis();
    if (analysisChoice !== undefined && !tools.includes(analysisChoice)) analysisChoice = undefined;
    const selected = analysisChoice !== undefined;
    const measuredMimoSelected = analysisChoice === "measured-mimo";
    analysisChooser.synchronize(selected);
    root.querySelector<HTMLElement>('[data-studio-message="tune"]')!.hidden = selected;
    mimoAnalysis.setSelected(measuredMimoSelected);
    root.dataset.analysisSelected = String(selected);
    experiments?.setSelectedMethod(acquisitionMethod.value === "project");
    identification.setSelectedMethod(acquisitionMethod.value === "recorded-fit");
    // The Tune and Validate gates read the engine's stage statuses, and an engine answer changes those without a render (applyStageStatuses):
    // the operation redraws its gates after every answer, so a stage the engine has just made ready is enabled (J14). That redraw is the
    // base of the page's controls, so it comes first: each lock below it is written over it, and the last word is the application's.
    operation.synchronize();
    // While Supplied plant model is the method, the Identify button names the model's file, which the operation's redraw has just replaced.
    suppliedModel.synchronize();
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
    if (experiments?.busy || startingExperiment) {
      for (const control of [...runLockedControls(root), ...Array.from(root.querySelectorAll<HTMLButtonElement>("[data-engineer-tune],[data-engineer-verify]"))]) control.disabled = true;
    }
    if (startingExperiment) root.querySelector<HTMLButtonElement>("[data-engineer-find]")!.disabled = true;
    // The Apply button's gates changed with the experiment or the start: the application writes it (threePageApplication.ts).
    application.synchronize();
  };
  tuningMethod.addEventListener("change", () => { operation.synchronize(); synchronizeTools(); });
  synchronizeTools();
  window.addEventListener("trust-control-studio-identification-open-recording", () => root.querySelector<HTMLButtonElement>("[data-identify-import]")!.click());
  window.addEventListener("trust-control-studio-identification-presentation", event => {
    const { presentation } = (event as CustomEvent<{ presentation?: IdentifiedModelPresentation }>).detail;
    if (!presentation) return;
    // The model's type, equation and key numbers are the engine's words as they arrived: the page draws them and composes none (B3.8).
    renderModelPresentation(root, presentation);
    window.dispatchEvent(new CustomEvent("trust-control-studio-engineer", { detail: { identificationPresentation: presentation } }));
  });
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
  /** Asks the engine to check or fit a recording: the page stays busy until its answer comes, as a run does (B3.6). */
  const askEngineRecording = (kind: string, payload: Record<string, unknown>): void => {
    operation.importing = true;
    operation.synchronize();
    post(kind, payload);
  };
  /**
   * A changed selection of a recording the study keeps asks the engine to fit it again with that selection (B3.6, §0.1a). The Recorded-data
   * fit reads its file with the loop's own signals and takes no selection (B3.3, B3.6), so while it is the method a changed selection asks
   * nothing: a refit would replace the manual identification's work.
   */
  const refitRecording = (selection: RecordingSignalSelection): void => {
    if (!identification.isSelected) askEngineRecording("control-studio.recording.refit", { recordingSelection: selection });
  };
  const bindings = loop && controller ? new ThreePageBindings(root, loop, post, (page, message) => operation.notice(page, message), (valid, reason) => { bindingsBlock = valid ? undefined : reason; operation.synchronize(); application.synchronize(); }, refitRecording) : undefined;
  bindRecordingOpen(root, () => operation.busy || application.active || bindings?.canRun === false,
    json => {
      // Recorded-data fit is the manual identification: the engine keeps its file for the steps it names (B3.3).
      if (identification.isSelected) identification.openRecording(json);
      // Any other method: the engine checks the file and keeps it, then fits it (B3.6, §0.1a); a loop it refuses answers with its refusal.
      else askEngineRecording("control-studio.recording.import", { json });
    },
    message => operation.notice("model", message));
  const start = (from: StudioStart) => {
    if (application.active) { application.abort(); return; }
    if (from === "tune" && !operation.busy && analysisChoice === "measured-mimo") { mimoAnalysis.run(); synchronizeTools(); return; }
    if (from === "tune" && !operation.busy && analysisChoice !== undefined) { analysis.run(); synchronizeTools(); return; }
    if (from === "find" && !operation.busy && suppliedModel.selected) { suppliedModel.open(); return; }
    if (from === "find" && !operation.busy && identification.isSelected) { identification.primaryAction(); return; }
    // The project test's review goes to the engine whatever the runtime's state: a runtime that is not running is answered with the
    // next action that applies (A2.8).
    if (experiments?.isSelected && (experiments.busy || from === "find")) {
      if (!startingExperiment) experiments.primaryAction();
      return;
    }
    if (from === "find" && !operation.busy) bindings?.useLiveSignals();
    if (bindings?.canRun === false) { bindings.explain(); return; }
    // Nothing is sent before the engine's open answer has been shown (it gives the limits, the Method list and the stage statuses).
    if (!loop || !limits) return;
    // The request names the stage; the engine reads the study's inputs (the limits, the method, the requirements) recorded when the
    // engineer set them, and asks for a missing limit itself (`needs`, B8). Tune also carries the method, the structure and the pins the
    // engineer chose (B4.2).
    try {
      operation.start(from, from === "tune" ? {
        family: controller?.chosenFamily,
        tuningPreferences: tuning?.preferences(!operation.busy), pinnedValues: tuning?.pinnedValues(),
      } : {});
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
  /**
   * The acquisition settings the study holds (B3.2), as the page shows them: the method the engine says the study holds (`heldInputs`, the
   * entry of the Method list) and the setpoint signal. The Limits are the open answer's own, which already hold the study's record.
   */
  const restoreAcquisition = (saved: AcquisitionSettings): void => {
    if (!loop) return;
    if (openedHeld?.identifyMethod) root.querySelector<HTMLSelectElement>("[data-engineer-acquisition-method]")!.value = openedHeld.identifyMethod;
    // A restored method shows its own section as a chosen one does (the select's value alone fires no change).
    showAcquisitionMethod();
    reference?.restore(saved.setpointSignalId ?? undefined);
  };
  bindThreePageHostState(root, (stage, payload) => {
    // The engine's open answer says whether the noise band is required (B4.3), when a stage forwards it to the page.
    if (typeof payload.measurementNoiseBandRequired === "boolean") measurementNoiseBandRequired = payload.measurementNoiseBandRequired;
    // A manual-tuning preview is a prediction: it draws the plot and never counts as a finished Tune, so Validate stays locked.
    // Its refusal names the reason in the Tune area. The analysis presenter never sees these messages.
    if (stage === "tune" && payload.status === "preview") {
      operation.notice("tune", typeof payload.reason === "string" ? payload.reason : "");
      synchronizeTools();
      return;
    }
    if (stage === "tune" && payload.status === "failed") {
      // A refused preview shows the engine's failure, its sentence and next action (B4.5); the page writes none of its own.
      const failure = studioFailureOf(payload);
      if (failure) operation.failure("tune", failure); else operation.notice("tune", "");
      synchronizeTools();
      return;
    }
    // The engine opened a served loop (its Apply availability is the last part of its open answer).
    if (stage === "engineer" && payload.status === "opened") {
      engineServesLoop = true;
      // The served loop's measured paths window each declared input (C7); the engine answers the rest.
      if (loop?.channels) mimoAnalysis.setContext({ inputIds: loop.channels.inputs.map(input => input.modelChannelId) });
    }
    // A run that waits for the limits asks for them in the engine's sentence, under the field the engine names (B8): the Limits editor shows it.
    if (stage === "flow" && payload.status === "paused" && payload.missing === "limits") {
      limitsAsked = typeof payload.missingField === "string" && typeof payload.reason === "string"
        ? { field: payload.missingField, message: payload.reason } : undefined;
    }
    analysis.receive(payload);
    if (stage !== "engineer" && stage !== "flow" && stage !== "bindings") {
      // The engine's Identify answers (a refit's operation, a recording's failure) belong to the engine path, not the manual identification (B3.3).
      // The Recorded-data fit's own answers name a primary action; the page holds them while another method is shown (threePageIdentification.ts).
      if (stage === "identify" && (identification.isSelected || typeof payload.primaryAction === "string")) identification.receive(stage, payload);
      synchronizeTools();
      return;
    }
    if (stage === "bindings") { bindings?.receive(payload); return; }
    controller?.receive(payload);
    tuning?.receive(payload);
    if (payload.task && typeof payload.task === "object") acceptRequirements(payload.task as TuningRequirements);
    // A running operation (a refit that starts one) ends the busy state the engine's recording answer began (B3.6).
    if (stage === "flow" && payload.status === "running") operation.importing = false;
    // A refused requirement reopens the requirements editor, and a refused limit reopens Limits, under the field the engine names (B4.3, B3.2);
    // the page message still shows it.
    if (stage === "flow" && (payload.status === "failed" || payload.status === "refused")) { showRefusedRequirement(payload.failure); showRefusedLimit(payload.failure); }
    operation.receive(stage, payload);
    application.receive(payload);
    bindings?.synchronize();
    synchronizeTools();
  }, select, message => {
    if (message.kind === "control-studio.identification-recording" && isRecord(message.payload)) {
      // The engine answers a file or a selection the page asked it about with `checked`, `recording-needed` or `failed` (B3.6); the
      // Recorded-data fit's file is answered as the manual identification's own answer (B3.3).
      operation.importing = false;
      operation.synchronize();
      showRecordingAnswer(message.payload);
    }
    if (message.kind === "control-studio.experiment.result") {
      experiments?.receive(message.phase as Parameters<ThreePageExperiments["receive"]>[0], message.payload);
      if (!experiments?.busy) operation.synchronize();
    }
    if (message.kind === "control-studio.recording.evidence") {
      experiments?.receive("recording", message.payload);
      // The study kept a recording: the analyses that apply may now differ (the measured ones need one), so the engine is asked again (B9.9).
      analysis.recordingsChanged();
    }
    if (message.kind === "control-studio.runtime.start.result" && startingExperiment) {
      startingExperiment = false;
      const result = message.result as { ok: boolean; failure?: { message: string } };
      operation.synchronize();
      // The engine's reply to the start (A2.8): a started simulator continues the project test with its review; a refused or failed
      // start shows the engine's sentence.
      const failure = result.ok ? undefined : studioFailureOf(result as Record<string, unknown>);
      if (failure) operation.failure("model", failure);
      else operation.notice("model", "");
      if (result.ok) experiments?.primaryAction();
    }
    synchronizeTools();
  });
  /** Fills the editors from the study's recorded inputs, as the engine recorded them when the loop opened (A8.6, §0.1a `inputs`). */
  const showRecordedInputs = (recorded: Record<string, unknown>): void => {
    if (isRecord(recorded.acquisition)) restoreAcquisition(recorded.acquisition as unknown as AcquisitionSettings);
    // The method the page showed last, which the study keeps as its view, wins over the recorded live method.
    if (openedMethod && Array.from(acquisitionMethod.options).some(option => option.value === openedMethod)) {
      acquisitionMethod.value = openedMethod;
      showAcquisitionMethod();
    }
    openedMethod = undefined;
    // The recording selection the study keeps shows the selectors again (B3.6): the answer on open names it, and nothing else does.
    if (isRecord(recorded.recordingSelection)) bindings?.restoreRecording(recorded.recordingSelection as unknown as RecordingSignalSelection);
    // The controller family and the pins that apply to the method the study holds are the engine's (`heldInputs`, A8.6, C4): the page picks
    // them out of no record. The state design's draft is the study's, and shows as the engine recorded it.
    const stateDesign = isRecord(recorded.stateDesign) ? recorded.stateDesign : undefined;
    const method = typeof recorded.method === "string" ? recorded.method : "automatic";
    controller?.restoreChoice(openedHeld?.family ?? undefined);
    tuning?.restoreInputs({
      method: method as TuningPreferencesView["method"],
      ...(isRecord(recorded.settings) ? { settings: recorded.settings as Record<string, number> } : {}),
      ...(openedHeld?.pins ? { pins: openedHeld.pins } : {}),
      ...(stateDesign && isRecord(stateDesign.draft) ? { stateDraft: stateDesign.draft as Record<string, string> } : {}),
    });
  };
  /**
   * True from the engine's open answer until the recorded inputs that follow it have filled the editors: the open answer is the one answer that
   * fills them from the study (A8.6). The answer to a later change echoes what the engineer has set, and an echo that reaches the page after a
   * newer edit must not put the older value back, nor replace the control the engineer is editing.
   */
  let editorsAwaitRecordedInputs = false;
  /**
   * The study's recorded inputs (§0.1a `inputs`), which an opened loop answers and each change answers again. Every answer renews the
   * engine's own records (the requirements, and which values stay Automatic, B4.3); the editors take the rest once, when the loop opens.
   */
  const restoreRecordedInputs = (recorded: Record<string, unknown>): void => {
    if (!loop) return;
    if (isRecord(recorded.requirements)) acceptRequirements(recorded.requirements as Partial<TuningRequirements>);
    if (Array.isArray(recorded.scenarios)) scenarioDefinitions = recorded.scenarios as typeof scenarioDefinitions;
    if (editorsAwaitRecordedInputs) {
      editorsAwaitRecordedInputs = false;
      showRecordedInputs(recorded);
    }
    // The gates follow every answer: the engine's stage statuses changed with it (applyStageStatuses).
    operation.synchronize();
    synchronizeTools();
  };
  /** The method the opened loop's view names; the recorded inputs that follow the open answer apply it (A8.6). */
  let openedMethod: string | undefined;
  /** What the open answer says the study's recorded inputs mean for the editors (`heldInputs`, A8.6); the recorded inputs that follow it apply it. */
  let openedHeld: HeldInputs | undefined;
  /** The project test's form (B6.1, C12): its signals, its duration and baseline, and the parameter tool, which the page fills before it wires the test. */
  const openProjectTest = (form: ProjectTestForm): void => {
    if (experiments) return;
    fillProjectTest(root, form);
    // The section's signals are the engine's, so their dropdown is made once the section holds them (the other dropdowns are made at start).
    const signals = root.querySelector<HTMLSelectElement>("[data-three-page-experiment-signals]");
    if (signals) initializeMultiSelect(signals);
    experiments = new ThreePageExperiments(root, form, post, () => operation.stageBlock());
  };
  /** The engine's open answer (B3.2, B3.3, B6.1): the Method list it offers, the Limits (which hold the study's record), and the project test's form. */
  const receiveOpened = (answer: Record<string, unknown>): void => {
    if (!loop) return;
    editorsAwaitRecordedInputs = true;
    openedHeld = isRecord(answer.heldInputs) ? answer.heldInputs as HeldInputs : undefined;
    const methods = Array.isArray(answer.identifyMethods) ? answer.identifyMethods as IdentifyMethodOffer[] : [];
    offers.clear();
    acquisitionMethod.replaceChildren(...methods.map(offer => {
      offers.set(offer.value, offer);
      return new Option(offer.label, offer.value);
    }));
    // The study holds the method the engine names; a study that records none holds the list's first method, as the engine lists it. No change
    // waits for an answer.
    methodList?.reset();
    methodList?.holds(openedHeld?.identifyMethod ?? acquisitionMethod.value);
    const experimentDefaults = isRecord(answer.experimentLimits) ? answer.experimentLimits as ExperimentLimits : undefined;
    if (experimentDefaults) limits = limitsOf(experimentDefaults);
    // The project test exists when the open answer says so (`projectTest`); its form comes from the engine, which reviews its declared
    // experiment (C12).
    if (answer.projectTest === true && isRecord(answer.projectTestForm)) openProjectTest(answer.projectTestForm as ProjectTestForm);
    showAcquisitionMethod();
    synchronizeTools();
  };
  window.addEventListener(ENGINE_OPENED_EVENT, event => receiveOpened((event as CustomEvent<Record<string, unknown>>).detail));
  window.addEventListener(ENGINE_VIEW_EVENT, event => {
    const method = (event as CustomEvent<Record<string, unknown>>).detail.identifyMethod;
    openedMethod = typeof method === "string" ? method : undefined;
  });
  window.addEventListener(ENGINE_INPUTS_EVENT, event => restoreRecordedInputs((event as CustomEvent<Record<string, unknown>>).detail));
  window.addEventListener(ENGINE_INPUTS_ANSWERED_EVENT, event => {
    const { refused, limits: limitsRefused } = (event as CustomEvent<InputsAnswered>).detail;
    methodList?.answered(refused, limitsRefused);
  });
  // The requirements an engine answer holds (B4.3): the record becomes the page's requirements, which the editor opens with, and nothing else.
  window.addEventListener(ENGINE_REQUIREMENTS_EVENT, event => {
    const requirements = (event as CustomEvent<unknown>).detail;
    if (isRecord(requirements)) acceptRequirements(requirements as Partial<TuningRequirements>);
  });
  root.dataset.ready = "true";
  post("control-studio.ready");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

/** The method that a change of the study inputs asks the study to record (`acquisitionSettings.method`), when it asks for one. */
function methodAskedBy(payload: Record<string, unknown>): string | undefined {
  const settings = payload.acquisitionSettings;
  return isRecord(settings) && typeof settings.method === "string" ? settings.method : undefined;
}

/** What the Requirements dialog posts (B4.3): the requirement rows, or for several outputs the rows typed for each, and the declared scenarios. */
type RequirementsChange = HeldRequirements & { scenarioDefinitions?: Record<string, unknown>[] };
import { acquisitionPreferences, editExperimentLimits, limitsOf, type StudioExperimentLimits } from "./threePageLimits";
import { editControlRequirements, type HeldRequirements, type RequirementEngineNotes } from "./threePageRequirements";
import { bindThreePageHostState, type StudioPage } from "./threePageHostState";
import { ThreePageOperation, type StudioStart } from "./threePageOperation";
import { ThreePageApplication } from "./threePageApplication";
import { ThreePageController } from "./threePageController";
import { bindRecordingOpen } from "./threePageRecording";
import { ThreePageTuning, type TuningPreferencesView } from "./threePageTuning";
import { ThreePageBindings } from "./threePageBindings";
import { runLockedControls } from "./runLock";
import { ENGINE_INPUTS_ANSWERED_EVENT, ENGINE_INPUTS_EVENT, ENGINE_VIEW_EVENT, type InputsAnswered } from "./engineInputs";
import { ENGINE_REQUIREMENTS_EVENT } from "./engineRequirements";
import { ThreePageMethod } from "./threePageMethod";

import { initializeMultiSelect } from "../../webview/multiSelect";

import { editControllerChannels } from "./threePageChannels";

import { ENGINE_OPENED_EVENT } from "./engineShared";
import { fillProjectTest } from "../threePageExperimentTemplate";
import type { AcquisitionSettings, ChannelChoices, ExperimentLimits, HeldInputs, IdentifyMethodOffer, LoopDefinition, ProjectTestForm } from "../generated/OpenResult";
import type { RecordingSignalSelection, TuningRequirements } from "../generated/InputsResult";

import { ThreePageReference } from "./threePageReference";
import { ThreePageSuppliedModel } from "./threePageSuppliedModel";
import { ThreePageExperiments } from "./threePageExperiments";
import { ThreePageIdentification } from "./threePageIdentification";
import { initializeThreePageAnalysis } from "./threePageAnalysis";
import { bindEngineAnalysis } from "./engineAnalysis";
import { initializeThreePageMimoAnalysis, routeMeasuredPaths } from "./threePageMimoAnalysis";
import { renderModelPresentation } from "./threePageModel";
import { initializeChartExportBridge } from "./chartExportBridge";
import { bindPlantSummary } from "./plantSummary";
import { studioFailureOf } from "./studioFailure";
import { ThreePageAnalysisChooser } from "./threePageAnalysisChooser";
import type { IdentifiedModelPresentation } from "../generated/ManualIdentificationAnswer";
