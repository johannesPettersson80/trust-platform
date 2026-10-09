import * as assert from "assert";
import { open } from "./control-studio-loop-setup-form-harness";

const singleModel = {
  structure: "pid", addressedReal: ["P1.AppliedPowerPct", "P1.ManualOutput", "P1.SetpointC", "P1.TemperatureC"], booleans: ["P1.AutoMode"],
  records: ["P1.Parameters"], writes: [], initializes: [], pids: [{ instance: "P1.Controller", task: { name: "MainTask", intervalMs: 100 } }],
  instances: ["P1"], reals: ["P1.AppliedPowerPct", "P1.ManualOutput", "P1.SetpointC", "P1.TemperatureC"],
  prefill: { instance: "P1.Controller", parameterRecord: "P1.Parameters", manipulatedVariable: "P1.AppliedPowerPct", setpoint: "P1.SetpointC", manualOutput: "P1.ManualOutput" },
};

/** A program that switches a compressor; the switching logic is its own code, so nothing is prefilled. */
const onOffModel = { ...singleModel, structure: "identify", pids: [], prefill: undefined, prefills: [], records: [], instances: ["P1"],
  addressedReal: ["P1.CompressorPct", "P1.ManualPct", "P1.RoomC", "P1.SetpointC"], booleans: ["P1.AutoMode", "P1.CoolingOn"],
  reals: ["P1.CompressorPct", "P1.DwellS", "P1.HysteresisC", "P1.ManualPct", "P1.MinimumOffS", "P1.MinimumOnS", "P1.RoomC", "P1.SetpointC"] };
const onOffFill: [string, string][] = [["structure", "onoff"], ["name", "Cold room"], ["action", "reverse"], ["onoff.instance", "P1"],
  ["onoff.processVariable", "P1.RoomC"], ["onoff.processVariable.unit", "degC"], ["onoff.processVariable.minimum", "-10"], ["onoff.processVariable.maximum", "20"],
  ["onoff.setpoint", "P1.SetpointC"], ["onoff.manipulatedVariable", "P1.CompressorPct"], ["onoff.manipulatedVariable.unit", "%"],
  ["onoff.manipulatedVariable.minimum", "0"], ["onoff.manipulatedVariable.maximum", "100"], ["onoff.mode", "P1.AutoMode"],
  ["onoff.manualOutput", "P1.ManualPct"], ["onoff.hysteresis", "P1.HysteresisC"], ["onoff.bandConvention", "half-band"],
  ["onoff.minimumOn", "P1.MinimumOnS"], ["onoff.minimumOff", "P1.MinimumOffS"], ["onoff.latch", "P1.CoolingOn"], ["onoff.dwellTimer", "P1.DwellS"]];
const onOffDraft = { structure: "onoff", name: "Cold room", action: "reverse", instance: "P1",
  processVariable: { symbol: "P1.RoomC", unit: "degC", minimum: -10, maximum: 20 }, setpoint: { symbol: "P1.SetpointC" },
  manipulatedVariable: { symbol: "P1.CompressorPct", unit: "%", minimum: 0, maximum: 100 },
  mode: { symbol: "P1.AutoMode", whenTrue: "Auto" }, manualOutput: "P1.ManualPct", hysteresis: "P1.HysteresisC", bandConvention: "half-band",
  minimumOn: "P1.MinimumOnS", minimumOff: "P1.MinimumOffS", latch: "P1.CoolingOn", dwellTimer: "P1.DwellS" };

/** Two tanks in series whose program feeds both levels back to the pump. */
const feedbackModel = { ...onOffModel, addressedReal: ["P1.ManualM3PerS", "P1.PumpM3PerS", "P1.Tank1M", "P1.Tank2M", "P1.Tank2SetpointM"],
  booleans: ["P1.FeedbackEnabled"], reals: ["P1.K1", "P1.K2", "P1.ManualM3PerS", "P1.N", "P1.PumpM3PerS", "P1.Tank1M", "P1.Tank2M", "P1.Tank2SetpointM"] };
const feedbackFill: [string, string][] = [["structure", "state-feedback"], ["name", "Tank 2 level"], ["feedback.family", "LQR"], ["feedback.instance", "P1"],
  ["feedback.manipulatedVariable", "P1.PumpM3PerS"], ["feedback.manipulatedVariable.unit", "m3/s"], ["feedback.manipulatedVariable.minimum", "0"],
  ["feedback.manipulatedVariable.maximum", "0.02"], ["feedback.processVariable", "P1.Tank2M"], ["feedback.processVariable.unit", "m"],
  ["feedback.processVariable.minimum", "0"], ["feedback.processVariable.maximum", "0.5"], ["feedback.setpoint", "P1.Tank2SetpointM"],
  ["feedback.gain", "P1.K2"], ["feedback.referenceGain", "P1.N"],
  ["feedback.state.0", "P1.Tank1M"], ["feedback.state.0.unit", "m"], ["feedback.state.0.minimum", "0"], ["feedback.state.0.maximum", "0.5"],
  ["feedback.state.0.gain", "P1.K1"], ["feedback.mode", "P1.FeedbackEnabled"], ["feedback.manualOutput", "P1.ManualM3PerS"]];
const feedbackDraft = { structure: "state-feedback", name: "Tank 2 level", action: "direct", family: "LQR", instance: "P1",
  manipulatedVariable: { symbol: "P1.PumpM3PerS", unit: "m3/s", minimum: 0, maximum: 0.02 },
  processVariable: { symbol: "P1.Tank2M", unit: "m", minimum: 0, maximum: 0.5 }, setpoint: { symbol: "P1.Tank2SetpointM" },
  gain: "P1.K2", referenceGain: "P1.N", states: [{ signal: { symbol: "P1.Tank1M", unit: "m", minimum: 0, maximum: 0.5 }, gain: "P1.K1" }],
  mode: { symbol: "P1.FeedbackEnabled", whenTrue: "Auto" }, manualOutput: "P1.ManualM3PerS" };


suite("Control Studio loop setup form", () => {
  test("names the controller action by the direction the output moves, not by Direct or Reverse", () => {
    const view = open(singleModel);
    try {
      const options = Array.from(view.root.querySelectorAll<HTMLOptionElement>("[data-setup-field=\"action\"] option"))
        .map(option => [option.value, option.textContent]);
      // Instrument practice names these the other way round (a heater's controller is "reverse acting"), so the words would mislead.
      assert.deepStrictEqual(options, [["direct", "Increase the output while the measurement is below the setpoint"],
        ["reverse", "Increase the output while the measurement is above the setpoint"]]);
    } finally { view.close(); }
  });

  test("keeps Save unavailable with its reason until the loop is complete, then posts one confirmed draft", () => {
    const view = open(singleModel);
    try {
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      const message = view.root.querySelector<HTMLElement>("[data-setup-message]")!;
      assert.strictEqual(view.root.querySelector<HTMLFormElement>("[data-loop-setup]")!.dataset.ready, "true",
        "automation waits for the live form, so the defaults never overwrite values entered before it started");
      assert.strictEqual(view.root.querySelector<HTMLSelectElement>("[data-setup-field=\"instance\"]")!.value, "P1.Controller");
      assert.strictEqual(view.root.querySelector<HTMLSelectElement>("[data-setup-field=\"manipulatedVariable\"]")!.value, "P1.AppliedPowerPct");
      assert.strictEqual(save.disabled, true);
      // B1.3: the loop name is prefilled from the controller, so the first missing item is the measurement the code does not state.
      assert.strictEqual(view.root.querySelector<HTMLInputElement>("[data-setup-field=\"name\"]")!.value, "Controller", "the loop name follows the controller");
      assert.match(message.textContent ?? "", /choose the measurement/i, "the first missing item is named");
      view.set("name", "Heater temperature");
      view.set("processVariable", "P1.TemperatureC");
      view.set("processVariable.unit", "degC"); view.set("processVariable.minimum", "0"); view.set("processVariable.maximum", "80");
      view.set("manipulatedVariable.unit", "%"); view.set("manipulatedVariable.minimum", "0"); view.set("manipulatedVariable.maximum", "100");
      view.set("mode", "P1.AutoMode"); view.set("modeWhenTrue", "Auto");
      assert.strictEqual(view.root.querySelector("[data-setup-field=\"experimentStep\"]"), null, "Identify chooses the identification step, so setup never asks for it");
      assert.strictEqual(save.disabled, false, message.textContent ?? "");
      save.click();
      assert.deepStrictEqual(view.posts, [{ kind: "control-studio.setup.save", draft: {
        structure: "pid", name: "Heater temperature", instance: "P1.Controller", action: "direct", parameterRecord: "P1.Parameters",
        processVariable: { symbol: "P1.TemperatureC", unit: "degC", minimum: 0, maximum: 80 },
        manipulatedVariable: { symbol: "P1.AppliedPowerPct", unit: "%", minimum: 0, maximum: 100 },
        setpoint: { symbol: "P1.SetpointC" }, mode: { symbol: "P1.AutoMode", whenTrue: "Auto" }, manualOutput: "P1.ManualOutput",
      } }]);
    } finally { view.close(); }
  });

  test("asks how long to record a project-run experiment, because Studio cannot choose its length (B1.3, C12)", () => {
    const view = open(onOffModel);
    try {
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      const message = view.root.querySelector<HTMLElement>("[data-setup-message]")!;
      for (const [field, value] of [["structure", "identify"], ["name", "Cold room"], ["identify.manipulatedVariable", "P1.CompressorPct"],
        ["identify.manipulatedVariable.unit", "%"], ["identify.manipulatedVariable.minimum", "0"], ["identify.manipulatedVariable.maximum", "100"],
        ["identify.processVariable", "P1.RoomC"], ["identify.processVariable.unit", "degC"], ["identify.processVariable.minimum", "-10"],
        ["identify.processVariable.maximum", "20"]] as const) view.set(field, value);
      assert.strictEqual(save.disabled, true);
      assert.match(message.textContent ?? "", /how long to record/i, "the missing duration is named");
      view.set("identify.durationSeconds", "15 min");
      assert.strictEqual(save.disabled, false, message.textContent ?? "");
      save.click();
      assert.deepStrictEqual(view.posts, [{ kind: "control-studio.setup.save", draft: { structure: "identify", name: "Cold room",
        manipulatedVariable: { symbol: "P1.CompressorPct", unit: "%", minimum: 0, maximum: 100 },
        processVariable: { symbol: "P1.RoomC", unit: "degC", minimum: -10, maximum: 20 }, experimentDurationSeconds: 900 } }]);
    } finally { view.close(); }
  });

  test("says why no Auto/Manual signal is offered when every BOOL has an I/O address", () => {
    const view = open({ ...singleModel, modes: [] });
    try {
      for (const [field, value] of [["name", "Heater temperature"], ["processVariable", "P1.TemperatureC"], ["processVariable.unit", "degC"],
        ["processVariable.minimum", "0"], ["processVariable.maximum", "80"], ["manipulatedVariable.unit", "%"], ["manipulatedVariable.minimum", "0"],
        ["manipulatedVariable.maximum", "100"]]) view.set(field, value);
      assert.match(view.root.querySelector<HTMLElement>("[data-setup-message]")!.textContent ?? "", /Auto\/Manual signal.*without an I\/O address/);
    } finally { view.close(); }
  });

  test("requires the startup flag when the program writes its own parameters", () => {
    const view = open({ ...singleModel, booleans: ["P1.AutoMode", "P1.Initialized"], writes: ["P1.Parameters.IntegralGain"] });
    try {
      view.set("name", "Heater"); view.set("processVariable", "P1.TemperatureC");
      view.set("processVariable.unit", "degC"); view.set("processVariable.minimum", "0"); view.set("processVariable.maximum", "80");
      view.set("manipulatedVariable.unit", "%"); view.set("manipulatedVariable.minimum", "0"); view.set("manipulatedVariable.maximum", "100");
      view.set("mode", "P1.AutoMode");
      const message = view.root.querySelector("[data-setup-message]")!.textContent ?? "";
      assert.match(message, /writes P1\.Parameters\.IntegralGain.*startup/i, "the reason names the write and asks for the startup flag");
      view.set("startupFlag", "P1.Initialized");
      view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.click();
      assert.strictEqual(view.posts[0].draft.startupFlag, "P1.Initialized");
    } finally { view.close(); }
  });

  test("does not ask for a startup flag when the program writes only its controller output", () => {
    const view = open({ ...singleModel, booleans: ["P1.AutoMode", "P1.Initialized"], writes: ["P1.AppliedPowerPct", "P1.Initialized"] });
    try {
      view.set("name", "Heater"); view.set("processVariable", "P1.TemperatureC");
      view.set("processVariable.unit", "degC"); view.set("processVariable.minimum", "0"); view.set("processVariable.maximum", "80");
      view.set("manipulatedVariable.unit", "%"); view.set("manipulatedVariable.minimum", "0"); view.set("manipulatedVariable.maximum", "100");
      view.set("mode", "P1.AutoMode");
      const startup = view.root.querySelector<HTMLElement>("[data-setup-startup]");
      assert.ok(startup, "the startup-done flag has its own section, shown only when validation needs it");
      assert.strictEqual(startup!.hidden, true, "the controller writes its own output every scan");
      assert.strictEqual(view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.disabled, false,
        view.root.querySelector("[data-setup-message]")!.textContent ?? "");
    } finally { view.close(); }
  });

  test("asks a cascade for the startup flag when the program sets a value validation starts from", () => {
    const view = open({ ...singleModel, structure: "cascade", cascade: { outer: "P1.Outer", inner: "P1.Inner" }, prefill: undefined,
      pids: [{ instance: "P1.Inner" }, { instance: "P1.Outer" }], records: ["P1.InnerParameters", "P1.OuterParameters"],
      addressedReal: ["P1.HeaterPct", "P1.InnerManual", "P1.JacketC", "P1.OuterManual", "P1.VesselC", "P1.VesselSetpointC"],
      booleans: ["P1.Initialized", "P1.InnerAuto", "P1.OuterAuto"], writes: ["P1.HeaterPct", "P1.Initialized", "P1.OuterManual"], initializes: [] });
    try {
      const fill: [string, string][] = [["name", "Vessel"], ["outer.parameterRecord", "P1.OuterParameters"], ["outer.mode", "P1.OuterAuto"],
        ["outer.manualOutput", "P1.OuterManual"], ["outer.processVariable", "P1.VesselC"], ["outer.processVariable.unit", "degC"],
        ["outer.processVariable.minimum", "20"], ["outer.processVariable.maximum", "60"], ["outer.setpoint", "P1.VesselSetpointC"],
        ["inner.parameterRecord", "P1.InnerParameters"], ["inner.mode", "P1.InnerAuto"], ["inner.manualOutput", "P1.InnerManual"],
        ["inner.processVariable", "P1.JacketC"], ["inner.processVariable.unit", "degC"], ["inner.processVariable.minimum", "20"],
        ["inner.processVariable.maximum", "80"], ["inner.manipulatedVariable", "P1.HeaterPct"], ["inner.manipulatedVariable.unit", "%"],
        ["inner.manipulatedVariable.minimum", "0"], ["inner.manipulatedVariable.maximum", "100"], ];
      for (const [field, value] of fill) view.set(field, value);
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      assert.strictEqual(save.disabled, true);
      assert.strictEqual(view.root.querySelector<HTMLElement>("[data-setup-startup]")!.hidden, false);
      assert.match(view.root.querySelector("[data-setup-message]")!.textContent ?? "", /writes P1\.OuterManual.*startup/i,
        "the outer manual output is the inner setpoint validation starts from");
      view.set("startupFlag", "P1.Initialized");
      assert.strictEqual(save.disabled, false, view.root.querySelector("[data-setup-message]")!.textContent ?? "");
      save.click();
      assert.strictEqual(view.posts[0].draft.startupFlag, "P1.Initialized");
      // Identify chooses the identification step and duration within the limits, so the cascade draft carries neither (Specification 35 B1.3).
      assert.ok(!("experimentStep" in view.posts[0].draft) && !("experimentDurationSeconds" in view.posts[0].draft));
    } finally { view.close(); }
  });

  test("fills in the chosen controller's own signals when the program has several PIDs, keeping what the user changed", () => {
    const left = { instance: "P1.Left", parameterRecord: "P1.LeftParameters", manipulatedVariable: "P1.LeftHeater", setpoint: "P1.LeftSetpoint", manualOutput: "P1.LeftManual" };
    const right = { instance: "P1.Right", parameterRecord: "P1.RightParameters", manipulatedVariable: "P1.RightHeater", setpoint: "P1.RightSetpoint", manualOutput: "P1.RightManual" };
    const view = open({ ...singleModel, structure: "group", prefill: undefined, prefills: [left, right],
      pids: [{ instance: "P1.Left" }, { instance: "P1.Right" }], records: ["P1.LeftParameters", "P1.RightParameters"],
      addressedReal: ["P1.LeftHeater", "P1.LeftManual", "P1.LeftSetpoint", "P1.RightHeater", "P1.RightManual", "P1.RightSetpoint"] });
    try {
      const field = (name: string) => view.root.querySelector<HTMLSelectElement>(`[data-setup-field="${name}"]`)!.value;
      assert.strictEqual(field("structure"), "pid", "independent PIDs are set up one loop at a time");
      view.set("instance", "P1.Right");
      assert.deepStrictEqual(["parameterRecord", "manipulatedVariable", "setpoint", "manualOutput"].map(field),
        ["P1.RightParameters", "P1.RightHeater", "P1.RightSetpoint", "P1.RightManual"]);
      view.set("manualOutput", "P1.LeftManual");
      view.set("instance", "P1.Left");
      assert.deepStrictEqual(["parameterRecord", "manipulatedVariable", "setpoint", "manualOutput"].map(field),
        ["P1.LeftParameters", "P1.LeftHeater", "P1.LeftSetpoint", "P1.LeftManual"], "a changed controller brings its own signals");
      view.set("manualOutput", "P1.RightManual");
      view.set("instance", "P1.Right");
      assert.strictEqual(field("manualOutput"), "P1.RightManual", "a field the user chose is kept");
    } finally { view.close(); }
  });

  test("fills in both controllers of a recognised cascade from their own calls", () => {
    const view = open({ ...singleModel, structure: "cascade", cascade: { outer: "P1.Outer", inner: "P1.Inner" }, prefill: undefined,
      pids: [{ instance: "P1.Inner" }, { instance: "P1.Outer" }], records: ["P1.InnerParameters", "P1.OuterParameters"],
      addressedReal: ["P1.HeaterPct", "P1.InnerManual", "P1.JacketC", "P1.JacketSetpointC", "P1.OuterManual", "P1.VesselC", "P1.VesselSetpointC"],
      prefills: [
        { instance: "P1.Inner", parameterRecord: "P1.InnerParameters", manipulatedVariable: "P1.HeaterPct", setpoint: "P1.JacketSetpointC", processValue: "P1.JacketC", manualOutput: "P1.InnerManual" },
        { instance: "P1.Outer", parameterRecord: "P1.OuterParameters", manipulatedVariable: "P1.JacketSetpointC", setpoint: "P1.VesselSetpointC", processValue: "P1.VesselC", manualOutput: "P1.OuterManual" }] });
    try {
      const field = (name: string) => view.root.querySelector<HTMLSelectElement>(`[data-setup-field="${name}"]`)!.value;
      assert.deepStrictEqual(["outer.parameterRecord", "outer.manualOutput", "outer.processVariable", "outer.setpoint",
        "inner.parameterRecord", "inner.manualOutput", "inner.processVariable", "inner.manipulatedVariable"].map(field),
      ["P1.OuterParameters", "P1.OuterManual", "P1.VesselC", "P1.VesselSetpointC", "P1.InnerParameters", "P1.InnerManual", "P1.JacketC", "P1.HeaterPct"]);
    } finally { view.close(); }
  });

  test("says why the controller's own measurement is not offered and how to map it", () => {
    const view = open({ ...singleModel, prefill: { ...singleModel.prefill, computed: { processValue: "P1.SafeProcessValueC" } },
      prefills: [{ ...singleModel.prefill, computed: { processValue: "P1.SafeProcessValueC" } }] });
    try {
      view.set("name", "Heater");
      assert.match(view.root.querySelector("[data-setup-message]")!.textContent ?? "",
        /Choose the measurement\. The controller reads P1\.SafeProcessValueC, which the program computes.*overwritten every scan; choose the measurement it is computed from/,
        "the missing item names the program's own variable and why it cannot serve");
    } finally { view.close(); }
  });

  test("offers a controller output, setpoint and manual output that have no I/O address (Specification 35 §17.1)", () => {
    const view = open({ ...singleModel, signals: [...singleModel.addressedReal, "P1.ControllerPct", "P1.OperatorSetpointC"].sort(),
      measurements: ["P1.TemperatureC"] });
    try {
      const options = (field: string) => Array.from(view.root.querySelectorAll<HTMLOptionElement>(`[data-setup-field="${field}"] option`)).map(option => option.value);
      assert.ok(options("manipulatedVariable").includes("P1.ControllerPct"), `${options("manipulatedVariable")}`);
      assert.ok(options("setpoint").includes("P1.OperatorSetpointC"));
      assert.deepStrictEqual(options("processVariable").filter(Boolean), ["P1.TemperatureC"]);
      assert.ok(options("manualOutput").includes("P1.ControllerPct"), "Studio steps a manual output by name, so it needs no I/O address");
    } finally { view.close(); }
  });

  test("sets up interacting PIDs as one group that shares the Auto/Manual signal", () => {
    const prefill = (n: number) => ({ instance: `P1.Zone${n}`, parameterRecord: `P1.Zone${n}Parameters`, manipulatedVariable: `P1.Heater${n}Pct`,
      setpoint: `P1.Zone${n}SetpointC`, processValue: `P1.Zone${n}C`, manualOutput: `P1.Zone${n}ManualPct` });
    const view = open({ ...singleModel, structure: "group", prefill: undefined, prefills: [prefill(1), prefill(2)],
      pids: [{ instance: "P1.Zone1" }, { instance: "P1.Zone2" }], records: ["P1.Zone1Parameters", "P1.Zone2Parameters"], booleans: ["P1.ZonesAuto"],
      addressedReal: [1, 2].flatMap(n => [`P1.Heater${n}Pct`, `P1.Zone${n}C`, `P1.Zone${n}ManualPct`, `P1.Zone${n}SetpointC`]).sort() });
    try {
      const structures = Array.from(view.root.querySelectorAll<HTMLOptionElement>('[data-setup-field="structure"] option')).map(option => option.value);
      assert.ok(structures.includes("group"), `a program with several PIDs offers the group structure: ${structures}`);
      view.set("structure", "group");
      view.set("name", "Zone temperatures");
      const include = (n: number) => { const box = view.root.querySelector<HTMLInputElement>(`[data-setup-field="group.${n}.include"]`)!;
        box.checked = true; box.dispatchEvent(new view.dom.window.Event("change", { bubbles: true })); };
      include(0); include(1);
      const field = (name: string) => view.root.querySelector<HTMLSelectElement>(`[data-setup-field="${name}"]`)!.value;
      assert.deepStrictEqual(["group.1.parameterRecord", "group.1.processVariable", "group.1.manipulatedVariable", "group.1.setpoint", "group.1.manualOutput"].map(field),
        ["P1.Zone2Parameters", "P1.Zone2C", "P1.Heater2Pct", "P1.Zone2SetpointC", "P1.Zone2ManualPct"], "each member is filled in from its own calls");
      for (const n of [0, 1]) {
        view.set(`group.${n}.processVariable.unit`, "degC"); view.set(`group.${n}.processVariable.minimum`, "20"); view.set(`group.${n}.processVariable.maximum`, "60");
        view.set(`group.${n}.manipulatedVariable.unit`, "%"); view.set(`group.${n}.manipulatedVariable.minimum`, "0"); view.set(`group.${n}.manipulatedVariable.maximum`, "100");
      }
      assert.strictEqual(view.root.querySelector('[data-setup-field="group.experimentStep"]'), null, "no member asks for an identification step");
      assert.strictEqual(view.root.querySelector('[data-setup-field="group.experimentDuration"]'), null, "the group asks for no identification duration");
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      assert.match(view.root.querySelector("[data-setup-message]")!.textContent ?? "", /Auto\/Manual signal.*share/i);
      view.set("group.mode", "P1.ZonesAuto");
      assert.strictEqual(save.disabled, false, view.root.querySelector("[data-setup-message]")!.textContent ?? "");
      save.click();
      const draft = view.posts[0].draft;
      assert.deepStrictEqual([draft.structure, draft.mode, draft.members.map((entry: any) => entry.instance)],
        ["group", { symbol: "P1.ZonesAuto", whenTrue: "Auto" }, ["P1.Zone1", "P1.Zone2"]]);
      assert.deepStrictEqual(draft.members[1], { instance: "P1.Zone2", parameterRecord: "P1.Zone2Parameters",
        processVariable: { symbol: "P1.Zone2C", unit: "degC", minimum: 20, maximum: 60 }, manipulatedVariable: { symbol: "P1.Heater2Pct", unit: "%", minimum: 0, maximum: 100 },
        setpoint: { symbol: "P1.Zone2SetpointC" }, manualOutput: "P1.Zone2ManualPct" });
      assert.ok(!("experimentStep" in draft.members[0]) && !("experimentStep" in draft), "no identification step in the group draft");
    } finally { view.close(); }
  });

  test("sets up an existing switching controller with its band, dwell times and retained switching state", () => {
    const view = open(onOffModel);
    try {
      const structures = Array.from(view.root.querySelectorAll<HTMLOptionElement>('[data-setup-field="structure"] option')).map(option => option.value);
      assert.ok(structures.includes("onoff"), `a switching controller has its own structure: ${structures}`);
      assert.doesNotMatch(view.root.querySelector("[data-setup-step-by-step]")!.textContent ?? "", /ON\/OFF/,
        "the secondary setup no longer offers what this form sets up");
      for (const [field, value] of onOffFill) view.set(field, value);
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      assert.strictEqual(save.disabled, false, view.root.querySelector("[data-setup-message]")!.textContent ?? "");
      save.click();
      assert.deepStrictEqual(view.posts[0].draft, onOffDraft);
    } finally { view.close(); }
  });

  test("takes the switched equipment's own minimum ON and OFF times, and none when they are blank", () => {
    const view = open(onOffModel);
    try {
      for (const [field, value] of [...onOffFill, ["onoff.equipmentMinimumOn", "-1"]]) view.set(field, value);
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      assert.strictEqual(save.disabled, true);
      assert.match(view.root.querySelector("[data-setup-message]")!.textContent ?? "", /equipment minimum ON time/i);
      view.set("onoff.equipmentMinimumOn", "20"); view.set("onoff.equipmentMinimumOff", "30");
      save.click();
      assert.deepStrictEqual(view.posts[0].draft, { ...onOffDraft, equipmentMinimumOnSeconds: 20, equipmentMinimumOffSeconds: 30 });
    } finally { view.close(); }
  });

  test("requires the startup flag when the program writes a switching setting itself", () => {
    const view = open({ ...onOffModel, booleans: [...onOffModel.booleans, "P1.Initialized"], writes: ["P1.CompressorPct", "P1.CoolingOn", "P1.MinimumOnS"] });
    try {
      for (const [field, value] of onOffFill) view.set(field, value);
      assert.match(view.root.querySelector("[data-setup-message]")!.textContent ?? "", /writes P1\.MinimumOnS.*startup/i,
        "a program that sets its own minimum ON time would replace the candidate being validated");
      view.set("startupFlag", "P1.Initialized");
      view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.click();
      assert.deepStrictEqual(view.posts[0].draft, { ...onOffDraft, startupFlag: "P1.Initialized" });
    } finally { view.close(); }
  });

  test("sets up an existing state-feedback law with one gain per measured output", () => {
    const view = open(feedbackModel);
    try {
      const structures = Array.from(view.root.querySelectorAll<HTMLOptionElement>('[data-setup-field="structure"] option')).map(option => option.value);
      assert.ok(structures.includes("state-feedback"), `state feedback has its own structure: ${structures}`);
      for (const [field, value] of feedbackFill.slice(0, 15)) view.set(field, value);
      const message = () => view.root.querySelector("[data-setup-message]")!.textContent ?? "";
      assert.match(message(), /further measured output/i, "state feedback needs the other states it feeds back");
      for (const [field, value] of feedbackFill.slice(15, 19)) view.set(field, value);
      assert.match(message(), /gain on P1\.Tank1M/i);
      for (const [field, value] of feedbackFill.slice(19)) view.set(field, value);
      assert.strictEqual(view.root.querySelector('[data-setup-field="feedback.experimentStep"]'), null, "setup asks for no identification step");
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      assert.strictEqual(save.disabled, false, message());
      save.click();
      assert.deepStrictEqual(view.posts[0].draft, feedbackDraft);
    } finally { view.close(); }
  });

  test("requires the startup flag when the program writes a state-feedback gain itself", () => {
    const view = open({ ...feedbackModel, booleans: [...feedbackModel.booleans, "P1.Initialized"], writes: ["P1.N", "P1.PumpM3PerS"] });
    try {
      for (const [field, value] of feedbackFill) view.set(field, value);
      assert.match(view.root.querySelector("[data-setup-message]")!.textContent ?? "", /writes P1\.N.*startup/i,
        "a program that sets its own reference gain would replace the candidate being validated");
      view.set("startupFlag", "P1.Initialized");
      view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.click();
      assert.deepStrictEqual(view.posts[0].draft, { ...feedbackDraft, startupFlag: "P1.Initialized" });
    } finally { view.close(); }
  });

  test("rejects an empty range before posting", () => {
    const view = open(singleModel);
    try {
      view.set("name", "Heater"); view.set("processVariable", "P1.TemperatureC");
      view.set("processVariable.unit", "degC"); view.set("processVariable.minimum", "80"); view.set("processVariable.maximum", "80");
      view.set("manipulatedVariable.unit", "%"); view.set("manipulatedVariable.minimum", "0"); view.set("manipulatedVariable.maximum", "100");
      view.set("mode", "P1.AutoMode");
      assert.strictEqual(view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.disabled, true);
      assert.match(view.root.querySelector("[data-setup-message]")!.textContent ?? "", /maximum.*above.*minimum/i);
    } finally { view.close(); }
  });

  test("shows the recognised cascade with its outer and inner controllers", () => {
    const view = open({ ...singleModel, structure: "cascade", cascade: { outer: "P1.Outer", inner: "P1.Inner" },
      pids: [{ instance: "P1.Inner" }, { instance: "P1.Outer" }], prefill: undefined, records: ["P1.InnerParameters", "P1.OuterParameters"] });
    try {
      assert.strictEqual(view.root.querySelector<HTMLSelectElement>("[data-setup-field=\"structure\"]")!.value, "cascade");
      assert.strictEqual(view.root.querySelector<HTMLSelectElement>("[data-setup-field=\"outer.instance\"]")!.value, "P1.Outer");
      assert.strictEqual(view.root.querySelector<HTMLSelectElement>("[data-setup-field=\"inner.instance\"]")!.value, "P1.Inner");
      assert.strictEqual(view.root.querySelector<HTMLElement>("[data-setup-section=\"pid\"]")!.hidden, true, "only the chosen structure's fields are shown");
      assert.strictEqual(view.root.querySelector<HTMLElement>("[data-setup-section=\"cascade\"]")!.hidden, false);
    } finally { view.close(); }
  });
});
