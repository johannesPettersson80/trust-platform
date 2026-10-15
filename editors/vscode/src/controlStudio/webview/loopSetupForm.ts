import { MAXIMUM_EXPERIMENT_DURATION_MS, MINIMUM_EXPERIMENT_DURATION_MS } from "../controlStudioLimits";
import { parseDurationSeconds } from "./durationField";

/** The one-page loop setup form: shows the chosen structure's fields, names the first missing item and posts one draft. */
export function initializeLoopSetupForm(root: HTMLElement, post: (message: unknown) => void): { showError(reason: string): void } {
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

  const signal = (field: string, label: string): { value?: { symbol: string; unit: string; minimum: number; maximum: number }; missing?: string } => {
    const symbol = value(field), unit = value(`${field}.unit`);
    const minimum = value(`${field}.minimum`), maximum = value(`${field}.maximum`);
    if (!symbol) return { missing: `Choose the ${label}.` };
    if (!unit) return { missing: `Enter the ${label} unit.` };
    if (!minimum || !maximum || !Number.isFinite(Number(minimum)) || !Number.isFinite(Number(maximum))) return { missing: `Enter the ${label} minimum and maximum.` };
    if (Number(maximum) <= Number(minimum)) return { missing: `The ${label} maximum must be above its minimum.` };
    return { value: { symbol, unit, minimum: Number(minimum), maximum: Number(maximum) } };
  };
  const controller = (prefix: string, label: string) => {
    const entries = [["instance", `${label} controller`], ["parameterRecord", `${label} parameter record`], ["mode", `${label} Auto/Manual signal`], ["manualOutput", `${label} manual output`]];
    const missing = entries.find(([field]) => !value(`${prefix}${field}`));
    return missing ? { missing: `Choose the ${missing[1].trim()}.` } : { value: {
      instance: value(`${prefix}instance`), parameterRecord: value(`${prefix}parameterRecord`),
      mode: { symbol: value(`${prefix}mode`), whenTrue: value(`${prefix}modeWhenTrue`) || "Auto" }, manualOutput: value(`${prefix}manualOutput`) } };
  };

  /** The draft for the chosen structure, or the first missing item in reading order. */
  const draft = (): { draft?: Record<string, unknown>; missing?: string } => {
    const name = value("name"), action = value("action") || "direct";
    if (!name) return { missing: "Enter a loop name." };
    if (structure() === "identify") {
      const input = signal("identify.manipulatedVariable", "plant input"); if (input.missing) return input;
      const output = signal("identify.processVariable", "measured output"); if (output.missing) return output;
      // The project runs this experiment, so Studio cannot choose its length: the engineer says how long to record it (C12).
      const seconds = parseDurationSeconds(value("identify.durationSeconds"));
      if (seconds === undefined || seconds < MINIMUM_EXPERIMENT_DURATION_MS / 1_000 || seconds > MAXIMUM_EXPERIMENT_DURATION_MS / 1_000) {
        return { missing: "Enter how long to record the project's experiment, from 1 s to 24 h (for example 15 min)." };
      }
      return { draft: { structure: "identify", name, manipulatedVariable: input.value, processVariable: output.value, experimentDurationSeconds: seconds } };
    }
    if (structure() === "state-feedback") {
      if (!value("feedback.instance")) return { missing: "Choose the instance that runs the state-feedback law." };
      const mv = signal("feedback.manipulatedVariable", "plant input"); if (mv.missing) return mv;
      const pv = signal("feedback.processVariable", "controlled output"); if (pv.missing) return pv;
      if (!value("feedback.setpoint")) return { missing: "Choose the setpoint." };
      if (value("feedback.family") === "LQG") return lqgDraft(name, action, mv.value!, pv.value!);
      if (!value("feedback.gain")) return { missing: `Choose the variable holding the gain on ${pv.value!.symbol}.` };
      if (!value("feedback.referenceGain")) return { missing: "Choose the variable holding the reference gain." };
      const states: Record<string, unknown>[] = [];
      for (const prefix of ["feedback.state.0", "feedback.state.1", "feedback.state.2"].filter(prefix => value(prefix))) {
        const state = signal(prefix, `measured output ${value(prefix)}`); if (state.missing) return state;
        if (!value(`${prefix}.gain`)) return { missing: `Choose the variable holding the gain on ${value(prefix)}.` };
        states.push({ signal: state.value, gain: value(`${prefix}.gain`) });
      }
      if (!states.length) return { missing: "Choose at least one further measured output: state feedback needs every state it feeds back." };
      if (!value("feedback.mode")) return { missing: "Choose the Auto/Manual signal." };
      if (!value("feedback.manualOutput")) return { missing: "Choose the manual output." };
      const startup = startupFlag(); if (startup.missing) return startup;
      return { draft: { structure: "state-feedback", name, action, ...(startup.value ? { startupFlag: startup.value } : {}),
        family: value("feedback.family") || "LQR", instance: value("feedback.instance"),
        manipulatedVariable: mv.value, processVariable: pv.value, setpoint: { symbol: value("feedback.setpoint") }, gain: value("feedback.gain"),
        referenceGain: value("feedback.referenceGain"), states, mode: { symbol: value("feedback.mode"), whenTrue: value("feedback.modeWhenTrue") || "Auto" },
        manualOutput: value("feedback.manualOutput") } };
    }
    if (structure() === "own-law") {
      const law = ownLawDraft(name, action, ownLaw()); if (!law.draft) return law;
      return { draft: { structure: "own-law", ...law.draft } };
    }
    if (structure() === "trajectory-feedforward") {
      // The program's PID law, with the planned trajectory it follows, the feedforward term it adds and the two feedforward gains (C11).
      const law = ownLawDraft(name, action, "PID"); if (!law.draft) return law;
      const rateGain = gainValue("traj.rateGain", "rate feedforward gain"); if (rateGain.missing) return rateGain;
      const accelerationGain = gainValue("traj.accelerationGain", "acceleration feedforward gain"); if (accelerationGain.missing) return accelerationGain;
      const planned = signal("traj.plannedTrajectory", "planned trajectory"); if (planned.missing) return planned;
      if (!value("traj.feedforwardTerm")) return { missing: "Choose the variable holding the feedforward term." };
      return { draft: { structure: "trajectory-feedforward", ownLaw: law.draft, rateGain: rateGain.value, accelerationGain: accelerationGain.value,
        plannedTrajectory: planned.value, feedforwardTerm: value("traj.feedforwardTerm") } };
    }
    if (structure() === "search") {
      if (!value("search.instance")) return { missing: "Choose the instance that computes the law." };
      const pv = signal("search.processVariable", "measurement"); if (pv.missing) return pv;
      if (!value("search.setpoint")) return { missing: "Choose the setpoint." };
      const mv = signal("search.manipulatedVariable", "output"); if (mv.missing) return mv;
      if (!value("search.mode")) return { missing: "Choose the Auto/Manual signal." };
      if (!value("search.manualOutput")) return { missing: "Choose the manual output." };
      const coefficients: { symbol: string; unit: string; values: number[] }[] = [];
      for (const index of [0, 1, 2, 3].filter(index => value(`search.coefficient.${index}`))) {
        const symbol = value(`search.coefficient.${index}`), unit = value(`search.coefficient.${index}.unit`);
        if (!unit) return { missing: `Enter the unit of ${symbol}.` };
        const values = value(`search.coefficient.${index}.values`).split(/[,;\s]+/).filter(Boolean).map(Number);
        if (!values.length || values.some(entry => !Number.isFinite(entry))) return { missing: `Enter the values of ${symbol} Tune may try, separated by commas.` };
        coefficients.push({ symbol, unit, values });
      }
      if (!coefficients.length) return { missing: "Choose at least one coefficient Tune may search." };
      const combinations = coefficients.reduce((count, coefficient) => count * coefficient.values.length, 1);
      if (combinations > 64) return { missing: `Tune tries at most 64 combinations; these values make ${combinations}.` };
      const states: { symbol: string; unit: string; start: string }[] = [];
      for (const index of [0, 1, 2].filter(index => value(`search.state.${index}`))) {
        const symbol = value(`search.state.${index}`), unit = value(`search.state.${index}.unit`);
        if (!unit) return { missing: `Enter the unit of ${symbol}.` };
        states.push({ symbol, unit, start: value(`search.state.${index}.start`) || "zero" });
      }
      const startup = startupFlag(); if (startup.missing) return startup;
      return { draft: { structure: "search", name, action, ...(startup.value ? { startupFlag: startup.value } : {}), family: value("search.family") || "MPC",
        instance: value("search.instance"), processVariable: pv.value, setpoint: { symbol: value("search.setpoint") }, manipulatedVariable: mv.value,
        mode: { symbol: value("search.mode"), whenTrue: value("search.modeWhenTrue") || "Auto" }, manualOutput: value("search.manualOutput"),
        coefficients, states } };
    }
    if (structure() === "onoff") {
      if (!value("onoff.instance")) return { missing: "Choose the instance that switches the output." };
      const pv = signal("onoff.processVariable", "measurement"); if (pv.missing) return pv;
      if (!value("onoff.setpoint")) return { missing: "Choose the setpoint." };
      const mv = signal("onoff.manipulatedVariable", "output"); if (mv.missing) return mv;
      const choices = [["mode", "the Auto/Manual signal"], ["manualOutput", "the manual output"], ["hysteresis", "the hysteresis"],
        ["minimumOn", "the minimum ON time"], ["minimumOff", "the minimum OFF time"], ["latch", "the BOOL that holds whether the output is on"]];
      const unset = choices.find(([field]) => !value(`onoff.${field}`)); if (unset) return { missing: `Choose ${unset[1]}.` };
      const minimumOn = equipmentSeconds("onoff.equipmentMinimumOn", "equipment minimum ON time"); if (minimumOn.missing) return minimumOn;
      const minimumOff = equipmentSeconds("onoff.equipmentMinimumOff", "equipment minimum OFF time"); if (minimumOff.missing) return minimumOff;
      const startup = startupFlag(); if (startup.missing) return startup;
      return { draft: { structure: "onoff", name, action, ...(startup.value ? { startupFlag: startup.value } : {}), instance: value("onoff.instance"), processVariable: pv.value,
        setpoint: { symbol: value("onoff.setpoint") }, manipulatedVariable: mv.value, mode: { symbol: value("onoff.mode"), whenTrue: value("onoff.modeWhenTrue") || "Auto" },
        manualOutput: value("onoff.manualOutput"), hysteresis: value("onoff.hysteresis"), bandConvention: value("onoff.bandConvention") || "half-band",
        minimumOn: value("onoff.minimumOn"), minimumOff: value("onoff.minimumOff"), latch: value("onoff.latch"),
        ...(value("onoff.dwellTimer") ? { dwellTimer: value("onoff.dwellTimer") } : {}),
        ...(minimumOn.value !== undefined ? { equipmentMinimumOnSeconds: minimumOn.value } : {}),
        ...(minimumOff.value !== undefined ? { equipmentMinimumOffSeconds: minimumOff.value } : {}) } };
    }
    if (structure() === "group") {
      const prefixes = included();
      if (prefixes.length < 2) return { missing: "Include at least two controllers in the group." };
      const members: Record<string, unknown>[] = [];
      for (const prefix of prefixes) {
        const instance = value(`${prefix}instance`);
        if (!value(`${prefix}parameterRecord`)) return { missing: `Choose the parameter record of ${instance}.` };
        const pv = signal(`${prefix}processVariable`, `${instance} measurement`); if (pv.missing) return hinted(pv, prefix, "processVariable");
        const mv = signal(`${prefix}manipulatedVariable`, `${instance} output`); if (mv.missing) return hinted(mv, prefix, "manipulatedVariable");
        if (!value(`${prefix}setpoint`)) return hinted({ missing: `Choose the setpoint of ${instance}.` }, prefix, "setpoint");
        if (!value(`${prefix}manualOutput`)) return hinted({ missing: `Choose the manual output of ${instance}.` }, prefix, "manualOutput");
        members.push({ instance, parameterRecord: value(`${prefix}parameterRecord`), processVariable: pv.value, manipulatedVariable: mv.value,
          setpoint: { symbol: value(`${prefix}setpoint`) }, manualOutput: value(`${prefix}manualOutput`) });
      }
      if (!value("group.mode")) return { missing: "Choose the Auto/Manual signal the members share." };
      // The program's static decoupler, one variable per plant input and member (Specification 35 §17.15).
      const decoupler: string[][] = [];
      if (checked("group.decoupled")) for (const member of members) {
        // Behind a decoupler the controller's own output is v; the plant input u = D·v is what the member drives.
        const own = prefills.get(String(member.instance))?.manipulatedVariable, output = (member.manipulatedVariable as { symbol?: string } | undefined)?.symbol;
        if (own && output === own) return { missing: `Behind a decoupler, ${member.instance}'s output is the plant input the program writes from D·v, `
          + `not the controller's own output ${own}. Choose the plant input.` };
      }
      if (checked("group.decoupled")) for (let row = 0; row < members.length; row++) {
        decoupler.push([]);
        for (let column = 0; column < members.length; column++) {
          const symbol = value(`group.decoupler.${row}.${column}`);
          if (!symbol) return { missing: `Choose the decoupler variable D${row + 1}${column + 1}: plant input ${row + 1} per member ${column + 1}'s output.` };
          decoupler[row].push(symbol);
        }
      }
      const startup = startupFlag(); if (startup.missing) return startup;
      return { draft: { structure: "group", name, action, ...(startup.value ? { startupFlag: startup.value } : {}),
        mode: { symbol: value("group.mode"), whenTrue: value("group.modeWhenTrue") || "Auto" },
        members, ...(decoupler.length ? { decoupler } : {}) } };
    }
    if (structure() === "cascade") {
      const outer = controller("outer.", "outer"); if (outer.missing) return hinted(outer, "outer.", "manualOutput");
      const outerPv = signal("outer.processVariable", "outer measurement"); if (outerPv.missing) return hinted(outerPv, "outer.", "processVariable");
      if (!value("outer.setpoint")) return hinted({ missing: "Choose the outer setpoint." }, "outer.", "setpoint");
      const inner = controller("inner.", "inner"); if (inner.missing) return hinted(inner, "inner.", "manualOutput");
      if (value("inner.instance") === value("outer.instance")) return { missing: "Choose two different controllers." };
      const innerPv = signal("inner.processVariable", "inner measurement"); if (innerPv.missing) return hinted(innerPv, "inner.", "processVariable");
      const output = signal("inner.manipulatedVariable", "output"); if (output.missing) return hinted(output, "inner.", "manipulatedVariable");
      // One identification of the plant input for both loops (Specification 35 §17.11).
      const startup = startupFlag(); if (startup.missing) return startup;
      return { draft: { structure: "cascade", name, action, ...(startup.value ? { startupFlag: startup.value } : {}),
        outer: { ...outer.value, processVariable: outerPv.value, setpoint: { symbol: value("outer.setpoint") } },
        inner: { ...inner.value, processVariable: innerPv.value, manipulatedVariable: output.value } } };
    }
    const loop = singleLoop(name, action); if (loop.missing) return loop;
    if (structure() === "schedule") {
      // Two to four operating points, each with the program's gain variables for it (Specification 35 §17.14).
      const points: Record<string, string>[] = [];
      for (let index = 0; index < 4; index++) {
        const entry = { name: value(`schedule.${index}.name`), proportionalGain: value(`schedule.${index}.proportionalGain`), integralGain: value(`schedule.${index}.integralGain`) };
        if (!entry.name && !entry.proportionalGain && !entry.integralGain) continue;
        const absent = ([["name", "name"], ["proportionalGain", "proportional gain variable"], ["integralGain", "integral gain variable"]] as const).find(([key]) => !entry[key]);
        if (absent) return { missing: `Enter operating point ${index + 1}'s ${absent[1]}.` };
        points.push(entry);
      }
      if (points.length < 2) return { missing: "Enter at least two operating points, each with its name and gain variables." };
      return { draft: { ...loop.draft, structure: "schedule", points } };
    }
    if (structure() === "smith") {
      // The program's predictor model and the correction it keeps (Specification 35 §17.13).
      const fields = [["smith.gain", "model gain"], ["smith.timeConstant", "model time constant"], ["smith.delay", "model delay"],
        ["smith.correction", "correction the program keeps"]] as const;
      const absent = fields.find(([field]) => !value(field));
      if (absent) return { missing: `Choose the variable holding the ${absent[1]}.` };
      return { draft: { ...loop.draft, structure: "smith", model: { gain: value("smith.gain"), timeConstant: value("smith.timeConstant"),
        delay: value("smith.delay"), correction: value("smith.correction") } } };
    }
    if (structure() !== "feedforward") return loop;
    // The measured disturbance and the program's feedforward variables (Specification 35 §17.12). The draft is the engine's
    // FeedforwardLoopDraft (`parse_setup_draft` in setup/save.rs): the step is in the disturbance's unit, and the engine checks its range.
    const disturbance = signal("ff.disturbance", "measured disturbance"); if (disturbance.missing) return disturbance;
    if (!value("ff.disturbanceCommand")) return { missing: "Choose the variable Identify steps to move the disturbance." };
    const step = Number(value("ff.disturbanceStep"));
    if (!value("ff.disturbanceStep") || !Number.isFinite(step)) return { missing: "Enter the disturbance's identification step, in its unit." };
    if (!value("ff.gain")) return { missing: "Choose the variable holding the feedforward gain." };
    const leadLag = [["ff.lead", "lead time"], ["ff.lag", "lag time"], ["ff.filtered", "filtered disturbance the lag keeps"]] as const;
    const absent = leadLag.find(([field]) => !value(field));
    if (absent && leadLag.some(([field]) => value(field))) return { missing: `Choose the ${absent[1]} too, or no lead/lag at all.` };
    return { draft: { ...loop.draft, structure: "feedforward", disturbance: disturbance.value, disturbanceCommand: value("ff.disturbanceCommand"),
      disturbanceStep: step, feedforward: { gain: value("ff.gain"),
        ...(absent ? {} : { lead: value("ff.lead"), lag: value("ff.lag"), filtered: value("ff.filtered") }) } } };
  };

  /** One library PID's fields, shared by the single loop and the feedforward structure. */
  const singleLoop = (name: string, action: string): { draft?: Record<string, unknown>; missing?: string } => {
    if (!value("instance")) return { missing: "Choose the controller." };
    if (!value("parameterRecord")) return { missing: "Choose the parameter record." };
    const pv = signal("processVariable", "measurement"); if (pv.missing) return hinted(pv, "", "processVariable");
    const mv = signal("manipulatedVariable", "output"); if (mv.missing) return hinted(mv, "", "manipulatedVariable");
    if (!value("setpoint")) return hinted({ missing: "Choose the setpoint." }, "", "setpoint");
    if (!value("mode")) return { missing: "Choose the Auto/Manual signal." };
    if (!value("manualOutput")) return hinted({ missing: "Choose the manual output." }, "", "manualOutput");
    const startup = startupFlag(); if (startup.missing) return startup;
    return { draft: { structure: "pid", name, ...(startup.value ? { startupFlag: startup.value } : {}), instance: value("instance"), action, parameterRecord: value("parameterRecord"),
      processVariable: pv.value, manipulatedVariable: mv.value, setpoint: { symbol: value("setpoint") },
      mode: { symbol: value("mode"), whenTrue: value("modeWhenTrue") || "Auto" }, manualOutput: value("manualOutput") } };
  };

  /** LQG: the program's estimates, their gains and observer gains for each state of the estimator's model (§17.1). */
  const estimatorStates = (): number => Number(root.querySelector<HTMLSelectElement>('[data-setup-field="feedback.estimatorModel"]')?.selectedOptions[0]?.dataset.states ?? 0);
  const lqgDraft = (name: string, action: string, mv: Record<string, unknown>, pv: Record<string, unknown>): { draft?: Record<string, unknown>; missing?: string } => {
    if (!value("feedback.referenceGain")) return { missing: "Choose the variable holding the reference gain." };
    if (!value("feedback.estimatorModel")) return { missing: "Choose the model file the program's estimator implements." };
    const states: Record<string, unknown>[] = [];
    for (let index = 0; index < estimatorStates(); index++) {
      const prefix = `feedback.estimate.${index}`;
      const absent = [["", "estimate"], [".gain", "gain"], [".observerGain", "observer gain"]].find(([suffix]) => !value(`${prefix}${suffix}`));
      if (absent) return { missing: `Choose the variable holding the ${absent[1]} of model state ${index + 1}.` };
      const noise = Number(value(`${prefix}.processNoise`));
      if (!value(`${prefix}.processNoise`) || !(noise > 0)) return { missing: `Enter the positive process-noise variance of model state ${index + 1}.` };
      states.push({ estimate: value(prefix), gain: value(`${prefix}.gain`), observerGain: value(`${prefix}.observerGain`), processNoise: noise });
    }
    const measurementNoise = Number(value("feedback.measurementNoise"));
    if (!value("feedback.measurementNoise") || !(measurementNoise > 0)) return { missing: "Enter the positive measurement-noise variance." };
    if (!value("feedback.mode")) return { missing: "Choose the Auto/Manual signal." };
    if (!value("feedback.manualOutput")) return { missing: "Choose the manual output." };
    const startup = startupFlag(); if (startup.missing) return startup;
    return { draft: { structure: "state-feedback", name, action, ...(startup.value ? { startupFlag: startup.value } : {}), family: "LQG",
      instance: value("feedback.instance"), manipulatedVariable: mv, processVariable: pv, setpoint: { symbol: value("feedback.setpoint") },
      referenceGain: value("feedback.referenceGain"), mode: { symbol: value("feedback.mode"), whenTrue: value("feedback.modeWhenTrue") || "Auto" },
      manualOutput: value("feedback.manualOutput"),
      estimator: { modelFile: value("feedback.estimatorModel"), measurementNoise, states } } };
  };
  // A trajectory feedforward's program law is PID (C11); the other own-law setups take the chosen law.
  const ownLaw = (): string => structure() === "trajectory-feedforward" ? "PID" : value("own.law") || "PI";
  /** A program's own P, PI or PID law, with its variables, gain ranges and startup flag, as the own-law and trajectory setups ask for them. */
  const ownLawDraft = (name: string, action: string, law: string): { draft?: Record<string, unknown>; missing?: string } => {
    if (!value("own.instance")) return { missing: "Choose the instance that computes the law." };
    const pv = signal("own.processVariable", "measurement"); if (pv.missing) return pv;
    if (!value("own.setpoint")) return { missing: "Choose the setpoint." };
    const mv = signal("own.manipulatedVariable", "output"); if (mv.missing) return mv;
    if (!value("own.mode")) return { missing: "Choose the Auto/Manual signal." };
    if (!value("own.manualOutput")) return { missing: "Choose the manual output." };
    const integrating = law !== "P", differentiating = law === "PID";
    const proportional = gainValue("own.proportionalGain", "proportional gain"); if (proportional.missing) return proportional;
    const integral = integrating ? gainValue("own.integralGain", "integral gain") : {}; if (integral.missing) return integral;
    if (integrating && !value("own.integralTerm")) return { missing: "Choose the variable holding the integral term." };
    const derivative = differentiating ? gainValue("own.derivativeGain", "derivative gain") : {}; if (derivative.missing) return derivative;
    if (differentiating && !value("own.derivativeState")) return { missing: "Choose the variable holding the previous value the derivative is taken from." };
    if (differentiating && !value("own.derivativeOn")) return { missing: "Choose whether that previous value is the error or the measurement." };
    const startup = startupFlag(); if (startup.missing) return startup;
    return { draft: { name, action, ...(startup.value ? { startupFlag: startup.value } : {}), law,
      instance: value("own.instance"), processVariable: pv.value, setpoint: { symbol: value("own.setpoint") }, manipulatedVariable: mv.value,
      mode: { symbol: value("own.mode"), whenTrue: value("own.modeWhenTrue") || "Auto" }, manualOutput: value("own.manualOutput"),
      proportionalGain: proportional.value, ...(integrating ? { integralGain: integral.value, integralTerm: value("own.integralTerm") } : {}),
      ...(differentiating ? { derivativeGain: derivative.value, derivativeState: value("own.derivativeState"), derivativeOn: value("own.derivativeOn") } : {}) } };
  };
  /** A gain variable of the program's own law with the range Tune may choose from. */
  const gainValue = (field: string, label: string): { value?: { symbol: string; minimum: number; maximum: number }; missing?: string } => {
    const symbol = value(field), minimum = value(`${field}.minimum`), maximum = value(`${field}.maximum`);
    if (!symbol) return { missing: `Choose the variable holding the ${label}.` };
    if (!minimum || !maximum || !Number.isFinite(Number(minimum)) || !Number.isFinite(Number(maximum))) return { missing: `Enter the ${label} minimum and maximum.` };
    if (Number(maximum) <= Number(minimum)) return { missing: `The ${label} maximum must be above its minimum.` };
    return { value: { symbol, minimum: Number(minimum), maximum: Number(maximum) } };
  };
  /** Validation starts the program from seeded values: the setpoint, manual outputs, modes, parameter records and PID states.
   * Code that writes one of them itself, typically a startup branch, would replace it on the first scan. */
  const listed = (kind: string) => new Set(Array.from(root.querySelectorAll(`[data-setup-writes] li[data-kind="${kind}"]`)).map(item => item.textContent ?? ""));
  const writes = listed("write"), initializes = listed("initialize");
  const included = (): string[] => Array.from(root.querySelectorAll<HTMLInputElement>('[data-setup-field^="group."][data-setup-field$=".include"]'))
    .filter(box => box.checked).map(box => (box.dataset.setupField ?? "").replace(/include$/, ""));
  const seededWrite = (): string | undefined => {
    const cascade = structure() === "cascade", group = structure() === "group", onoff = structure() === "onoff", feedback = structure() === "state-feedback";
    const own = structure() === "own-law" || structure() === "trajectory-feedforward", search = structure() === "search";
    if (structure() === "identify") return undefined;
    const prefixes = cascade ? ["outer.", "inner."] : group ? included() : onoff ? ["onoff."] : feedback ? ["feedback."] : own ? ["own."] : search ? ["search."] : [""];
    const instance = prefixes.map(prefix => value(`${prefix}instance`)).find(name => initializes.has(name));
    if (instance) return `The program initializes ${instance} in its own code.`;
    // A switching controller's settings are seeded like a parameter record; its latch and dwell timer are its own running state,
    // as the integral term is of a law the program computes itself.
    const symbols = [...(cascade ? [value("outer.setpoint")] : group ? [value("group.mode"), ...prefixes.map(prefix => value(`${prefix}setpoint`))]
      : onoff ? ["setpoint", "hysteresis", "minimumOn", "minimumOff"].map(field => value(`onoff.${field}`))
      : feedback ? ["setpoint", "gain", "referenceGain", "state.0.gain", "state.1.gain", "state.2.gain"].map(field => value(`feedback.${field}`))
      : own ? ["setpoint", "proportionalGain", ...(ownLaw() !== "P" ? ["integralGain"] : []), ...(ownLaw() === "PID" ? ["derivativeGain"] : [])]
        .map(field => value(`own.${field}`))
      : search ? [value("search.setpoint"), ...[0, 1, 2, 3].map(index => value(`search.coefficient.${index}`))] : [value("setpoint")]),
      ...prefixes.flatMap(prefix => [value(`${prefix}manualOutput`), value(`${prefix}mode`)])].filter(Boolean);
    // A gain schedule's program writes the PID's record from its table on every scan, by design.
    const records = structure() === "schedule" ? [] : prefixes.map(prefix => value(`${prefix}parameterRecord`)).filter(Boolean);
    const written = symbols.find(symbol => writes.has(symbol)) ?? [...writes].find(symbol => records.some(record => symbol.startsWith(`${record}.`)));
    return written ? `The program writes ${written} in its own code.` : undefined;
  };
  const startupFlag = (): { value?: string; missing?: string } => {
    const reason = seededWrite();
    if (!reason) return {};
    const flag = value("startupFlag");
    return flag ? { value: flag } : { missing: `${reason} Choose the flag that marks its startup as done, so validation keeps the values it starts from.` };
  };
  /** An equipment limit such as a compressor's restart protection; blank when the equipment has none. */
  const equipmentSeconds = (field: string, label: string): { value?: number; missing?: string } => {
    const raw = value(field), seconds = Number(raw);
    if (!raw) return {};
    return Number.isFinite(seconds) && seconds >= 0 ? { value: seconds }
      : { missing: `The ${label} must be zero or more seconds; leave it blank when the equipment has none.` };
  };
  /** When a field is still empty because the controller's own variable cannot serve, say why and what to choose. */
  const hinted = <T extends { missing?: string }>(state: T, prefix: string, field: string): T => {
    if (value(`${prefix}${field}`)) return state;
    const prefill = prefills.get(value(`${prefix}instance`));
    const computed = field === "processVariable" ? prefill?.computedProcessValue : undefined;
    if (computed) return { ...state, missing: `${state.missing} The controller reads ${computed}, which the program computes, so a simulated `
      + "measurement would be overwritten every scan; choose the measurement it is computed from." };
    return state;
  };
  /** A chosen controller fills in what its own calls say, except fields the user has chosen. */
  const prefills = new Map(Array.from(root.querySelectorAll<HTMLElement>("[data-setup-prefills] li")).map(item => [item.dataset.instance ?? "", item.dataset]));
  // The Auto/Manual signal, what its TRUE means and the startup-done flag come from the code too (Specification 35 §17.1).
  const prefilled: Record<string, readonly [string, string][]> = {
    "": [["parameterRecord", "parameterRecord"], ["manipulatedVariable", "manipulatedVariable"], ["setpoint", "setpoint"], ["processVariable", "processValue"], ["manualOutput", "manualOutput"],
      ["mode", "mode"], ["modeWhenTrue", "modeWhenTrue"], ["startupFlag", "startupFlag"]],
    "outer.": [["outer.parameterRecord", "parameterRecord"], ["outer.manualOutput", "manualOutput"], ["outer.processVariable", "processValue"], ["outer.setpoint", "setpoint"],
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
      const text = field === "mode" ? (chosenText("mode") ? `${chosenText("mode")} · TRUE means ${chosenText("modeWhenTrue") || "Auto"}` : "")
        : field === "task" ? tasks.get(value("instance")) ?? ""
        : chosenText(field);
      element.textContent = text || (field === "task" ? "Not stated in the code" : "Not found in the code");
    }
  };
  const nameField = fields.get("name") as HTMLInputElement | undefined;
  if (nameField) nameField.addEventListener("input", () => { nameField.dataset.auto = "false"; });
  const prefillName = (): void => {
    if (!nameField || nameField.dataset.auto === "false" || !value("instance")) return;
    nameField.value = value("instance").split(".").pop() ?? value("instance");
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

  const render = (): void => {
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
    // A further operating point appears once the previous one has a name.
    root.querySelectorAll<HTMLElement>("[data-schedule-point]").forEach(row => {
      const index = Number(row.dataset.schedulePoint);
      row.hidden = index > 1 && !value(`schedule.${index - 1}.name`) && !value(`schedule.${index}.name`);
    });
    const startup = root.querySelector<HTMLElement>("[data-setup-startup]");
    if (startup) startup.hidden = seededWrite() === undefined;
    const state = draft();
    save.disabled = !state.draft;
    message.textContent = explained(state.missing ?? "");
  };
  /** An empty Auto/Manual list means every BOOL has an I/O address; say why, since Apply switches the mode directly. */
  const explained = (missing: string): string => /Auto\/Manual signal/.test(missing)
    && !Array.from(root.querySelectorAll<HTMLSelectElement>('select[data-setup-field$="mode"]')).some(select => Array.from(select.options).some(option => option.value))
    ? `${missing} It must be a BOOL without an I/O address, because Apply switches it directly; declare one without AT.` : missing;
  form.addEventListener("input", render);
  form.addEventListener("change", render);
  root.querySelector("[data-setup-step-by-step]")?.addEventListener("click", () => post({ kind: "control-studio.setup.step-by-step" }));
  save.addEventListener("click", () => {
    const state = draft();
    if (state.draft) post({ kind: "control-studio.setup.save", draft: state.draft });
  });
  render();
  form.dataset.ready = "true";
  // A refused save keeps the form and its values; the reason replaces the completeness hint.
  return { showError: reason => { message.textContent = reason; } };
}
