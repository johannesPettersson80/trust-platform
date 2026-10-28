import * as assert from "assert";
import { open } from "./control-studio-loop-setup-form-harness";

/**
 * A program whose own PID law follows a planned trajectory (specification 35, C11; lesson 90): the setup form asks for the PID as the
 * own-law setup does, the planned trajectory, the feedforward term the program adds, and the two feedforward gains. The engine decides
 * the rules and writes the loop; these tests prove what the page shows and posts.
 */
const trajectorySignals = ["P1.AngleRad", "P1.ManualTorqueNm", "P1.ReferenceRad", "P1.SetpointRad", "P1.TorqueNm"];
const trajectoryModel = {
  structure: "identify", pids: [], prefills: [], loopNames: [], records: [], initializes: [], instances: ["P1"],
  addressedReal: trajectorySignals, measurements: trajectorySignals, signals: trajectorySignals,
  booleans: ["P1.AutoMode", "P1.Started"], modes: ["P1.AutoMode", "P1.Started"],
  reals: ["P1.AccelerationGain", "P1.AngleRad", "P1.FeedForward", "P1.Integral", "P1.Kd", "P1.Ki", "P1.Kp", "P1.ManualTorqueNm",
    "P1.PreviousAngleRad", "P1.RateGain", "P1.ReferenceRad", "P1.SetpointRad", "P1.TorqueNm"],
  writes: ["P1.TorqueNm"],
};

/** The form's fields for the trajectory structure, each as a user's edit sets it. */
const trajectoryFill: [string, string][] = [["structure", "trajectory-feedforward"], ["name", "Axis"], ["action", "direct"],
  ["own.instance", "P1"], ["own.processVariable", "P1.AngleRad"], ["own.processVariable.unit", "rad"],
  ["own.processVariable.minimum", "-2"], ["own.processVariable.maximum", "2"], ["own.setpoint", "P1.SetpointRad"],
  ["own.manipulatedVariable", "P1.TorqueNm"], ["own.manipulatedVariable.unit", "N·m"], ["own.manipulatedVariable.minimum", "-20"],
  ["own.manipulatedVariable.maximum", "20"], ["own.mode", "P1.AutoMode"], ["own.manualOutput", "P1.ManualTorqueNm"],
  ["own.proportionalGain", "P1.Kp"], ["own.proportionalGain.minimum", "0"], ["own.proportionalGain.maximum", "100"],
  ["own.integralGain", "P1.Ki"], ["own.integralGain.minimum", "0"], ["own.integralGain.maximum", "100"], ["own.integralTerm", "P1.Integral"],
  ["own.derivativeGain", "P1.Kd"], ["own.derivativeGain.minimum", "0"], ["own.derivativeGain.maximum", "100"],
  ["own.derivativeState", "P1.PreviousAngleRad"], ["own.derivativeOn", "measurement"],
  ["traj.plannedTrajectory", "P1.ReferenceRad"], ["traj.plannedTrajectory.unit", "rad"],
  ["traj.plannedTrajectory.minimum", "-2"], ["traj.plannedTrajectory.maximum", "2"],
  ["traj.rateGain", "P1.RateGain"], ["traj.rateGain.minimum", "0"], ["traj.rateGain.maximum", "10"],
  ["traj.accelerationGain", "P1.AccelerationGain"], ["traj.accelerationGain.minimum", "0"], ["traj.accelerationGain.maximum", "10"],
  ["traj.feedforwardTerm", "P1.FeedForward"]];

/** The draft the trajectory structure posts for the fill above: the PID law as the own-law draft, then the trajectory's variables, every
 * number as the text typed (B1.3). */
const trajectoryDraft = {
  structure: "trajectory-feedforward",
  ownLaw: {
    name: "Axis", action: "direct", law: "PID", instance: "P1",
    processVariable: { symbol: "P1.AngleRad", unit: "rad", minimum: "-2", maximum: "2" },
    setpoint: { symbol: "P1.SetpointRad" },
    manipulatedVariable: { symbol: "P1.TorqueNm", unit: "N·m", minimum: "-20", maximum: "20" },
    mode: { symbol: "P1.AutoMode", whenTrue: "Auto" }, manualOutput: "P1.ManualTorqueNm",
    proportionalGain: { symbol: "P1.Kp", minimum: "0", maximum: "100" },
    integralGain: { symbol: "P1.Ki", minimum: "0", maximum: "100" }, integralTerm: "P1.Integral",
    derivativeGain: { symbol: "P1.Kd", minimum: "0", maximum: "100" }, derivativeState: "P1.PreviousAngleRad", derivativeOn: "measurement",
  },
  rateGain: { symbol: "P1.RateGain", minimum: "0", maximum: "10" },
  accelerationGain: { symbol: "P1.AccelerationGain", minimum: "0", maximum: "10" },
  plannedTrajectory: { symbol: "P1.ReferenceRad", unit: "rad", minimum: "-2", maximum: "2" },
  feedforwardTerm: "P1.FeedForward",
};

suite("Control Studio trajectory feedforward setup form", () => {
  test("offers the trajectory structure beside the own law, and shows its PID law and its trajectory's variables", () => {
    const view = open(trajectoryModel);
    try {
      const structures = Array.from(view.root.querySelectorAll<HTMLOptionElement>('[data-setup-field="structure"] option')).map(option => option.value);
      assert.ok(structures.includes("trajectory-feedforward"), `a trajectory feedforward has its own structure: ${structures}`);
      view.set("structure", "trajectory-feedforward");
      const section = (name: string) => view.root.querySelector<HTMLElement>(`[data-setup-section="${name}"]`)!;
      assert.strictEqual(section("trajectory-feedforward").hidden, false, "the trajectory's variables show");
      assert.strictEqual(section("own-law").hidden, false, "the program's PID law shows with them");
      assert.strictEqual(section("search").hidden, true, "no other structure shows");
      assert.strictEqual(view.root.querySelector<HTMLElement>("[data-own-law-choice]")!.hidden, true, "the law is PID by its structure, so no law is chosen");
      assert.strictEqual(view.root.querySelector<HTMLElement>("[data-own-derivative]")!.hidden, false, "a PID law asks for its derivative");
      assert.strictEqual(view.root.querySelector<HTMLElement>("[data-own-integral]")!.hidden, false, "a PID law asks for its integral gain");
    } finally { view.close(); }
  });

  test("posts the PID law as an own-law draft, with the planned trajectory, the feedforward term and the two feedforward gains", function () {
    // The fill is about thirty-seven edits through JSDOM, each re-rendering the form: the budget the range test below
    // explains.
    this.timeout(10000);
    const view = open(trajectoryModel);
    try {
      view.set("structure", "trajectory-feedforward");
      for (const [field, value] of trajectoryFill) view.set(field, value);
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      assert.strictEqual(save.disabled, false, view.root.querySelector("[data-setup-message]")!.textContent ?? "");
      save.click();
      assert.deepStrictEqual(view.posts[0].draft, trajectoryDraft);
    } finally { view.close(); }
  });

  test("keeps the PID law PID whatever the own law choice holds, since the trajectory's law is PID", function () {
    // The fill is about thirty-seven edits through JSDOM, each re-rendering the form: the budget the range test below
    // explains.
    this.timeout(10000);
    const view = open(trajectoryModel);
    try {
      view.set("structure", "trajectory-feedforward");
      for (const [field, value] of trajectoryFill) view.set(field, value);
      view.set("own.law", "P");
      view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.click();
      assert.strictEqual(view.posts[0].draft.ownLaw.law, "PID");
      assert.strictEqual(view.root.querySelector<HTMLElement>("[data-own-derivative]")!.hidden, false, "a PID law still asks for its derivative");
    } finally { view.close(); }
  });

  test("posts a missing planned trajectory blank, so the engine names it", function () {
    // The fill is about thirty-seven edits through JSDOM, each re-rendering the form: the budget the range test below
    // explains.
    this.timeout(10000);
    const view = open(trajectoryModel);
    try {
      view.set("structure", "trajectory-feedforward");
      for (const [field, value] of trajectoryFill.filter(([field]) => !field.startsWith("traj.plannedTrajectory"))) view.set(field, value);
      view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.click();
      assert.deepStrictEqual(view.posts[0].draft.plannedTrajectory, { symbol: "", unit: "", minimum: "", maximum: "" },
        "the planned trajectory is posted as typed, blank; the engine refuses the draft and names it");
    } finally { view.close(); }
  });

  test("posts the rate feedforward gain's missing range blank, so the engine names it", function () {
    // The fill is about thirty-seven edits through JSDOM, each re-rendering the form, so this synchronous test needs the budget that
    // control-studio-decoupler.test.ts also sets for its fill (Mocha fails a synchronous test that runs longer than its timeout).
    this.timeout(10000);
    const view = open(trajectoryModel);
    try {
      view.set("structure", "trajectory-feedforward");
      for (const [field, value] of trajectoryFill.filter(([field]) => field !== "traj.rateGain.minimum")) view.set(field, value);
      view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.click();
      assert.strictEqual(view.posts[0].draft.rateGain.minimum, "", "the missing minimum of the range is posted as the blank text typed, and the engine names it");
      assert.strictEqual(view.posts[0].draft.rateGain.maximum, "10", "the range that is set is posted as the text typed, for the engine to read");
    } finally { view.close(); }
  });
});
