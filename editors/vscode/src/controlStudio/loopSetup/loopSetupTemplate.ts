import type { SetupFormModel } from "./setupFormModel";

/** The body of the one-page loop setup form. Choices come only from the compiled model; nothing is guessed. */
export function renderLoopSetupBody(model: SetupFormModel, escape: (value: string) => string): string {
  const options = (values: readonly string[], selected?: string, empty = "Choose…") =>
    `<option value="">${escape(empty)}</option>${values.map(value => `<option value="${escape(value)}"${value === selected ? " selected" : ""}>${escape(value)}</option>`).join("")}`;
  const select = (field: string, label: string, values: readonly string[], selected?: string) =>
    `<label class="studio-field">${escape(label)}<select data-setup-field="${field}">${options(values, selected)}</select></label>`;
  const choice = (field: string, label: string, values: readonly [string, string][], selected: string) =>
    `<label class="studio-field">${escape(label)}<select data-setup-field="${field}">${values.map(([value, text]) =>
      `<option value="${value}"${value === selected ? " selected" : ""}>${escape(text)}</option>`).join("")}</select></label>`;
  /** A loop signal with its unit and range: measurements from what a simulated plant can write, outputs from every signal. */
  const signal = (field: string, label: string, selected?: string, options: readonly string[] = model.measurements ?? model.addressedReal) =>
    `${select(field, label, options, selected)}
  <div class="studio-field-group">
   <label class="studio-field">Unit<input type="text" data-setup-field="${field}.unit"></label>
   <label class="studio-field">Minimum<input type="number" step="any" data-setup-field="${field}.minimum"></label>
   <label class="studio-field">Maximum<input type="number" step="any" data-setup-field="${field}.maximum"></label>
  </div>`;
  // The step the identification experiment applies to the output, in the output's own unit, and how long it runs.
  const step = (field: string, label = "Identification step") =>
    `<label class="studio-field">${escape(label)}<input type="number" step="any" min="0" data-setup-field="${field}"></label>`;
  const duration = (field: string, label = "Identification duration (s)") =>
    `<label class="studio-field">${escape(label)}<input type="number" step="any" min="2" max="600" data-setup-field="${field}"></label>`;
  /** A variable holding a gain of the program's own law, with the range Tune may choose from; its unit follows the signals'. */
  const gain = (field: string, label: string) => `${select(field, label, model.reals)}
  <div class="studio-field-group">
   <label class="studio-field">Minimum<input type="number" step="any" data-setup-field="${field}.minimum"></label>
   <label class="studio-field">Maximum<input type="number" step="any" data-setup-field="${field}.maximum"></label>
  </div>`;
  const pidNames = model.pids.map(pid => pid.instance);
  // Apply switches the Auto/Manual signal directly, so only a BOOL without an I/O address can be one.
  const modes = model.modes ?? model.booleans;
  const controller = (prefix: string, label: string, selected?: string) => `
  ${select(`${prefix}instance`, label, pidNames, selected)}
  ${select(`${prefix}parameterRecord`, "Parameter record", model.records)}
  ${select(`${prefix}mode`, "Auto/Manual signal", modes)}
  ${choice(`${prefix}modeWhenTrue`, "TRUE means", [["Auto", "Auto"], ["Manual", "Manual"]], "Auto")}
  ${select(`${prefix}manualOutput`, "Manual output", model.addressedReal)}`;
  const prefill = model.prefill;
  const structures: [string, string][] = [["pid", "One PID loop"], ["cascade", "Cascade: one PID sets the other's setpoint"],
    ["feedforward", "Feedforward: the PID adds the program's feedforward from a measured disturbance"],
    ["smith", "Smith predictor: the PID reads the measurement plus the program's predicted correction"],
    ["schedule", "Gain schedule: the program interpolates the PID's gains between operating points"],
    ...(model.pids.length > 1 ? [["group", "PID group: interacting PIDs tuned together"] as [string, string]] : []),
    ...(model.instances.length ? [["onoff", "ON/OFF: the program switches the output"], ["state-feedback", "State feedback: the program feeds measured outputs back"],
      ["own-law", "Own P, PI or PID: the program computes the output from its gain variables"],
      ["search", "Other law: Tune searches the program's coefficient values"]] as [string, string][] : []),
    ["identify", "Identify only, no controller"]];
  const band: [string, string][] = [["half-band", "Half the band: switches at setpoint ± hysteresis"],
    ["full-band", "The whole band: switches at setpoint ± hysteresis / 2"]];
  // One block per compiled PID; its own calls fill it in once included.
  const member = (instance: string, index: number) => `<fieldset class="studio-group-member" data-group-member>
   <legend><label class="studio-group-include"><input type="checkbox" data-setup-field="group.${index}.include"> ${escape(instance)}</label></legend>
   <input type="hidden" data-setup-field="group.${index}.instance" value="${escape(instance)}">
   <div class="studio-fields" data-group-member-fields hidden>
    ${select(`group.${index}.parameterRecord`, "Parameter record", model.records)}
    ${signal(`group.${index}.processVariable`, "Measurement")}
    ${signal(`group.${index}.manipulatedVariable`, "Output", undefined, model.signals ?? model.addressedReal)}
    ${select(`group.${index}.setpoint`, "Setpoint", model.signals ?? model.addressedReal)}
    ${select(`group.${index}.manualOutput`, "Manual output", model.addressedReal)}
    ${step(`group.${index}.experimentStep`)}
   </div>
  </fieldset>`;
  return `<form class="studio-fields" data-loop-setup novalidate>
 <label class="studio-field">Loop name<input type="text" data-setup-field="name"></label>
 ${choice("structure", "Structure", structures, model.structure === "cascade" ? "cascade" : model.structure === "identify" ? "identify" : "pid")}
 <section class="studio-fields" data-setup-section="pid">
  ${select("instance", "Controller", pidNames, prefill?.instance)}
  ${select("parameterRecord", "Parameter record", model.records, prefill?.parameterRecord)}
  ${signal("processVariable", "Measurement", prefill?.processValue)}
  ${signal("manipulatedVariable", "Output", prefill?.manipulatedVariable, model.signals ?? model.addressedReal)}
  ${step("experimentStep")}
  ${duration("experimentDuration")}
  ${select("setpoint", "Setpoint", model.signals ?? model.addressedReal, prefill?.setpoint)}
  ${select("mode", "Auto/Manual signal", modes)}
  ${choice("modeWhenTrue", "TRUE means", [["Auto", "Auto"], ["Manual", "Manual"]], "Auto")}
  ${select("manualOutput", "Manual output", model.addressedReal, prefill?.manualOutput)}
 </section>
 <section class="studio-fields" data-setup-section="feedforward">
  ${signal("ff.disturbance", "Measured disturbance", undefined, model.signals ?? model.addressedReal)}
  ${select("ff.disturbanceCommand", "Variable Identify steps to move the disturbance", model.addressedReal)}
  ${step("ff.disturbanceStep", "Disturbance identification step")}
  ${select("ff.gain", "Feedforward gain", model.reals)}
  <label class="studio-field">Lead time (s)<select data-setup-field="ff.lead">${options(model.reals, undefined, "None: no lead/lag")}</select></label>
  <label class="studio-field">Lag time (s)<select data-setup-field="ff.lag">${options(model.reals, undefined, "None")}</select></label>
  <label class="studio-field">Filtered disturbance the lag keeps<select data-setup-field="ff.filtered">${options(model.reals, undefined, "None")}</select></label>
 </section>
 <section class="studio-fields" data-setup-section="schedule">
  ${[0, 1, 2, 3].map(index => `<div class="studio-fields" data-schedule-point="${index}">
   <label class="studio-field">Operating point ${index + 1}<input type="text" data-setup-field="schedule.${index}.name"></label>
   ${select(`schedule.${index}.proportionalGain`, "Its proportional gain variable", model.reals)}
   ${select(`schedule.${index}.integralGain`, "Its integral gain variable", model.reals)}
  </div>`).join("")}
 </section>
 <section class="studio-fields" data-setup-section="smith">
  ${select("smith.gain", "Model gain", model.reals)}
  ${select("smith.timeConstant", "Model time constant (s)", model.reals)}
  ${select("smith.delay", "Model delay (s)", model.reals)}
  ${select("smith.correction", "Correction the program keeps", model.reals)}
 </section>
 <section class="studio-fields" data-setup-section="cascade">
  ${controller("outer.", "Outer controller", model.cascade?.outer)}
  ${signal("outer.processVariable", "Outer measurement")}
  ${select("outer.setpoint", "Outer setpoint", model.signals ?? model.addressedReal)}
  ${controller("inner.", "Inner controller", model.cascade?.inner)}
  ${signal("inner.processVariable", "Inner measurement")}
  ${signal("inner.manipulatedVariable", "Output", undefined, model.signals ?? model.addressedReal)}
  ${step("cascade.experimentStep")}
  ${duration("cascade.experimentDuration")}
 </section>
 <section class="studio-fields" data-setup-section="group">
  ${model.pids.map((pid, index) => member(pid.instance, index)).join("")}
  ${select("group.mode", "Shared Auto/Manual signal", modes)}
  ${choice("group.modeWhenTrue", "TRUE means", [["Auto", "Auto"], ["Manual", "Manual"]], "Auto")}
  ${duration("group.experimentDuration")}
  <label class="studio-field studio-group-include"><input type="checkbox" data-setup-field="group.decoupled"> The program applies a static decoupler, u = D·v in Auto</label>
  <div class="studio-fields" data-group-decoupler hidden>
   ${[0, 1, 2, 3].flatMap(row => [0, 1, 2, 3].map(column => `<div data-decoupler-entry="${row}.${column}" hidden>${select(`group.decoupler.${row}.${column}`,
    `Decoupler D${row + 1}${column + 1}: plant input ${row + 1} per member ${column + 1}'s output`, model.reals)}</div>`)).join("")}
  </div>
 </section>
 <section class="studio-fields" data-setup-section="onoff">
  ${select("onoff.instance", "Instance that switches the output", model.instances)}
  ${signal("onoff.processVariable", "Measurement")}
  ${select("onoff.setpoint", "Setpoint", model.signals ?? model.addressedReal)}
  ${signal("onoff.manipulatedVariable", "Output, switched between its minimum and maximum", undefined, model.signals ?? model.addressedReal)}
  ${select("onoff.mode", "Auto/Manual signal", modes)}
  ${choice("onoff.modeWhenTrue", "TRUE means", [["Auto", "Auto"], ["Manual", "Manual"]], "Auto")}
  ${select("onoff.manualOutput", "Manual output", model.addressedReal)}
  ${select("onoff.hysteresis", "Hysteresis", model.reals)}
  ${choice("onoff.bandConvention", "Hysteresis holds", band, "half-band")}
  ${select("onoff.minimumOn", "Minimum ON time (s)", model.reals)}
  ${select("onoff.minimumOff", "Minimum OFF time (s)", model.reals)}
  <label class="studio-field">Equipment minimum ON time (s)<input type="number" step="any" min="0" placeholder="None" data-setup-field="onoff.equipmentMinimumOn"></label>
  <label class="studio-field">Equipment minimum OFF time (s)<input type="number" step="any" min="0" placeholder="None" data-setup-field="onoff.equipmentMinimumOff"></label>
  ${select("onoff.latch", "Output-on state", model.booleans)}
  <label class="studio-field">Time in current state (s)<select data-setup-field="onoff.dwellTimer">${options(model.reals, undefined, "None")}</select></label>
  ${duration("onoff.experimentDuration")}
 </section>
 <section class="studio-fields" data-setup-section="state-feedback">
  ${choice("feedback.family", "Design", [["LQR", "LQR, weighted by the signal ranges"], ["state-feedback", "Pole placement"],
    ["LQG", "LQG: the program estimates the states from the controlled output"]], "LQR")}
  ${select("feedback.instance", "Instance that runs the law", model.instances)}
  ${signal("feedback.manipulatedVariable", "Plant input", undefined, model.signals ?? model.addressedReal)}
  ${signal("feedback.processVariable", "Controlled output")}
  ${select("feedback.setpoint", "Setpoint", model.signals ?? model.addressedReal)}
  <div data-feedback-measured>${select("feedback.gain", "Gain on the controlled output", model.reals)}</div>
  ${select("feedback.referenceGain", "Reference gain", model.reals)}
  <div class="studio-fields" data-feedback-estimator hidden>
   <label class="studio-field">Model the estimator implements<select data-setup-field="feedback.estimatorModel"><option value="">Choose…</option>${(model.estimatorModels ?? []).map(entry =>
     `<option value="${escape(entry.file)}" data-states="${entry.states}">${escape(entry.file)}</option>`).join("")}</select></label>
   ${[0, 1, 2].map(index => `<div class="studio-fields" data-estimate-state="${index}" hidden>
    ${select(`feedback.estimate.${index}`, `Estimate of model state ${index + 1}`, model.reals)}
    ${select(`feedback.estimate.${index}.gain`, "Gain on this estimate", model.reals)}
    ${select(`feedback.estimate.${index}.observerGain`, "Observer gain of this estimate", model.reals)}
    <label class="studio-field">Process-noise variance of this state<input type="number" step="any" min="0" data-setup-field="feedback.estimate.${index}.processNoise"></label>
   </div>`).join("")}
   <label class="studio-field">Measurement-noise variance<input type="number" step="any" min="0" data-setup-field="feedback.measurementNoise"></label>
  </div>
  ${[0, 1, 2].map(index => `<div class="studio-fields" data-feedback-state="${index}">
   ${signal(`feedback.state.${index}`, `Further measured output ${index + 1}`)}
   ${select(`feedback.state.${index}.gain`, "Gain on this output", model.reals)}
  </div>`).join("")}
  ${select("feedback.mode", "Auto/Manual signal", modes)}
  ${choice("feedback.modeWhenTrue", "TRUE means", [["Auto", "Auto"], ["Manual", "Manual"]], "Auto")}
  ${select("feedback.manualOutput", "Manual output", model.addressedReal)}
  ${step("feedback.experimentStep")}
  ${duration("feedback.experimentDuration")}
 </section>
 <section class="studio-fields" data-setup-section="own-law">
  ${select("own.instance", "Instance that computes the law", model.instances)}
  ${choice("own.law", "Law", [["PI", "PI: u = Kp·e + I, where I integrates Ki·e"], ["P", "P: u = Kp·e plus the program's own offset"],
    ["PID", "PID: u = Kp·e + I + D, where D is Kd times the rate of the error or measurement"]], "PI")}
  ${signal("own.processVariable", "Measurement")}
  ${select("own.setpoint", "Setpoint", model.signals ?? model.addressedReal)}
  ${signal("own.manipulatedVariable", "Output", undefined, model.signals ?? model.addressedReal)}
  ${select("own.mode", "Auto/Manual signal", modes)}
  ${choice("own.modeWhenTrue", "TRUE means", [["Auto", "Auto"], ["Manual", "Manual"]], "Auto")}
  ${select("own.manualOutput", "Manual output", model.addressedReal)}
  ${gain("own.proportionalGain", "Proportional gain Kp")}
  <div class="studio-fields" data-own-integral>
   ${gain("own.integralGain", "Integral gain Ki")}
   ${select("own.integralTerm", "Integral term I, in the output's unit", model.reals)}
  </div>
  <div class="studio-fields" data-own-derivative hidden>
   ${gain("own.derivativeGain", "Derivative gain Kd")}
   ${select("own.derivativeState", "Previous value the derivative is taken from", model.reals)}
   <label class="studio-field">That previous value is<select data-setup-field="own.derivativeOn">${options([], undefined)}<option value="error">${escape("The error")}</option><option value="measurement">${escape("The measurement")}</option></select></label>
  </div>
  ${step("own.experimentStep")}
  ${duration("own.experimentDuration")}
 </section>
 <section class="studio-fields" data-setup-section="search">
  ${choice("search.family", "Law", [["MPC", "Model predictive control"], ["ratio", "Ratio"], ["split-range", "Split range"], ["override", "Override"],
    ["gain-scheduled", "Gain schedule"], ["feedforward", "Feedforward"], ["Smith", "Smith predictor"], ["lead-lag", "Lead/lag"], ["adaptive", "Adaptive"],
    ["robust", "Robust"], ["feedback-linearization", "Feedback linearization"], ["observer-feedback", "Observer feedback"]], "MPC")}
  ${select("search.instance", "Instance that computes the law", model.instances)}
  ${signal("search.processVariable", "Measurement")}
  ${select("search.setpoint", "Setpoint", model.signals ?? model.addressedReal)}
  ${signal("search.manipulatedVariable", "Output", undefined, model.signals ?? model.addressedReal)}
  ${select("search.mode", "Auto/Manual signal", modes)}
  ${choice("search.modeWhenTrue", "TRUE means", [["Auto", "Auto"], ["Manual", "Manual"]], "Auto")}
  ${select("search.manualOutput", "Manual output", model.addressedReal)}
  ${[0, 1, 2, 3].map(index => `<div class="studio-fields" data-search-coefficient="${index}">
   ${select(`search.coefficient.${index}`, `Coefficient ${index + 1}`, model.reals)}
   <div class="studio-field-group">
    <label class="studio-field">Unit<input type="text" data-setup-field="search.coefficient.${index}.unit"></label>
    <label class="studio-field">Values to try<input type="text" placeholder="0.5, 1, 2" data-setup-field="search.coefficient.${index}.values"></label>
   </div>
  </div>`).join("")}
  ${[0, 1, 2].map(index => `<div class="studio-fields" data-search-state="${index}">
   <label class="studio-field">${escape(index ? `Value kept from scan to scan ${index + 1}` : "Value kept from scan to scan")}<select data-setup-field="search.state.${index}">${options(model.reals, undefined, "None")}</select></label>
   <div class="studio-field-group">
    <label class="studio-field">Unit<input type="text" data-setup-field="search.state.${index}.unit"></label>
    <label class="studio-field">Starts at<select data-setup-field="search.state.${index}.start"><option value="zero">Zero</option><option value="output">The output</option><option value="measurement">The measurement</option></select></label>
   </div>
  </div>`).join("")}
  ${step("search.experimentStep")}
  ${duration("search.experimentDuration")}
 </section>
 <section class="studio-fields" data-setup-section="identify">
  ${signal("identify.manipulatedVariable", "Plant input")}
  ${signal("identify.processVariable", "Measured output")}
  ${duration("identify.experimentDuration", "Recording duration (s)")}
 </section>
 <section class="studio-fields" data-setup-startup hidden>${select("startupFlag", "Startup-done flag", model.booleans)}</section>
 <ul hidden data-setup-prefills>${(model.prefills ?? []).map(({ unaddressed, computed, ...prefill }) => `<li${[
  ...Object.entries(prefill), ...Object.entries(unaddressed ?? {}).map(([key, symbol]) => [`unaddressed-${key}`, symbol] as const),
  ...Object.entries(computed ?? {}).map(([key, symbol]) => [`computed-${key}`, symbol] as const)].map(([key, symbol]) =>
  ` data-${key.replace(/[A-Z]/g, letter => `-${letter.toLowerCase()}`)}="${escape(symbol ?? "")}"`).join("")}></li>`).join("")}</ul>
 <ul hidden data-setup-writes>${model.writes.map(symbol => `<li data-kind="write">${escape(symbol)}</li>`).join("")}${
  model.initializes.map(instance => `<li data-kind="initialize">${escape(instance)}</li>`).join("")}</ul>
 ${choice("action", "Controller action", [["direct", "Increase the output while the measurement is below the setpoint"], ["reverse", "Increase the output while the measurement is above the setpoint"]], "direct")}
 <div class="studio-actions"><button type="button" class="trust-button trust-button--primary" data-setup-save disabled>Save loop</button><button type="button" class="trust-button studio-auxiliary" data-setup-step-by-step>Step-by-step setup…</button></div>
 <p class="studio-action-message" data-setup-message role="status"></p>
</form>`;
}
