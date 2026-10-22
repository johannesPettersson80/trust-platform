/** The one-page loop setup form: shows the chosen structure's fields and posts that structure's draft as typed. The draft carries every
 * field and every row of the structure, each number as the text typed: the engine reads the numbers, takes the fields the law or design
 * needs, ignores the others and the blank rows, and answers its refusal, which the form shows under the field the refusal names (B1.3).
 * The preselected choices, the rows drawn and the words shown are the engine's: the template renders them, and the form keeps none of
 * its own. */
export function initializeLoopSetupForm(root: HTMLElement, post: (message: unknown) => void): { showError(reason: string, field?: string): void } {
  const form = root.querySelector<HTMLFormElement>("[data-loop-setup]");
  const save = root.querySelector<HTMLButtonElement>("[data-setup-save]");
  const message = root.querySelector<HTMLElement>("[data-setup-message]");
  if (!form || !save || !message) return { showError: () => undefined };
  // The form's fields are fixed when it renders, so each is looked up once.
  const fields = new Map(Array.from(root.querySelectorAll<HTMLInputElement | HTMLSelectElement>("[data-setup-field]"))
    .map(element => [element.dataset.setupField ?? "", element]));
  const value = (field: string): string => fields.get(field)?.value.trim() ?? "";
  const checked = (field: string): boolean => (fields.get(field) as HTMLInputElement | undefined)?.checked === true;
  const structure = (): string => value("structure");
  // A refusal that names the startup-done flag opens its section; the section stays open until the structure changes.
  let startupRefused = false;
  // The words the engine answered, as the template rendered them on the form (B1.3).
  const words = { trueMeans: form.dataset.trueMeans ?? "", notFound: form.dataset.notFound ?? "", notStated: form.dataset.notStated ?? "" };
  /** The indexes of the rows the template drew, which the engine's limits set (B1.3). */
  const rowIndexes = (selector: string, key: string): number[] => Array.from(root.querySelectorAll<HTMLElement>(selector), row => Number(row.dataset[key]));

  /** A text the engine reads as absent when it is blank: sent as null, never as "". */
  const optionalText = (field: string): string | null => value(field) || null;
  /** A signal as the engine reads it (`FormSignal`): its symbol, its unit and its range, each as typed; the engine reads the numbers. */
  const rawSignal = (field: string) => ({ symbol: value(field), unit: value(`${field}.unit`),
    minimum: value(`${field}.minimum`), maximum: value(`${field}.maximum`) });
  /** A gain variable of the program's own law as the engine reads it (`OwnLawGain`): its variable and the range Tune may choose, as typed. */
  const gainOf = (field: string) => ({ symbol: value(field), minimum: value(`${field}.minimum`), maximum: value(`${field}.maximum`) });
  /** A controller's Auto/Manual signal as the engine reads it (`ModeChoice`): its path, and the mode its TRUE value means. */
  const modeOf = (prefix: string) => ({ symbol: value(`${prefix}mode`), whenTrue: value(`${prefix}modeWhenTrue`) });
  /** The startup-done flag, when the user chose one. The engine decides whether the program needs it (C1 Setup). */
  const startupFor = (): Record<string, string> => value("startupFlag") ? { startupFlag: value("startupFlag") } : {};
  /** The chosen structure's draft, posted as typed: the engine checks it and answers the refusal. */
  const draft = (): Record<string, unknown> => {
    switch (structure()) {
      case "state-feedback": return stateFeedbackDraft();
      case "search": return searchDraft();
      case "cascade": return cascadeDraft();
      case "smith": return smithDraft();
      case "schedule": return scheduleDraft();
      case "onoff": return onOffDraft();
      case "group": return groupDraft();
      case "identify": return identifyDraft();
      case "own-law": return { structure: "own-law", ...ownLawFields(ownLaw()) };
      case "trajectory-feedforward": return trajectoryDraft();
      case "feedforward": return feedforwardDraft();
      default: return pidDraft();
    }
  };

  /** The number of states the chosen estimator model has, which sets the estimate rows shown (C4): the engine's count for each model file. */
  const estimatorStates = (): number => Number(root.querySelector<HTMLSelectElement>('[data-setup-field="feedback.estimatorModel"]')?.selectedOptions[0]?.dataset.states ?? 0);
  /** The state-feedback draft as the engine reads it (`StateFeedbackLoopDraft`, C4): the form's values as typed, and no check of its own.
   * It carries the measured gain and further outputs and the estimator for every design; the engine takes the ones the design uses. */
  const stateFeedbackDraft = (): Record<string, unknown> => ({ structure: "state-feedback", name: value("name"), action: value("action"),
    family: value("feedback.family"), instance: value("feedback.instance"),
    manipulatedVariable: rawSignal("feedback.manipulatedVariable"), processVariable: rawSignal("feedback.processVariable"),
    setpoint: { symbol: value("feedback.setpoint") }, gain: value("feedback.gain"), referenceGain: value("feedback.referenceGain"),
    states: furtherOutputs(), estimator: { modelFile: value("feedback.estimatorModel"), measurementNoise: value("feedback.measurementNoise"), states: estimateRows() },
    mode: modeOf("feedback."), manualOutput: value("feedback.manualOutput"), ...startupFor() });
  /** Each further measured output row the template drew, with its range and the variable that holds its gain, as typed. */
  const furtherOutputs = () => rowIndexes("[data-feedback-state]", "feedbackState").map(index => `feedback.state.${index}`)
    .map(prefix => ({ signal: rawSignal(prefix), gain: value(`${prefix}.gain`) }));
  /** Each estimate row the template drew: the estimate's variable, its gain, its observer gain and its process-noise variance, as typed. */
  const estimateRows = () => rowIndexes("[data-estimate-state]", "estimateState").map(index => {
    const prefix = `feedback.estimate.${index}`;
    return { estimate: value(prefix), gain: value(`${prefix}.gain`), observerGain: value(`${prefix}.observerGain`), processNoise: value(`${prefix}.processNoise`) };
  });
  /** The search draft as the engine reads it (`SearchLawLoopDraft`, C5): every coefficient and kept-value row the template drew, as typed. */
  const searchDraft = (): Record<string, unknown> => ({ structure: "search", name: value("name"), action: value("action"),
    family: value("search.family"), instance: value("search.instance"), processVariable: rawSignal("search.processVariable"),
    setpoint: { symbol: value("search.setpoint") }, manipulatedVariable: rawSignal("search.manipulatedVariable"),
    mode: modeOf("search."), manualOutput: value("search.manualOutput"), ...startupFor(),
    coefficients: rowIndexes("[data-search-coefficient]", "searchCoefficient").map(index => ({ symbol: value(`search.coefficient.${index}`),
      unit: value(`search.coefficient.${index}.unit`), values: value(`search.coefficient.${index}.values`) })),
    states: rowIndexes("[data-search-state]", "searchState").map(index => ({ symbol: value(`search.state.${index}`),
      unit: value(`search.state.${index}.unit`), start: value(`search.state.${index}.start`) })) });
  /** The single loop's fields as the engine reads them (`PidLoopDraft`, and the structures that extend it: C9, C10), as typed. */
  const singleLoopFields = (): Record<string, unknown> => ({ instance: value("instance"), parameterRecord: value("parameterRecord"),
    processVariable: rawSignal("processVariable"), manipulatedVariable: rawSignal("manipulatedVariable"), setpoint: { symbol: value("setpoint") },
    mode: modeOf(""), manualOutput: value("manualOutput") });
  /** One library PID as the engine reads it (`PidLoopDraft`, C1), as typed. */
  const pidDraft = (): Record<string, unknown> => ({ structure: "pid", name: value("name"), action: value("action"), ...startupFor(), ...singleLoopFields() });
  /** The identify-only draft as the engine reads it (`IdentifyLoopDraft`, C12): the two signals and the recording duration, as typed. The engine
   * reads the duration's text (a number of seconds or words such as "15 min"). */
  const identifyDraft = (): Record<string, unknown> => ({ structure: "identify", name: value("name"), manipulatedVariable: rawSignal("identify.manipulatedVariable"),
    processVariable: rawSignal("identify.processVariable"), experimentDurationSeconds: optionalText("identify.durationSeconds") });
  /** A program's own P, PI or PID law as the engine reads it (`OwnLawLoopDraft`, C2): its variables and gain ranges as typed. The draft
   * carries the integral and derivative variables for every law; the engine takes the ones the law uses. */
  const ownLawFields = (law: string): Record<string, unknown> => ({ name: value("name"), action: value("action"), law, instance: value("own.instance"),
    processVariable: rawSignal("own.processVariable"), setpoint: { symbol: value("own.setpoint") }, manipulatedVariable: rawSignal("own.manipulatedVariable"),
    mode: modeOf("own."), manualOutput: value("own.manualOutput"), proportionalGain: gainOf("own.proportionalGain"), ...startupFor(),
    integralGain: gainOf("own.integralGain"), integralTerm: optionalText("own.integralTerm"),
    derivativeGain: gainOf("own.derivativeGain"), derivativeState: optionalText("own.derivativeState"), derivativeOn: optionalText("own.derivativeOn") });
  /** The trajectory feedforward as the engine reads it (`TrajectoryLoopDraft`, C11): the program's PID law, the two feedforward gains, the
   * planned trajectory and the feedforward term, as typed. */
  const trajectoryDraft = (): Record<string, unknown> => ({ structure: "trajectory-feedforward", ownLaw: ownLawFields(ownLaw()),
    rateGain: gainOf("traj.rateGain"), accelerationGain: gainOf("traj.accelerationGain"), plannedTrajectory: rawSignal("traj.plannedTrajectory"),
    feedforwardTerm: optionalText("traj.feedforwardTerm") });
  /** The feedforward as the engine reads it (`FeedforwardLoopDraft`, C8): the PID's fields, the measured disturbance, its command and step, and the
   * feedforward variables, as typed. */
  const feedforwardDraft = (): Record<string, unknown> => ({ structure: "feedforward", name: value("name"), action: value("action"), ...startupFor(),
    ...singleLoopFields(), disturbance: rawSignal("ff.disturbance"), disturbanceCommand: optionalText("ff.disturbanceCommand"),
    disturbanceStep: value("ff.disturbanceStep"),
    feedforward: { gain: value("ff.gain"), lead: optionalText("ff.lead"), lag: optionalText("ff.lag"), filtered: optionalText("ff.filtered") } });
  /** The cascade draft as the engine reads it (`CascadeLoopDraft`, C6): both controllers and their signals as typed. The outer has no manual output. */
  const cascadeDraft = (): Record<string, unknown> => ({ structure: "cascade", name: value("name"), action: value("action"), ...startupFor(),
    outer: { instance: value("outer.instance"), parameterRecord: value("outer.parameterRecord"), mode: modeOf("outer."),
      processVariable: rawSignal("outer.processVariable"), setpoint: { symbol: value("outer.setpoint") } },
    inner: { instance: value("inner.instance"), parameterRecord: value("inner.parameterRecord"), mode: modeOf("inner."),
      manualOutput: value("inner.manualOutput"), processVariable: rawSignal("inner.processVariable"),
      manipulatedVariable: rawSignal("inner.manipulatedVariable") } });
  /** The Smith predictor's draft as the engine reads it (`SmithLoopDraft`, C9): the single loop, the model's variables and, when the
   * program uses the library block, its four variables together. */
  const smithDraft = (): Record<string, unknown> => ({ structure: "smith", name: value("name"), action: value("action"), ...singleLoopFields(),
    ...startupFor(), model: { gain: value("smith.gain"), timeConstant: value("smith.timeConstant"), delay: value("smith.delay"), correction: value("smith.correction") },
    ...(checked("smith.libraryBlock") ? { libraryBlock: { output: value("smith.block.output"), maximumCorrection: value("smith.block.maximumCorrection"),
      operatingInput: value("smith.block.operatingInput"), operatingOutput: value("smith.block.operatingOutput") } } : {}) });
  /** The gain schedule's draft as the engine reads it (`ScheduleLoopDraft`, C10): the single loop, the scheduling variable and every
   * operating-point row the template drew, as typed; the engine takes the rows that have an entry. */
  const scheduleDraft = (): Record<string, unknown> => ({ structure: "schedule", name: value("name"), action: value("action"), ...singleLoopFields(),
    ...startupFor(), scheduling: { symbol: value("schedule.variable"), unit: value("schedule.variable.unit") },
    points: rowIndexes("[data-schedule-point]", "schedulePoint").map(index => ({ name: value(`schedule.${index}.name`), value: value(`schedule.${index}.value`),
      tolerance: value(`schedule.${index}.tolerance`), proportionalGain: value(`schedule.${index}.proportionalGain`), integralGain: value(`schedule.${index}.integralGain`) })) });
  /** The ON/OFF controller's draft as the engine reads it (`OnOffLoopDraft`, C3): the switched output, its levels and its equipment limits as
   * typed. The engine reads a blank level as the output's limit and a blank equipment time as not declared, never zero. */
  const onOffDraft = (): Record<string, unknown> => ({ structure: "onoff", name: value("name"), action: value("action"), ...startupFor(),
    instance: value("onoff.instance"), processVariable: rawSignal("onoff.processVariable"), setpoint: { symbol: value("onoff.setpoint") },
    manipulatedVariable: rawSignal("onoff.manipulatedVariable"), mode: modeOf("onoff."), manualOutput: value("onoff.manualOutput"),
    hysteresis: value("onoff.hysteresis"), bandConvention: value("onoff.bandConvention"),
    minimumOn: value("onoff.minimumOn"), minimumOff: value("onoff.minimumOff"), latch: value("onoff.latch"),
    ...(value("onoff.dwellTimer") ? { dwellTimer: value("onoff.dwellTimer") } : {}),
    equipmentMinimumOnSeconds: value("onoff.equipmentMinimumOn"), equipmentMinimumOffSeconds: value("onoff.equipmentMinimumOff"),
    lowOutput: value("onoff.lowOutput"), highOutput: value("onoff.highOutput"),
    ...(value("onoff.thresholdEquality") ? { thresholdEquality: value("onoff.thresholdEquality") } : {}) });
  /** The PID group's draft as the engine reads it (`GroupLoopDraft`, C7): the included members and the decoupler's variables, as typed.
   * The engine checks it, including the decoupler's rule that a member drives a plant input, not its controller's own output. */
  const groupDraft = (): Record<string, unknown> => {
    const prefixes = included(), decoupled = checked("group.decoupled");
    const members = prefixes.map(prefix => ({ instance: value(`${prefix}instance`), parameterRecord: value(`${prefix}parameterRecord`),
      processVariable: rawSignal(`${prefix}processVariable`), manipulatedVariable: rawSignal(`${prefix}manipulatedVariable`),
      setpoint: { symbol: value(`${prefix}setpoint`) }, manualOutput: value(`${prefix}manualOutput`) }));
    const decoupler = decoupled ? members.map((_, row) => members.map((_, column) => value(`group.decoupler.${row}.${column}`))) : undefined;
    return { structure: "group", name: value("name"), action: value("action"), ...startupFor(),
      mode: modeOf("group."), members, ...(decoupler ? { decoupler } : {}) };
  };
  /** The controller instances the chosen structure runs, as the form names their fields. The startup section asks for a flag when one of them needs it. */
  const chosenInstances = (): string[] => {
    const names = structure() === "cascade" ? ["outer.instance", "inner.instance"] : structure() === "group" ? included().map(prefix => `${prefix}instance`)
      : structure() === "own-law" || structure() === "trajectory-feedforward" ? ["own.instance"] : structure() === "onoff" ? ["onoff.instance"]
      : structure() === "state-feedback" ? ["feedback.instance"] : structure() === "search" ? ["search.instance"] : ["instance"];
    return names.map(name => value(name));
  };
  // A trajectory feedforward's program law is the one its section holds (C11, the engine's preselection); the other own-law setups take the chosen law.
  const ownLaw = (): string => structure() === "trajectory-feedforward" ? value("traj.ownLaw") : value("own.law");
  const included = (): string[] => Array.from(root.querySelectorAll<HTMLInputElement>('[data-setup-field^="group."][data-setup-field$=".include"]'))
    .filter(box => box.checked).map(box => (box.dataset.setupField ?? "").replace(/include$/, ""));
  /** The controllers whose program needs the startup-done flag at their default settings: the engine's answer (C1 Setup). */
  const flaggedInstances = new Set(Array.from(root.querySelectorAll<HTMLElement>("[data-setup-startup-instances] li")).map(item => item.textContent ?? ""));
  /** A chosen controller fills in what its own calls say, except fields the user has chosen. */
  const prefills = new Map(Array.from(root.querySelectorAll<HTMLElement>("[data-setup-prefills] li")).map(item => [item.dataset.instance ?? "", item.dataset]));
  /** The loop name the engine starts from for each library PID (B1.3). */
  const loopNames = new Map(Array.from(root.querySelectorAll<HTMLElement>("[data-setup-loop-names] li")).map(item => [item.dataset.instance ?? "", item.dataset.name ?? ""]));
  // The Auto/Manual signal, what its TRUE means and the startup-done flag come from the code too (Specification 35 §17.1).
  const prefilled: Record<string, readonly [string, string][]> = {
    "": [["parameterRecord", "parameterRecord"], ["manipulatedVariable", "manipulatedVariable"], ["setpoint", "setpoint"], ["processVariable", "processValue"], ["manualOutput", "manualOutput"],
      ["mode", "mode"], ["modeWhenTrue", "modeWhenTrue"], ["startupFlag", "startupFlag"]],
    "outer.": [["outer.parameterRecord", "parameterRecord"], ["outer.processVariable", "processValue"], ["outer.setpoint", "setpoint"],
      ["outer.mode", "mode"], ["outer.modeWhenTrue", "modeWhenTrue"]],
    "inner.": [["inner.parameterRecord", "parameterRecord"], ["inner.manualOutput", "manualOutput"], ["inner.processVariable", "processValue"], ["inner.manipulatedVariable", "manipulatedVariable"],
      ["inner.mode", "mode"], ["inner.modeWhenTrue", "modeWhenTrue"]],
  };
  const memberFields = (prefix: string): readonly [string, string][] => [[`${prefix}parameterRecord`, "parameterRecord"], [`${prefix}processVariable`, "processValue"],
    [`${prefix}manipulatedVariable`, "manipulatedVariable"], [`${prefix}setpoint`, "setpoint"], [`${prefix}manualOutput`, "manualOutput"]];
  const fillFrom = (prefix: string): void => {
    const known = prefills.get(value(`${prefix}instance`));
    for (const [field, key] of prefilled[prefix] ?? (prefix.startsWith("group.") ? memberFields(prefix) : [])) {
      const select = root.querySelector<HTMLSelectElement>(`[data-setup-field="${field}"]`), symbol = known?.[key];
      if (!select || select.dataset.edited === "true" || !symbol || !Array.from(select.options).some(option => option.value === symbol)) continue;
      select.value = symbol;
    }
  };
  form.addEventListener("change", event => {
    const target = event.target as HTMLElement | null, field = target?.dataset?.setupField;
    if (!target || !field) return;
    if (field === "structure") startupRefused = false;
    if (field.endsWith("instance")) fillFrom(field.slice(0, -"instance".length));
    else if (target.tagName === "SELECT") target.dataset.edited = "true";
  });
  for (const prefix of Object.keys(prefilled)) fillFrom(prefix);
  root.querySelectorAll<HTMLInputElement>('[data-setup-field^="group."][data-setup-field$=".instance"]')
    .forEach(input => fillFrom((input.dataset.setupField ?? "").replace(/instance$/, "")));
  // What the code states is shown in words. Change reveals the selector that changes a fact; the loop name follows the controller
  // until it is typed, and the unit and range fields appear only where the code did not supply them.
  const chosenText = (field: string): string => {
    const element = fields.get(field);
    if (!element) return "";
    if (element.tagName !== "SELECT") return element.value.trim();
    const select = element as HTMLSelectElement;
    return select.value ? select.selectedOptions[0]?.textContent?.trim() ?? "" : "";
  };
  // The compiled task of each PID comes from the program; it is shown, never chosen.
  const tasks = new Map(Array.from(root.querySelectorAll<HTMLElement>("[data-setup-tasks] li")).map(item => [item.dataset.instance ?? "", item.dataset.task ?? ""]));
  const refreshFound = (): void => {
    for (const element of Array.from(root.querySelectorAll<HTMLElement>("[data-setup-found-value]"))) {
      const field = element.dataset.setupFoundValue ?? "";
      const text = field === "mode" ? (chosenText("mode") ? `${chosenText("mode")} · ${words.trueMeans} ${chosenText("modeWhenTrue")}` : "")
        : field === "task" ? tasks.get(value("instance")) ?? ""
        : chosenText(field);
      element.textContent = text || (field === "task" ? words.notStated : words.notFound);
    }
  };
  const nameField = fields.get("name") as HTMLInputElement | undefined;
  if (nameField) nameField.addEventListener("input", () => { nameField.dataset.auto = "false"; });
  const prefillName = (): void => {
    const offered = loopNames.get(value("instance"));
    if (!nameField || nameField.dataset.auto === "false" || offered === undefined) return;
    nameField.value = offered;
    nameField.dataset.auto = "true";
  };
  for (const button of Array.from(root.querySelectorAll<HTMLButtonElement>("[data-setup-change]"))) {
    button.addEventListener("click", () => {
      const panel = root.querySelector<HTMLElement>(`[data-setup-change-panel="${button.dataset.setupChange ?? ""}"]`);
      if (!panel) return;
      panel.hidden = !panel.hidden;
      button.setAttribute("aria-expanded", String(!panel.hidden));
      if (!panel.hidden) panel.querySelector<HTMLElement>("select, input")?.focus();
    });
  }

  /** The engine's refusal sits under the field it names (`Failure.field`), or in the message area when it names none or the field is hidden (B1.3). */
  let refusal: HTMLElement | undefined;
  const clearRefusal = (): void => { refusal?.remove(); refusal = undefined; };
  const showRefusal = (reason: string, field = ""): void => {
    clearRefusal();
    if (field === "startupFlag") {
      startupRefused = true;
      const startup = root.querySelector<HTMLElement>("[data-setup-startup]");
      if (startup) startup.hidden = false;
    }
    const element = fields.get(field);
    const visible = element !== undefined && !element.closest("[hidden]");
    message.textContent = visible ? "" : reason;
    if (!element || !visible) return;
    refusal = root.ownerDocument.createElement("p");
    refusal.className = "studio-action-message";
    refusal.setAttribute("role", "alert");
    refusal.textContent = reason;
    (element.closest("label") ?? element).insertAdjacentElement("afterend", refusal);
  };
  const render = (): void => {
    clearRefusal();
    message.textContent = "";
    prefillName();
    refreshFound();
    for (const group of Array.from(root.querySelectorAll<HTMLElement>("[data-setup-range-group]"))) {
      const field = group.dataset.setupRangeGroup ?? "";
      group.hidden = Boolean(value(`${field}.unit`) && value(`${field}.minimum`) && value(`${field}.maximum`));
    }
    // A further coefficient or kept value appears once the previous one is chosen.
    for (const [attribute, field] of [["data-search-coefficient", "search.coefficient"], ["data-search-state", "search.state"]] as const) {
      root.querySelectorAll<HTMLElement>(`[${attribute}]`).forEach(row => {
        const index = Number(row.getAttribute(attribute));
        row.hidden = index > 0 && !value(`${field}.${index - 1}`) && !value(`${field}.${index}`);
      });
    }
    const integralFields = root.querySelector<HTMLElement>("[data-own-integral]");
    if (integralFields) integralFields.hidden = ownLaw() === "P";
    const derivativeFields = root.querySelector<HTMLElement>("[data-own-derivative]");
    if (derivativeFields) derivativeFields.hidden = ownLaw() !== "PID";
    // A trajectory feedforward's law is PID by its structure, so its law choice does not show.
    const lawChoice = root.querySelector<HTMLElement>("[data-own-law-choice]");
    if (lawChoice) lawChoice.hidden = structure() === "trajectory-feedforward";
    // A further measured output appears once the previous one is chosen; LQG shows one estimate per state of its model instead.
    const lqg = value("feedback.family") === "LQG";
    root.querySelectorAll<HTMLElement>("[data-feedback-state]").forEach(row => {
      const index = Number(row.dataset.feedbackState);
      row.hidden = lqg || index > 0 && !value(`feedback.state.${index - 1}`) && !value(`feedback.state.${index}`);
    });
    const measuredGain = root.querySelector<HTMLElement>("[data-feedback-measured]"), estimator = root.querySelector<HTMLElement>("[data-feedback-estimator]");
    if (measuredGain) measuredGain.hidden = lqg;
    if (estimator) estimator.hidden = !lqg;
    root.querySelectorAll<HTMLElement>("[data-estimate-state]").forEach(row => { row.hidden = Number(row.dataset.estimateState) >= estimatorStates(); });
    root.querySelectorAll<HTMLElement>("[data-group-member]").forEach(member => {
      const fields = member.querySelector<HTMLElement>("[data-group-member-fields]");
      if (fields) fields.hidden = !member.querySelector<HTMLInputElement>('input[type="checkbox"]')?.checked;
    });
    // A feedforward or a Smith predictor is one PID loop plus its own variables. A trajectory feedforward is the program's own PID law
    // (its section) plus the trajectory's variables (its section).
    const shown = structure() === "trajectory-feedforward" ? ["own-law", "trajectory-feedforward"] : [structure()];
    for (const section of Array.from(root.querySelectorAll<HTMLElement>("[data-setup-section]"))) section.hidden = !shown.includes(section.dataset.setupSection ?? "")
      && !(section.dataset.setupSection === "pid" && ["feedforward", "smith", "schedule"].includes(structure()));
    // A decoupler has one entry per plant input and included member.
    const decoupled = checked("group.decoupled"), size = included().length;
    const grid = root.querySelector<HTMLElement>("[data-group-decoupler]");
    if (grid) grid.hidden = !decoupled;
    root.querySelectorAll<HTMLElement>("[data-decoupler-entry]").forEach(entry => {
      const [row, column] = (entry.dataset.decouplerEntry ?? "").split(".").map(Number);
      entry.hidden = !decoupled || row >= size || column >= size;
    });
    // A further operating point appears once the previous one has a name. The points the engine always asks for are not optional (C10).
    root.querySelectorAll<HTMLElement>("[data-schedule-point]").forEach(row => {
      const index = Number(row.dataset.schedulePoint);
      row.hidden = row.hasAttribute("data-schedule-optional") && !value(`schedule.${index - 1}.name`) && !value(`schedule.${index}.name`);
    });
    // The Smith predictor's library block is asked together, and only when the program uses it (C9).
    const smithBlock = root.querySelector<HTMLElement>("[data-smith-block]");
    if (smithBlock) smithBlock.hidden = !checked("smith.libraryBlock");
    // A blank output level is the output's limit (C3): the limit shows in the level's place until a level is typed.
    for (const [field, limit] of [["onoff.lowOutput", "onoff.manipulatedVariable.minimum"], ["onoff.highOutput", "onoff.manipulatedVariable.maximum"]]) {
      const level = fields.get(field) as HTMLInputElement | undefined;
      if (level) level.placeholder = value(limit) || "output limit";
    }
    // The startup-done section shows where the engine asks for a flag: a chosen controller that needs one, a refusal that names it, or a flag
    // the user has already chosen (C1 Setup).
    const startup = root.querySelector<HTMLElement>("[data-setup-startup]");
    if (startup) startup.hidden = !(startupRefused || value("startupFlag") || chosenInstances().some(name => flaggedInstances.has(name)));
  };
  form.addEventListener("input", render);
  form.addEventListener("change", render);
  root.querySelector("[data-setup-step-by-step]")?.addEventListener("click", () => post({ kind: "control-studio.setup.step-by-step" }));
  save.addEventListener("click", () => post({ kind: "control-studio.setup.save", draft: draft() }));
  render();
  form.dataset.ready = "true";
  // A refused save keeps the form and its values; the reason sits under the field it names, or in the message area.
  return { showError: (reason, field) => showRefusal(reason, field) };
}
