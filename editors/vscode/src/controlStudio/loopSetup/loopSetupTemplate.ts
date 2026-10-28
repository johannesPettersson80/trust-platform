import type { EstimatorModelFile, SetupFormModel } from "../generated/SetupDiscoverResult";

/** The setup form's model as the page reads it: the engine's answer, with the model files an LQG estimator can implement (C4). */
export type SetupPageModel = SetupFormModel & { estimatorModels?: EstimatorModelFile[] };

/** Each value of an engine list with the engine's words for it (`SetupFormModel.labels`); a value without words shows as itself. */
const labelled = (values: readonly string[], words: Readonly<Record<string, string>>): [string, string][] =>
  values.map((value): [string, string] => [value, words[value] ?? value]);

/** The body of the one-page loop setup form. Choices come only from the compiled model; nothing is guessed. The preselected choices, the
 * rows drawn and the words shown are the engine's answer (B1.3). Every number field is a text field, because the engine reads the text
 * typed (B1.3): a number input would apply the browser's number grammar and post an entry it cannot read, such as `1,5`, as blank. */
export function renderLoopSetupBody(model: SetupPageModel, escape: (value: string) => string): string {
  const { defaults, limits, wording, choices, labels } = model;
  const options = (values: readonly string[], selected?: string | null, empty = "Choose…") =>
    `<option value="">${escape(empty)}</option>${values.map(value => `<option value="${escape(value)}"${value === selected ? " selected" : ""}>${escape(value)}</option>`).join("")}`;
  const select = (field: string, label: string, values: readonly string[], selected?: string | null) =>
    `<label class="studio-field">${escape(label)}<select data-setup-field="${field}">${options(values, selected)}</select></label>`;
  const choice = (field: string, label: string, values: readonly [string, string][], selected: string) =>
    `<label class="studio-field">${escape(label)}<select data-setup-field="${field}">${values.map(([value, text]) =>
      `<option value="${value}"${value === selected ? " selected" : ""}>${escape(text)}</option>`).join("")}</select></label>`;
  /** The Auto/Manual choice of a TRUE value: labelled with the engine's words, preselected at the engine's default. */
  const modeWhenTrue = (field: string) => choice(field, wording.trueMeans, labelled(choices.modeWhenTrue, labels.modeWhenTrue), defaults.modeWhenTrue);
  /** A loop signal with its unit and range: measurements from what a simulated plant can write, outputs from every signal. */
  const signal = (field: string, label: string, selected?: string, options: readonly string[] = model.measurements) =>
    `${select(field, label, options, selected)}
  <div class="studio-field-group">
   <label class="studio-field">Unit<input type="text" data-setup-field="${field}.unit"></label>
   <label class="studio-field">Minimum<input type="text" data-setup-field="${field}.minimum"></label>
   <label class="studio-field">Maximum<input type="text" data-setup-field="${field}.maximum"></label>
  </div>`;
  // What the code states is shown in words; its Change link reveals the selector that can change it (Specification 35 B1.3).
  const found = (field: string, label: string, control: string) => `<div class="studio-setup-found" data-setup-found="${field}">
   <span class="studio-setup-found-label">${escape(label)}</span>
   <span class="studio-setup-found-value" data-setup-found-value="${field}"></span>
   <button type="button" class="studio-link" data-setup-change="${field}" aria-expanded="false">Change</button>
   <div class="studio-setup-change" data-setup-change-panel="${field}" hidden>${control}</div>
  </div>`;
  /** The unit and range of a signal, asked only where the code did not supply them. */
  const rangeGroup = (field: string, label: string) => `<div class="studio-setup-range" data-setup-range-group="${field}">
   <p class="studio-setup-ask">${escape(label)}: its unit and range, which the code does not state</p>
   <div class="studio-field-group">
    <label class="studio-field">Unit<input type="text" data-setup-field="${field}.unit"></label>
    <label class="studio-field">Minimum<input type="text" data-setup-field="${field}.minimum"></label>
    <label class="studio-field">Maximum<input type="text" data-setup-field="${field}.maximum"></label>
   </div>
  </div>`;
  /** A variable holding a gain of the program's own law, with the range Tune may choose from; its unit follows the signals'. */
  const gain = (field: string, label: string) => `${select(field, label, model.reals)}
  <div class="studio-field-group">
   <label class="studio-field">Minimum<input type="text" data-setup-field="${field}.minimum"></label>
   <label class="studio-field">Maximum<input type="text" data-setup-field="${field}.maximum"></label>
  </div>`;
  const pidNames = model.pids.map(pid => pid.instance);
  // Studio writes a manual output by name, so any REAL/LREAL serves; it needs no I/O address (Specification 35 §17.1).
  const manualOutputs = model.signals;
  // Apply switches the Auto/Manual signal directly, so only a BOOL without an I/O address can be one.
  const modes = model.modes;
  // A cascade's outer controller has no manual output: nothing in its structure uses one (C6).
  const controller = (prefix: string, label: string, selected?: string, manual = true) => `
  ${select(`${prefix}instance`, label, pidNames, selected)}
  ${select(`${prefix}parameterRecord`, "Parameter record", model.records)}
  ${select(`${prefix}mode`, "Auto/Manual signal", modes)}
  ${modeWhenTrue(`${prefix}modeWhenTrue`)}
  ${manual ? select(`${prefix}manualOutput`, "Manual output", manualOutputs) : ""}`;
  const prefill = model.prefill;
  // The structures the engine offers for this program, in its order, with its words.
  const structures = labelled(model.structures, labels.structures);
  const band = labelled(choices.bandConventions, labels.bandConventions);
  // One block per compiled PID; its own calls fill it in once included.
  const member = (instance: string, index: number) => `<fieldset class="studio-group-member" data-group-member>
   <legend><label class="studio-group-include"><input type="checkbox" data-setup-field="group.${index}.include"> ${escape(instance)}</label></legend>
   <input type="hidden" data-setup-field="group.${index}.instance" value="${escape(instance)}">
   <div class="studio-fields" data-group-member-fields hidden>
    ${select(`group.${index}.parameterRecord`, "Parameter record", model.records)}
    ${signal(`group.${index}.processVariable`, "Measurement")}
    ${signal(`group.${index}.manipulatedVariable`, "Output", undefined, model.signals)}
    ${select(`group.${index}.setpoint`, "Setpoint", model.signals)}
    ${select(`group.${index}.manualOutput`, "Manual output", manualOutputs)}
     </div>
  </fieldset>`;
  return `<form class="studio-fields" data-loop-setup novalidate data-true-means="${escape(wording.trueMeans)}" data-not-found="${escape(wording.notFound)}" data-not-stated="${escape(wording.notStated)}">
 <label class="studio-field">Loop name<input type="text" data-setup-field="name"></label>
 ${choice("structure", "Structure", structures, defaults.structure)}
 <section class="studio-fields" data-setup-section="pid">
  <div class="studio-setup-summary" data-setup-summary>
   ${found("instance", "Controller", select("instance", "Controller", pidNames, prefill?.instance))}
   ${found("parameterRecord", "Parameter record", select("parameterRecord", "Parameter record", model.records, prefill?.parameterRecord))}
   ${found("processVariable", "Measurement", select("processVariable", "Measurement", model.measurements, prefill?.processValue))}
   ${found("manipulatedVariable", "Output", select("manipulatedVariable", "Output", model.signals, prefill?.manipulatedVariable))}
   ${found("setpoint", "Setpoint", select("setpoint", "Setpoint", model.signals, prefill?.setpoint))}
   ${found("mode", "Auto/Manual signal", `${select("mode", "Auto/Manual signal", modes)}${modeWhenTrue("modeWhenTrue")}`)}
   ${found("manualOutput", "Manual output", select("manualOutput", "Manual output", manualOutputs, prefill?.manualOutput))}
   <div class="studio-setup-found" data-setup-found="task"><span class="studio-setup-found-label">Task</span><span class="studio-setup-found-value" data-setup-found-value="task"></span></div>
   <ul hidden data-setup-tasks>${model.pids.map(pid => `<li data-instance="${escape(pid.instance)}" data-task="${escape(pid.task ? `${pid.task.name} (${pid.task.intervalMs} ms)` : "")}"></li>`).join("")}</ul>
  </div>
  ${rangeGroup("processVariable", "Measurement")}
  ${rangeGroup("manipulatedVariable", "Output")}
 </section>
 <section class="studio-fields" data-setup-section="feedforward">
  ${signal("ff.disturbance", "Measured disturbance", undefined, model.signals)}
  ${select("ff.disturbanceCommand", "Variable Identify steps to move the disturbance", manualOutputs)}
  <label class="studio-field">Identification step of the disturbance, in its unit<input type="text" data-setup-field="ff.disturbanceStep"></label>
  ${select("ff.gain", "Feedforward gain", model.reals)}
  <label class="studio-field">Lead time (s)<select data-setup-field="ff.lead">${options(model.reals, undefined, "None: no lead/lag")}</select></label>
  <label class="studio-field">Lag time (s)<select data-setup-field="ff.lag">${options(model.reals, undefined, "None")}</select></label>
  <label class="studio-field">Filtered disturbance the lag keeps<select data-setup-field="ff.filtered">${options(model.reals, undefined, "None")}</select></label>
 </section>
 <section class="studio-fields" data-setup-section="schedule">
  ${select("schedule.variable", "Scheduling variable", model.reals)}
  <label class="studio-field">Unit of the scheduling variable<input type="text" data-setup-field="schedule.variable.unit"></label>
  ${model.schedulePoints.map(({ required }, index) => `<div class="studio-fields" data-schedule-point="${index}"${required ? "" : " data-schedule-optional"}>
   <label class="studio-field">Operating point ${index + 1}<input type="text" data-setup-field="schedule.${index}.name"></label>
   <label class="studio-field">Value of the scheduling variable<input type="text" data-setup-field="schedule.${index}.value"></label>
   <label class="studio-field">Tolerance on that value, in the scheduling variable's unit<input type="text" data-setup-field="schedule.${index}.tolerance"></label>
   ${select(`schedule.${index}.proportionalGain`, "Its proportional gain variable", model.reals)}
   ${select(`schedule.${index}.integralGain`, "Its integral gain variable", model.reals)}
  </div>`).join("")}
 </section>
 <section class="studio-fields" data-setup-section="smith">
  ${select("smith.gain", "Model gain K", model.reals)}
  ${select("smith.timeConstant", "Time constant T (s)", model.reals)}
  ${select("smith.delay", "Delay θ (s)", model.reals)}
  ${select("smith.correction", "Correction c the program keeps", model.reals)}
  <label class="studio-field"><input type="checkbox" data-setup-field="smith.libraryBlock"> The program uses the library block Control.SmithPredictorFOPDT</label>
  <div class="studio-fields" data-smith-block hidden>
   ${select("smith.block.output", "Block output (SmithPredictorOutput)", model.reals)}
   ${select("smith.block.maximumCorrection", "Variable of MaximumCorrection", model.reals)}
   ${select("smith.block.operatingInput", "Variable of OperatingInput", model.reals)}
   ${select("smith.block.operatingOutput", "Variable of OperatingOutput", model.reals)}
  </div>
 </section>
 <section class="studio-fields" data-setup-section="cascade">
  ${controller("outer.", "Outer controller", model.cascade?.outer, false)}
  ${signal("outer.processVariable", "Outer measurement")}
  ${select("outer.setpoint", "Outer setpoint", model.signals)}
  ${controller("inner.", "Inner controller", model.cascade?.inner)}
  ${signal("inner.processVariable", "Inner measurement")}
  ${signal("inner.manipulatedVariable", "Output", undefined, model.signals)}
 </section>
 <section class="studio-fields" data-setup-section="group">
  ${model.pids.map((pid, index) => member(pid.instance, index)).join("")}
  ${select("group.mode", "Shared Auto/Manual signal", modes)}
  ${modeWhenTrue("group.modeWhenTrue")}
  <label class="studio-field studio-group-include"><input type="checkbox" data-setup-field="group.decoupled"> The program applies a static decoupler, u = D·v in Auto</label>
  <div class="studio-fields" data-group-decoupler hidden>
   ${Array.from({ length: model.decoupler.rows }, (_, row) => Array.from({ length: model.decoupler.columns }, (_, column) => `<div data-decoupler-entry="${row}.${column}" hidden>${select(`group.decoupler.${row}.${column}`,
    `Decoupler D${row + 1}${column + 1}: plant input ${row + 1} per member ${column + 1}'s output`, model.reals)}</div>`).join("")).join("")}
  </div>
 </section>
 <section class="studio-fields" data-setup-section="onoff">
  ${select("onoff.instance", "Instance that switches the output", model.instances)}
  ${signal("onoff.processVariable", "Measurement")}
  ${select("onoff.setpoint", "Setpoint", model.signals)}
  ${signal("onoff.manipulatedVariable", "Output, switched between its minimum and maximum", undefined, model.signals)}
  <div class="studio-field-group">
   <label class="studio-field">Low output level<input type="text" placeholder="Output minimum" data-setup-field="onoff.lowOutput"></label>
   <label class="studio-field">High output level<input type="text" placeholder="Output maximum" data-setup-field="onoff.highOutput"></label>
  </div>
  ${select("onoff.mode", "Auto/Manual signal", modes)}
  ${modeWhenTrue("onoff.modeWhenTrue")}
  ${select("onoff.manualOutput", "Manual output", manualOutputs)}
  ${select("onoff.hysteresis", "Hysteresis", model.reals)}
  ${choice("onoff.bandConvention", "Hysteresis holds", band, defaults.bandConvention)}
  ${choice("onoff.thresholdEquality", "Threshold-equality policy", labelled(choices.thresholdEqualities, labels.thresholdEqualities), defaults.thresholdEquality)}
  ${select("onoff.minimumOn", "Minimum ON time (s)", model.reals)}
  ${select("onoff.minimumOff", "Minimum OFF time (s)", model.reals)}
  <label class="studio-field">Equipment minimum ON time (s)<input type="text" placeholder="Not declared" data-setup-field="onoff.equipmentMinimumOn"></label>
  <label class="studio-field">Equipment minimum OFF time (s)<input type="text" placeholder="Not declared" data-setup-field="onoff.equipmentMinimumOff"></label>
  <p class="studio-setup-ask">Zero when the equipment has no such limit. A blank answer is not zero: Tune refuses until both times are declared.</p>
  ${select("onoff.latch", "Output-on state", model.booleans)}
  <label class="studio-field">Time in current state (s)<select data-setup-field="onoff.dwellTimer">${options(model.reals, undefined, "None")}</select></label>
 </section>
 <section class="studio-fields" data-setup-section="state-feedback">
  ${choice("feedback.family", "Design", labelled(choices.stateFamilies, labels.stateFamilies), defaults.stateFamily)}
  ${select("feedback.instance", "Instance that runs the law", model.instances)}
  ${signal("feedback.manipulatedVariable", "Plant input", undefined, model.signals)}
  ${signal("feedback.processVariable", "Controlled output")}
  ${select("feedback.setpoint", "Setpoint", model.signals)}
  <div data-feedback-measured>${select("feedback.gain", "Gain on the controlled output", model.reals)}</div>
  ${select("feedback.referenceGain", "Reference gain", model.reals)}
  <div class="studio-fields" data-feedback-estimator hidden>
   <label class="studio-field">Model the estimator implements<select data-setup-field="feedback.estimatorModel"><option value="">Choose…</option>${(model.estimatorModels ?? []).map(entry =>
     `<option value="${escape(entry.file)}" data-states="${entry.states}">${escape(entry.file)}</option>`).join("")}</select></label>
   ${Array.from({ length: limits.estimatorStates }, (_, index) => `<div class="studio-fields" data-estimate-state="${index}" hidden>
    ${select(`feedback.estimate.${index}`, `Estimate of model state ${index + 1}`, model.reals)}
    ${select(`feedback.estimate.${index}.gain`, "Gain on this estimate", model.reals)}
    ${select(`feedback.estimate.${index}.observerGain`, "Observer gain of this estimate", model.reals)}
    <label class="studio-field">Process-noise variance of this state<input type="text" data-setup-field="feedback.estimate.${index}.processNoise"></label>
   </div>`).join("")}
   <label class="studio-field">Measurement-noise variance<input type="text" data-setup-field="feedback.measurementNoise"></label>
  </div>
  ${Array.from({ length: limits.furtherOutputs }, (_, index) => `<div class="studio-fields" data-feedback-state="${index}">
   ${signal(`feedback.state.${index}`, `Further measured output ${index + 1}`)}
   ${select(`feedback.state.${index}.gain`, "Gain on this output", model.reals)}
  </div>`).join("")}
  ${select("feedback.mode", "Auto/Manual signal", modes)}
  ${modeWhenTrue("feedback.modeWhenTrue")}
  ${select("feedback.manualOutput", "Manual output", manualOutputs)}
 </section>
 <section class="studio-fields" data-setup-section="own-law">
  ${select("own.instance", "Instance that computes the law", model.instances)}
  <div data-own-law-choice>${choice("own.law", "Law", labelled(choices.ownLaws, labels.ownLaws), defaults.ownLaw)}</div>
  ${signal("own.processVariable", "Measurement")}
  ${select("own.setpoint", "Setpoint", model.signals)}
  ${signal("own.manipulatedVariable", "Output", undefined, model.signals)}
  ${select("own.mode", "Auto/Manual signal", modes)}
  ${modeWhenTrue("own.modeWhenTrue")}
  ${select("own.manualOutput", "Manual output", manualOutputs)}
  ${gain("own.proportionalGain", "Proportional gain Kp")}
  <div class="studio-fields" data-own-integral>
   ${gain("own.integralGain", "Integral gain Ki")}
   ${select("own.integralTerm", "Integral term I, in the output's unit", model.reals)}
  </div>
  <div class="studio-fields" data-own-derivative hidden>
   ${gain("own.derivativeGain", "Derivative gain Kd")}
   ${select("own.derivativeState", "Previous value the derivative is taken from", model.reals)}
   <label class="studio-field">That previous value is<select data-setup-field="own.derivativeOn">${options([], undefined)}${labelled(choices.derivativeOn, labels.derivativeOn).map(([value, text]) =>
     `<option value="${escape(value)}">${escape(text)}</option>`).join("")}</select></label>
  </div>
 </section>
 <section class="studio-fields" data-setup-section="trajectory-feedforward">
  <input type="hidden" data-setup-field="traj.ownLaw" value="${escape(defaults.trajectoryOwnLaw)}">
  ${signal("traj.plannedTrajectory", "Planned trajectory the feedforward follows", undefined, model.signals)}
  ${gain("traj.rateGain", "Rate feedforward gain Kv")}
  ${gain("traj.accelerationGain", "Acceleration feedforward gain Ka")}
  ${select("traj.feedforwardTerm", "Feedforward term the program adds to its output", model.reals)}
 </section>
 <section class="studio-fields" data-setup-section="search">
  ${choice("search.family", "Law", labelled(choices.searchFamilies, labels.searchFamilies), defaults.searchFamily)}
  ${select("search.instance", "Instance that computes the law", model.instances)}
  ${signal("search.processVariable", "Measurement")}
  ${select("search.setpoint", "Setpoint", model.signals)}
  ${signal("search.manipulatedVariable", "Output", undefined, model.signals)}
  ${select("search.mode", "Auto/Manual signal", modes)}
  ${modeWhenTrue("search.modeWhenTrue")}
  ${select("search.manualOutput", "Manual output", manualOutputs)}
  ${Array.from({ length: limits.searchCoefficients }, (_, index) => `<div class="studio-fields" data-search-coefficient="${index}">
   ${select(`search.coefficient.${index}`, `Coefficient ${index + 1}`, model.reals)}
   <div class="studio-field-group">
    <label class="studio-field">Unit<input type="text" data-setup-field="search.coefficient.${index}.unit"></label>
    <label class="studio-field">Values to try<input type="text" placeholder="0.5, 1, 2" data-setup-field="search.coefficient.${index}.values"></label>
   </div>
  </div>`).join("")}
  ${Array.from({ length: limits.searchKeptValues }, (_, index) => `<div class="studio-fields" data-search-state="${index}">
   <label class="studio-field">${escape(index ? `Value kept from scan to scan ${index + 1}` : "Value kept from scan to scan")}<select data-setup-field="search.state.${index}">${options(model.reals, undefined, "None")}</select></label>
   <div class="studio-field-group">
    <label class="studio-field">Unit<input type="text" data-setup-field="search.state.${index}.unit"></label>
    ${choice(`search.state.${index}.start`, "Starts at", labelled(choices.keptStarts, labels.keptStarts), defaults.searchKeptStart)}
   </div>
  </div>`).join("")}
 </section>
 <section class="studio-fields" data-setup-section="identify">
  ${signal("identify.manipulatedVariable", "Plant input")}
  ${signal("identify.processVariable", "Measured output")}
  <label class="studio-field">Recording duration<input type="text" data-setup-field="identify.durationSeconds" placeholder="for example 15 min"></label>
 </section>
 <section class="studio-setup-summary" data-setup-startup hidden>${found("startupFlag", "Startup-done flag", select("startupFlag", "Startup-done flag", model.booleans))}</section>
 <ul hidden data-setup-prefills>${model.prefills.map(({ computed, ...prefill }) => `<li${[
  ...Object.entries(prefill), ...Object.entries(computed ?? {}).map(([key, symbol]) => [`computed-${key}`, symbol] as const)].map(([key, symbol]) =>
  ` data-${key.replace(/[A-Z]/g, letter => `-${letter.toLowerCase()}`)}="${escape(String(symbol ?? ""))}"`).join("")}></li>`).join("")}</ul>
 <ul hidden data-setup-startup-instances>${model.startupFlagInstances.map(instance => `<li>${escape(instance)}</li>`).join("")}</ul>
 <ul hidden data-setup-loop-names>${model.loopNames.map(({ instance, name }) => `<li data-instance="${escape(instance)}" data-name="${escape(name)}"></li>`).join("")}</ul>
 ${choice("action", "Controller action", labelled(choices.actions, labels.actions), defaults.action)}
 <div class="studio-actions"><button type="button" class="trust-button trust-button--primary" data-setup-save>Save loop</button><button type="button" class="trust-button studio-auxiliary" data-setup-step-by-step>Step-by-step setup…</button></div>
 <p class="studio-action-message" data-setup-message role="status"></p>
</form>`;
}
