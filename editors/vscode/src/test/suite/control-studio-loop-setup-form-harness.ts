import * as assert from "assert";
import { renderLoopSetupBody } from "../../controlStudio/loopSetup/loopSetupTemplate";
import { initializeLoopSetupForm } from "../../controlStudio/webview/loopSetupForm";

/** The structures the engine offers a program with one library PID, a switching instance and a state-feedback instance (C0 to C12). A
 * fixture names its own `structures` when it needs more, such as the group for several PIDs. */
export const DEFAULT_STRUCTURES = ["pid", "cascade", "feedforward", "smith", "schedule", "onoff", "state-feedback", "own-law",
  "trajectory-feedforward", "search", "identify"];

/** The engine's answer for the choices it preselects, the rows it draws and the words it shows (B1.3). These are the values
 * `form_tests.rs` pins; a fixture changes one only to show that the page follows the engine's answer. */
export const ENGINE_FORM_FIELDS = {
  defaults: { structure: "pid", stateFamily: "LQR", searchFamily: "MPC", searchKeptStart: "zero", modeWhenTrue: "Auto", action: "direct",
    bandConvention: "half-band", thresholdEquality: "at-or-beyond", ownLaw: "PI", trajectoryOwnLaw: "PID" },
  limits: { furtherOutputs: 3, estimatorStates: 3, searchCoefficients: 4, searchKeptValues: 3, groupMembersMinimum: 2 },
  /** The operating-point rows of a gain schedule: four, the first two required (C10). */
  schedulePoints: [{ required: true }, { required: true }, { required: false }, { required: false }],
  wording: { trueMeans: "TRUE means", notFound: "Not found in the code", notStated: "Not stated in the code" },
  choices: { modeWhenTrue: ["Auto", "Manual"], bandConventions: ["half-band", "full-band"], thresholdEqualities: ["at-or-beyond", "beyond"],
    stateFamilies: ["LQR", "state-feedback", "LQG"], ownLaws: ["PI", "P", "PID"], derivativeOn: ["error", "measurement"],
    searchFamilies: ["MPC", "ratio", "split-range", "override", "gain-scheduled", "feedforward", "Smith", "lead-lag", "adaptive", "robust",
      "feedback-linearization", "observer-feedback"],
    keptStarts: ["zero", "output", "measurement"], actions: ["direct", "reverse"] },
  /** The engine's words for each value (`labels`, `setup/texts/form.rs`): the page shows them and keeps none of its own. */
  labels: {
    structures: {
      pid: "One PID loop",
      cascade: "Cascade: one PID sets the other's setpoint",
      feedforward: "Feedforward: the PID adds the program's feedforward from a measured disturbance",
      smith: "Smith predictor: the PID reads the measurement plus the program's predicted correction",
      schedule: "Gain schedule: the program interpolates the PID's gains between operating points",
      group: "PID group: interacting PIDs tuned together",
      onoff: "ON/OFF: the program switches the output",
      "state-feedback": "State feedback: the program feeds measured outputs back",
      "own-law": "Own P, PI or PID: the program computes the output from its gain variables",
      "trajectory-feedforward": "Trajectory feedforward: the PID adds a feedforward along the program's planned trajectory",
      search: "Other law: Tune searches the program's coefficient values",
      identify: "Identify only, no controller",
    },
    modeWhenTrue: { Auto: "Auto", Manual: "Manual" },
    bandConventions: { "half-band": "Half the band: switches at setpoint ± hysteresis", "full-band": "The whole band: switches at setpoint ± hysteresis / 2" },
    thresholdEqualities: { "at-or-beyond": "At or beyond the threshold (the default)", beyond: "Only strictly beyond the threshold" },
    stateFamilies: { LQR: "LQR, weighted by the signal ranges", "state-feedback": "Pole placement",
      LQG: "LQG: the program estimates the states from the controlled output" },
    ownLaws: { PI: "PI: u = Kp·e + I, where I integrates Ki·e", P: "P: u = Kp·e plus the program's own offset",
      PID: "PID: u = Kp·e + I + D, where D is Kd times the rate of the error or measurement" },
    derivativeOn: { error: "The error", measurement: "The measurement" },
    searchFamilies: { MPC: "Model predictive control", ratio: "Ratio", "split-range": "Split range", override: "Override",
      "gain-scheduled": "Gain schedule", feedforward: "Feedforward", Smith: "Smith predictor", "lead-lag": "Lead/lag", adaptive: "Adaptive",
      robust: "Robust", "feedback-linearization": "Feedback linearization", "observer-feedback": "Observer feedback" },
    keptStarts: { zero: "Zero", output: "The output", measurement: "The measurement" },
    actions: { direct: "Increase the output while the measurement is below the setpoint",
      reverse: "Increase the output while the measurement is above the setpoint" },
  },
};

/** Static imports: the webview module is compiled for tests only when a test imports it. */
function form(): any {
  return { renderLoopSetupBody, initializeLoopSetupForm };
}

/** The lists of the form model the engine answers for every program: empty unless a fixture names them (SetupFormModel). */
const EMPTY_LISTS = { booleans: [], instances: [], loopNames: [], modes: [], pids: [], prefills: [], records: [], reals: [],
  startupFlagInstances: [] };

/** The setup form rendered in a DOM, with a setter that fires the events a user's edit fires. The model is the engine's answer (the
 * discover answer's form model): a fixture gives its `structures`, `startupFlagInstances` and the lists of its program. The engine
 * preselects the structure the program shows, so `defaults.structure` is the fixture's `structure` unless the fixture names its own defaults.
 * The engine's `signals` are the REAL and LREAL symbols a loop can use, and its `measurements` the addressed inputs a plant can write, so
 * a fixture that names its program's symbols gets both from them unless it names them. */
export function open(model: any): { dom: any; posts: any[]; root: HTMLElement; set: (field: string, value: string) => void;
  showError: (reason: string, field?: string) => void; close: () => void } {
  const api = form();
  assert.strictEqual(typeof api.renderLoopSetupBody, "function", "setup needs one form instead of a prompt chain");
  assert.strictEqual(typeof api.initializeLoopSetupForm, "function");
  const { JSDOM } = require("jsdom");
  const answer: any = { ...ENGINE_FORM_FIELDS, ...EMPTY_LISTS, structures: DEFAULT_STRUCTURES, ...model };
  answer.signals = model.signals ?? answer.reals;
  answer.measurements = model.measurements ?? answer.addressedReal ?? [];
  // The engine's decoupler has a row and a column for each library PID of the program (C7), unless a fixture names another shape.
  answer.decoupler = model.decoupler ?? { rows: answer.pids.length, columns: answer.pids.length };
  answer.defaults = { ...ENGINE_FORM_FIELDS.defaults, structure: model.structure, ...model.defaults };
  const dom = new JSDOM(`<main>${api.renderLoopSetupBody(answer, (text: string) => text)}</main>`);
  const root = dom.window.document.querySelector("main") as HTMLElement;
  const posts: any[] = [];
  const setup = api.initializeLoopSetupForm(root, (message: unknown) => posts.push(message));
  const set = (field: string, value: string) => {
    const element = root.querySelector<HTMLInputElement>(`[data-setup-field="${field}"]`);
    assert.ok(element, `field ${field} is present`);
    element!.value = value;
    element!.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
    element!.dispatchEvent(new dom.window.Event("change", { bubbles: true }));
  };
  return { dom, posts, root, set, showError: (reason: string, field?: string) => setup.showError(reason, field), close: () => dom.window.close() };
}
