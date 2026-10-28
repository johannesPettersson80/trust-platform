import * as assert from "assert";
import { open } from "./control-studio-loop-setup-form-harness";

/** The oven's compiled facts as the setup form sees them: the program computes its own PI. */
const ownLawSignals = ["P1.HeaterPct", "P1.ManualPct", "P1.OvenC", "P1.SetpointC"];
const ownLawModel = { structure: "identify", pids: [], prefills: [], loopNames: [], records: [], initializes: [], instances: ["P1"],
  addressedReal: ownLawSignals, measurements: ownLawSignals, signals: ownLawSignals, booleans: ["P1.AutoMode", "P1.Started"], modes: ["P1.AutoMode", "P1.Started"],
  reals: ["P1.HeaterPct", "P1.Integral", "P1.Ki", "P1.Kp", "P1.ManualPct", "P1.OvenC", "P1.SetpointC"], writes: ["P1.HeaterPct", "P1.Integral"] };
const ownLawFill: [string, string][] = [["structure", "own-law"], ["name", "Oven"], ["action", "direct"], ["own.instance", "P1"], ["own.law", "PI"],
  ["own.processVariable", "P1.OvenC"], ["own.processVariable.unit", "degC"], ["own.processVariable.minimum", "0"], ["own.processVariable.maximum", "300"],
  ["own.setpoint", "P1.SetpointC"], ["own.manipulatedVariable", "P1.HeaterPct"], ["own.manipulatedVariable.unit", "%"],
  ["own.manipulatedVariable.minimum", "0"], ["own.manipulatedVariable.maximum", "100"], ["own.mode", "P1.AutoMode"], ["own.manualOutput", "P1.ManualPct"],
  ["own.proportionalGain", "P1.Kp"], ["own.proportionalGain.minimum", "0"], ["own.proportionalGain.maximum", "20"],
  ["own.integralGain", "P1.Ki"], ["own.integralGain.minimum", "0"], ["own.integralGain.maximum", "2"]];
/** The PI draft as the form posts it (B1.3): every number as the text typed, and the derivative fields of the hidden PID section as
 * well, blank here; the engine takes the fields the law uses (`setup_draft_ignores_gains_the_law_does_not_take`). */
const ownLawDraft = { structure: "own-law", name: "Oven", action: "direct", law: "PI", instance: "P1",
  processVariable: { symbol: "P1.OvenC", unit: "degC", minimum: "0", maximum: "300" }, setpoint: { symbol: "P1.SetpointC" },
  manipulatedVariable: { symbol: "P1.HeaterPct", unit: "%", minimum: "0", maximum: "100" },
  mode: { symbol: "P1.AutoMode", whenTrue: "Auto" }, manualOutput: "P1.ManualPct",
  proportionalGain: { symbol: "P1.Kp", minimum: "0", maximum: "20" }, integralGain: { symbol: "P1.Ki", minimum: "0", maximum: "2" },
  integralTerm: "P1.Integral", derivativeGain: { symbol: "", minimum: "", maximum: "" }, derivativeState: null, derivativeOn: null };

suite("Control Studio own P or PI law setup form", () => {
  test("sets up a P or PI law the program computes itself from its own gain variables", () => {
    const view = open(ownLawModel);
    try {
      const structures = Array.from(view.root.querySelectorAll<HTMLOptionElement>('[data-setup-field="structure"] option')).map(option => option.value);
      assert.ok(structures.includes("own-law"), `a law the program computes itself has its own structure: ${structures}`);
      for (const [field, value] of ownLawFill) view.set(field, value);
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      save.click();
      assert.strictEqual(view.posts[0].draft.integralTerm, null, "a PI law without its integral term posts it as null, for the engine to name");
      view.set("own.integralTerm", "P1.Integral");
      assert.strictEqual(view.root.querySelector('[data-setup-field="own.experimentStep"]'), null, "setup asks for no identification step");
      save.click();
      assert.deepStrictEqual(view.posts[1].draft, ownLawDraft);
    } finally { view.close(); }
  });

  test("asks a PID law for its derivative gain, the previous value it differentiates and whether that is the error", () => {
    const view = open({ ...ownLawModel, reals: [...ownLawModel.reals, "P1.Kd", "P1.PreviousErrorC"].sort() });
    try {
      const laws = Array.from(view.root.querySelectorAll<HTMLOptionElement>('[data-setup-field="own.law"] option')).map(option => option.value);
      assert.ok(laws.includes("PID") && view.root.querySelector("[data-own-derivative]"), `the law choice offers PID with its derivative fields: ${laws}`);
      for (const [field, value] of [...ownLawFill, ["own.law", "PID"], ["own.integralTerm", "P1.Integral"]]) view.set(field, value);
      assert.strictEqual(view.root.querySelector<HTMLElement>("[data-own-derivative]")!.hidden, false, "a PID law shows its derivative fields");
      for (const [field, value] of [["own.derivativeGain", "P1.Kd"], ["own.derivativeGain.minimum", "0"], ["own.derivativeGain.maximum", "50"],
        ["own.derivativeState", "P1.PreviousErrorC"], ["own.derivativeOn", "error"]]) view.set(field, value);
      view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.click();
      assert.deepStrictEqual(view.posts[0].draft, { ...ownLawDraft, law: "PID", derivativeGain: { symbol: "P1.Kd", minimum: "0", maximum: "50" },
        derivativeState: "P1.PreviousErrorC", derivativeOn: "error" });
      view.set("own.law", "PI");
      assert.strictEqual(view.root.querySelector<HTMLElement>("[data-own-derivative]")!.hidden, true);
    } finally { view.close(); }
  });

  test("asks a P law for neither an integral gain nor an integral term", () => {
    const view = open(ownLawModel);
    try {
      for (const [field, value] of ownLawFill.filter(([field]) => !field.startsWith("own.integralGain"))) view.set(field, value);
      view.set("own.law", "P");
      assert.strictEqual(view.root.querySelector<HTMLElement>("[data-own-integral]")!.hidden, true, "a P law has no integral fields");
      view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.click();
      // The hidden integral fields are posted as typed, blank here; the engine takes no integral gain or term for a P law (C2).
      assert.deepStrictEqual(view.posts[0].draft, { ...ownLawDraft, law: "P", integralGain: { symbol: "", minimum: "", maximum: "" }, integralTerm: null });
    } finally { view.close(); }
  });

  test("opens the startup section when the engine refuses for the flag, and posts the flag once chosen", () => {
    const view = open(ownLawModel);
    try {
      for (const [field, value] of [...ownLawFill, ["own.integralTerm", "P1.Integral"]]) view.set(field, value);
      const startup = view.root.querySelector<HTMLElement>("[data-setup-startup]")!;
      assert.strictEqual(startup.hidden, true, "no controller the form names needs the flag before the engine says so");
      view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.click();
      view.showError("The program writes P1.Kp in its own code. Choose the flag that marks its startup as done, so validation keeps the values it starts from.", "startupFlag");
      assert.strictEqual(startup.hidden, false, "a refusal that names the flag opens its section");
      view.set("startupFlag", "P1.Started");
      view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.click();
      assert.deepStrictEqual(view.posts[1].draft, { ...ownLawDraft, startupFlag: "P1.Started" });
    } finally { view.close(); }
  });

  test("the integral term the program updates every scan needs no startup flag", () => {
    const view = open(ownLawModel);
    try {
      for (const [field, value] of [...ownLawFill, ["own.integralTerm", "P1.Integral"]]) view.set(field, value);
      assert.strictEqual(view.root.querySelector<HTMLElement>("[data-setup-startup]")!.hidden, true);
    } finally { view.close(); }
  });
});
