import * as assert from "assert";
import { formatEngineeringNumber } from "../../controlStudio/chartContract";
import { inputsReceiver } from "../../controlStudio/webview/engineInputs";
import { receiveEngineMessage, type EngineHostPorts } from "../../controlStudio/webview/engineOpened";
import { tuneReceiver } from "../../controlStudio/webview/engineTune";
import { ThreePageController } from "../../controlStudio/webview/threePageController";
import { ThreePageTuning } from "../../controlStudio/webview/threePageTuning";
import { pidLoop } from "./control-studio-pid-fixtures";

const { JSDOM } = require("jsdom");

/** The Tune page's controls, design form and result area, as the three-page template draws them for a state-feedback loop. */
const MARKUP = `<main>
  <select data-studio-controller><option value="automatic">Automatic</option></select>
  <select data-engineer-tuning-method><option value="automatic">Automatic</option><option value="SIMC">SIMC</option><option value="manual">Manual</option></select>
  <section data-studio-state-design hidden></section>
  <div data-studio-tuning-result hidden></div>
</main>`;

/** A form field as the engine lists it: its key and label, and for a choice its options with their words and the option that stands (C4, D-B12). */
interface ListedField { field: string; label: string; choices?: { value: string; label: string }[]; selected?: string }

/** The solver policy as the engine lists it (`form_fields`, preferences.rs): its two options with their words, and the one that stands. */
const SOLVER_CHOICES = [{ value: "infinite", label: "Infinite-horizon / steady-state" }, { value: "finite", label: "Finite-horizon (fixed policy)" }];
const SOLVER: ListedField = { field: "solverMode", label: "Solver policy", choices: SOLVER_CHOICES, selected: "infinite" };
const SOLVER_FINITE: ListedField = { ...SOLVER, selected: "finite" };

/** The fields the engine lists for each family, in the order the engine lists them (C4, D-B12). */
const FIELDS: Record<string, ListedField[]> = {
  "state-feedback": [{ field: "desiredPoles", label: "Desired poles" }],
  LQR: [
    SOLVER,
    { field: "stateWeights", label: "State weights (Q)" },
    { field: "inputWeights", label: "Input weights (R)" },
  ],
  "LQR finite": [
    SOLVER_FINITE,
    { field: "stateWeights", label: "State weights (Q)" },
    { field: "inputWeights", label: "Input weights (R)" },
    { field: "horizonSteps", label: "Finite horizon steps" },
    { field: "terminalWeights", label: "Terminal weights" },
  ],
  LQG: [
    SOLVER,
    { field: "stateWeights", label: "State weights (Q)" },
    { field: "inputWeights", label: "Input weights (R)" },
    { field: "processNoise", label: "Process noise covariance" },
    { field: "measurementNoise", label: "Measurement noise covariance" },
  ],
  "LQG finite": [
    SOLVER_FINITE,
    { field: "stateWeights", label: "State weights (Q)" },
    { field: "inputWeights", label: "Input weights (R)" },
    { field: "processNoise", label: "Process noise covariance" },
    { field: "measurementNoise", label: "Measurement noise covariance" },
    { field: "horizonSteps", label: "Finite horizon steps" },
    { field: "terminalWeights", label: "Terminal weights" },
  ],
};

/**
 * A state law's Tune answer as the engine gives it (C4): two state gains and a reference gain, a closed-loop pole pair, a two-step
 * finite horizon, and a phase margin with its delay margin.
 */
const TUNE_RESULT: Record<string, unknown> = {
  settings: { "Main.K1": 0.05616, "Main.K2": 0.03693, "Main.N": 0.11309 },
  method: "state-space",
  methodTitle: "State-space design",
  methodDescription: "The state-space design of the loop's state law.",
  reason: "The engineer chose the state-space design.",
  objective: "more-robust",
  attempts: [],
  gainUnits: {},
  requirements: {},
  outcome: { status: "unpredicted", reason: "The study keeps no prediction." },
  // The engine's words of the answer (TuneWords): the outcome sentence and the rows of the gains, named by the loop's conventions.
  words: { outcome: "No prediction: The study keeps no prediction.", horizonPolicy: "Finite horizon: 2 steps · Repeated first gain", horizonStages: "Riccati stages", settingLabels: [
    { symbol: "Main.K1", label: "Gain K1 on tank 1's level deviation" }, { symbol: "Main.K2", label: "Gain K2 on tank 2's level deviation" },
    { symbol: "Main.N", label: "Reference gain N" }] },
  margins: { gainMarginDb: { finite: 14.2 }, phaseMarginDegrees: { finite: 95 }, peakSensitivity: 1.3, peakSensitivityRadPerSecond: 0.4 },
  delayMarginSeconds: { finite: 6.1 },
  comparison: [],
  stageEvidence: {},
  stages: [],
  state: {
    family: "LQR",
    closedLoopPoles: [{ real: 0.9231163463866358, imaginary: 0 }, { real: 0.5, imaginary: 0.2 }],
    horizon: {
      steps: 2,
      policy: "repeat-first-finite-gain",
      gainSequence: [[[0.5, 0.1]], [[0.4, 0.05]]],
      riccatiSequence: [[[10, 0], [0, 10]], [[8, 0], [0, 8]], [[5, 0], [0, 5]]],
    },
    fields: FIELDS.LQR,
  },
};

/** A state-feedback loop with two state gains and a reference gain, as lesson 68 declares them (C4). */
function stateLoop(): any {
  const loop = pidLoop();
  loop.controller = { ...loop.controller, instance: "P1", type: "State feedback", law: undefined, stateFeedback: {
    family: "state-feedback", convention: "sampled-deviation-negative-feedback", gainSymbols: [["Main.K1", "Main.K2"]], referenceGainSymbols: [["Main.N"]],
  } };
  loop.parameterBinding = { ...loop.parameterBinding, parameters: [
    { symbol: "Main.K1", iec_type: "LREAL", unit: "m3/s/m", convention: "Gain K1 on tank 1's level deviation", minimum: -1, maximum: 1 },
    { symbol: "Main.K2", iec_type: "LREAL", unit: "m3/s/m", convention: "Gain K2 on tank 2's level deviation", minimum: -1, maximum: 1 },
    { symbol: "Main.N", iec_type: "LREAL", unit: "m3/s/m", convention: "Reference gain N", minimum: -1, maximum: 1 },
  ] };
  return loop;
}

/** The controller the engine offers a state law on open (`controllerOffer`, C2, C4): its two families, labelled, and the one it recommends. */
function stateOffer(recommended: string | null, familySettings: Record<string, string[]> = {}) {
  return { controllerOffer: { families: [{ family: "state-feedback", label: "State feedback" }, { family: "LQR", label: "LQR" }],
    recommended, familySettings, settingRows: [] } };
}

interface Page {
  readonly root: HTMLElement;
  readonly controller: ThreePageController;
  readonly tuning: ThreePageTuning;
  readonly ports: EngineHostPorts;
  /** The page's posts to the host, in order. */
  readonly posts: Array<{ kind: string; payload: Record<string, unknown> | undefined }>;
}

/** Runs `check` on a state law's Tune page, with the engine's messages routed as the workflow routes them. */
function withStatePage(check: (page: Page) => void): void {
  const dom = new JSDOM(MARKUP);
  const previous = (globalThis as any).document;
  // The controller's options are built with the browser's `Option` constructor, which the test's DOM provides as a global.
  const previousOption = (globalThis as any).Option;
  (globalThis as any).document = dom.window.document;
  (globalThis as any).Option = dom.window.Option;
  try {
    const root = dom.window.document.querySelector("main") as HTMLElement;
    const loop = stateLoop();
    const posts: Page["posts"] = [];
    const post = (kind: string, payload?: Record<string, unknown>): void => {
      posts.push({ kind, payload });
    };
    const controller = new ThreePageController(root, loop, post);
    const tuning = new ThreePageTuning(root, loop, post, () => controller.selectedFamily, () => controller.chosenFamily,
      family => controller.symbolsOf(family));
    controller.methodsChanged = (methods, titles) => tuning.offerMethods(methods, titles);
    const ports: EngineHostPorts = {
      select: () => undefined,
      receive: (stage, payload) => {
        if (stage !== "engineer") return;
        controller.receive(payload);
        tuning.receive(payload);
      },
    };
    check({ root, controller, tuning, ports, posts });
  } finally {
    (globalThis as any).document = previous;
    (globalThis as any).Option = previousOption;
    dom.window.close();
  }
}

/** Dispatches a DOM event of the page's window, as an edit or a choice does. */
function fire(element: Element, type: string): void {
  const view = element.ownerDocument.defaultView as any;
  element.dispatchEvent(new view.Event(type, { bubbles: true }));
}

function controlsOf(root: HTMLElement): string[] {
  return Array.from(root.querySelectorAll<HTMLElement>("[data-state-design-field]")).map(control => control.dataset.stateDesignField ?? "");
}

suite("Control Studio state-space design page", () => {
  test("renders the fields the engine lists for each family, in the engine's order and labels", () => {
    for (const family of Object.keys(FIELDS)) {
      withStatePage(({ root, ports }) => {
        inputsReceiver.result?.(root, { stateFields: FIELDS[family] }, ports);
        assert.deepStrictEqual(controlsOf(root), FIELDS[family].map(item => item.field), `${family} shows the fields the engine lists`);
        const labels = Array.from(root.querySelectorAll<HTMLElement>("[data-studio-state-design] > label")).map(label => label.firstChild?.textContent);
        assert.deepStrictEqual(labels, FIELDS[family].map(item => item.label), `${family} shows the engine's labels`);
      });
    }
  });

  test("keeps the text as typed and does not refuse a draft itself: the engine refuses it, with the field", () => {
    withStatePage(({ root, tuning, ports }) => {
      inputsReceiver.result?.(root, { stateFields: FIELDS.LQR }, ports);
      const weights = root.querySelector<HTMLTextAreaElement>("[data-state-design-field=\"stateWeights\"]")!;
      // The engineer's typing is an input event; the page keeps the text it holds from those events.
      weights.value = "[[";
      fire(weights, "input");
      assert.doesNotThrow(() => tuning.preferences(true), "a draft the page cannot read is still sent");
      assert.strictEqual(tuning.preferences(true).stateDesignDraft?.stateWeights, "[[", "the text is kept as typed");
    });
  });

  test("sends the typed text as the study's draft, with the chosen family and the state-space method", () => {
    withStatePage(({ root, controller, ports, posts }) => {
      controller.receive({ ...stateOffer("LQR"), methods: ["state-space"] });
      const select = root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
      select.value = "P1::LQR";
      fire(select, "change");
      const method = root.querySelector<HTMLSelectElement>("[data-engineer-tuning-method]")!;
      method.value = "state-space";
      fire(method, "change");
      inputsReceiver.result?.(root, { stateFields: FIELDS.LQR }, ports);
      const weights = root.querySelector<HTMLTextAreaElement>("[data-state-design-field=\"stateWeights\"]")!;
      weights.value = "[[400, 0], [0, 400]]";
      fire(weights, "input");
      const sent = posts[posts.length - 1];
      assert.strictEqual(sent.kind, "control-studio.engineer.invalidate");
      assert.strictEqual(sent.payload?.family, "LQR", "the chosen family goes with the change");
      const preferences = sent.payload?.tuningPreferences as { method: string; stateDesignDraft: Record<string, string> };
      assert.strictEqual(preferences.method, "state-space", "Tune's method is the engine's name");
      assert.strictEqual(preferences.stateDesignDraft.stateWeights, "[[400, 0], [0, 400]]", "the text is sent as typed");
      assert.strictEqual(preferences.stateDesignDraft.solverMode, "infinite", "the solver is one of the engine's two choices");
    });
  });

  test("shows a state result: the gains under the loop's symbols, the closed-loop poles, the margins and the Riccati stages", () => {
    withStatePage(({ root, ports }) => {
      receiveEngineMessage(root, { kind: "control-studio.engine.result", method: "tune", result: TUNE_RESULT }, ports);
      const area = root.querySelector<HTMLElement>("[data-studio-tuning-result]")!;
      assert.strictEqual(area.hidden, false);
      const k1 = root.querySelector<HTMLElement>("[data-studio-parameter=\"Main.K1\"]");
      assert.ok(k1?.textContent?.includes(formatEngineeringNumber(0.05616)), "K1 is shown under the loop's own symbol");
      const poles = root.querySelector<HTMLElement>("[data-studio-closed-loop-poles]")?.textContent ?? "";
      assert.ok(poles.includes(formatEngineeringNumber(0.9231163463866358)), "the real pole is shown");
      assert.ok(poles.includes(`${formatEngineeringNumber(0.5)} + ${formatEngineeringNumber(0.2)}i`), `the complex pole is shown: ${poles}`);
      const margins = root.querySelector<HTMLElement>("[data-studio-robustness]")?.textContent ?? "";
      assert.ok(margins.includes(`phase margin ${formatEngineeringNumber(95)}°`), `the phase margin is shown: ${margins}`);
      assert.strictEqual(root.querySelector("[data-studio-tune-note]"), null, "no margin is missing, so no sentence says so");
      assert.strictEqual(root.querySelector("[data-studio-design-policy]")?.textContent, "Finite horizon: 2 steps · Repeated first gain");
      const details = root.querySelector("details");
      assert.strictEqual(details?.querySelector("summary")?.textContent, "Riccati stages");
      assert.strictEqual(root.querySelectorAll("[data-studio-riccati-stage]").length, 3, "one row per Riccati matrix");
    });
  });

  test("shows the engine's sentence when the margins are unavailable", () => {
    withStatePage(({ root, ports }) => {
      const result = { ...TUNE_RESULT, margins: undefined, marginsUnavailable: "The margins are not given for a sampled model." };
      receiveEngineMessage(root, { kind: "control-studio.engine.result", method: "tune", result }, ports);
      assert.strictEqual(root.querySelector("[data-studio-tune-note]")?.textContent, "The margins are not given for a sampled model.");
      assert.strictEqual(root.querySelector("[data-studio-robustness]"), null, "no margins line is shown without margins");
    });
  });

  test("renders a reopened state law's answer with the same code as the live one", () => {
    let live = "";
    withStatePage(({ root, ports }) => {
      receiveEngineMessage(root, { kind: "control-studio.engine.result", method: "tune", result: TUNE_RESULT }, ports);
      live = root.querySelector<HTMLElement>("[data-studio-tuning-result]")!.innerHTML;
    });
    withStatePage(({ root, ports }) => {
      tuneReceiver.opened?.(root, { tune: TUNE_RESULT }, ports);
      assert.strictEqual(root.querySelector<HTMLElement>("[data-studio-tuning-result]")!.innerHTML, live,
        "the reopened answer shows what the live answer showed");
    });
  });

  test("offers exactly the methods the engine gives, and labels the family the engine recommends", () => {
    withStatePage(({ root, controller }) => {
      controller.receive({ ...stateOffer("state-feedback"), methods: ["state-space"], methodTitles: { "state-space": "State-space design" } });
      const families = Array.from(root.querySelectorAll<HTMLOptionElement>("[data-studio-controller] option")).map(option => option.textContent);
      assert.ok(families.includes("P1 · State feedback — recommended"), `the recommended family is labelled: ${families.join(", ")}`);
      assert.ok(families.includes("P1 · LQR"), `the other family is listed: ${families.join(", ")}`);
      const methods = Array.from(root.querySelectorAll<HTMLOptionElement>("[data-engineer-tuning-method] option"))
        .map(option => [option.value, option.textContent]);
      assert.deepStrictEqual(methods, [["automatic", "Automatic"], ["state-space", "State-space design"]],
        "Automatic and the engine's methods only: the PID methods are not offered for a state law");
    });
  });

  test("the open answer of lesson 69's state-feedback loop offers its State-space design, with LQR recommended", () => {
    // The answer the engine gives an opened state law: its two families, the symbols they take and the one method (C4).
    withStatePage(({ root, controller }) => {
      controller.receive({ ...stateOffer("LQR", { "state-feedback": ["P1.K1", "P1.K2", "P1.N"], LQR: ["P1.K1", "P1.K2", "P1.N"] }),
        methods: ["state-space"], methodTitles: { "state-space": "State-space design" } });
      const methods = Array.from(root.querySelectorAll<HTMLOptionElement>("[data-engineer-tuning-method] option"))
        .map(option => [option.value, option.textContent]);
      assert.deepStrictEqual(methods, [["automatic", "Automatic"], ["state-space", "State-space design"]],
        "the Tune page offers State-space design for the opened loop");
      const options = Array.from(root.querySelectorAll<HTMLOptionElement>("[data-studio-controller] option")).map(option => option.textContent);
      assert.ok(options.includes("P1 · LQR — recommended"), `the engine's recommended family is marked: ${options.join(", ")}`);
      assert.strictEqual(controller.selectedFamily, "automatic", "Automatic is shown until the engineer chooses (B4.2): the recommendation is only marked");
    });
  });

  test("shows a refused field's sentence under that field, and a refusal of the whole form at the form's head", () => {
    withStatePage(({ root, ports }) => {
      inputsReceiver.result?.(root, { stateFields: FIELDS.LQR }, ports);
      receiveEngineMessage(root, { kind: "control-studio.engine.failure", method: "tune",
        failure: { message: "State weights must be valid JSON.", cause: "settings", nextAction: "none", field: "stateWeights" } }, ports);
      const message = root.querySelector<HTMLElement>("[data-state-design-error=\"stateWeights\"]")!;
      assert.strictEqual(message.textContent, "State weights must be valid JSON.");
      assert.strictEqual(message.hidden, false, "the sentence is shown under the field");
      assert.strictEqual(root.querySelector("[data-state-design-field=\"stateWeights\"]")?.getAttribute("aria-invalid"), "true");
      assert.strictEqual(root.querySelector<HTMLElement>("[data-state-design-form-error]")?.hidden, true, "a refusal that names a field is not repeated at the form's head");

      receiveEngineMessage(root, { kind: "control-studio.engine.failure", method: "tune",
        failure: { message: "The weights are not symmetric.", detail: "The weights are not symmetric. Enter the same value in both off-diagonal places.",
          cause: "settings", nextAction: "none" } }, ports);
      assert.strictEqual(root.querySelector("[data-state-design-form-error]")?.textContent,
        "The weights are not symmetric. Enter the same value in both off-diagonal places.", "a refusal of the whole form shows its whole text at the form's head");
      assert.strictEqual(root.querySelector<HTMLElement>("[data-studio-tuning-result]")!.hidden, true, "the result area shows no refusal");

      receiveEngineMessage(root, { kind: "control-studio.engine.failure", method: "tune",
        failure: { message: "Desired poles must be valid JSON.", cause: "settings", nextAction: "none", field: "desiredPoles" } }, ports);
      assert.strictEqual(root.querySelector("[data-state-design-form-error]")?.textContent, "Desired poles must be valid JSON.",
        "a field this form does not list is refused at the form's head");
    });
  });

  test("a refused field's sentence clears when the engineer edits that field", () => {
    withStatePage(({ root, ports }) => {
      inputsReceiver.result?.(root, { stateFields: FIELDS.LQR }, ports);
      receiveEngineMessage(root, { kind: "control-studio.engine.failure", method: "tune",
        failure: { message: "State weights must be valid JSON.", cause: "settings", nextAction: "none", field: "stateWeights" } }, ports);
      const weights = root.querySelector<HTMLTextAreaElement>("[data-state-design-field=\"stateWeights\"]")!;
      const message = root.querySelector<HTMLElement>("[data-state-design-error=\"stateWeights\"]")!;
      assert.strictEqual(message.hidden, false);
      weights.value = "[[1]]";
      fire(weights, "input");
      assert.strictEqual(message.hidden, true, "the sentence goes once the field is edited");
      assert.strictEqual(weights.getAttribute("aria-invalid"), null, "the invalid mark goes with it");
    });
  });

  test("lists a choice as the engine gives it, with the option that stands, and invents neither an option nor a default (R3, R4)", () => {
    withStatePage(({ root, ports, posts }) => {
      // A policy the page has never heard of is listed under the engine's words and stands; the page keeps no table that would drop it.
      const adaptive: ListedField = { field: "solverMode", label: "Solver policy", selected: "adaptive",
        choices: [{ value: "adaptive", label: "Adaptive horizon" }, ...SOLVER_CHOICES] };
      inputsReceiver.result?.(root, { stateFields: [adaptive, { field: "stateWeights", label: "State weights (Q)" }] }, ports);
      const solver = root.querySelector<HTMLSelectElement>("select[data-state-design-mode]")!;
      assert.deepStrictEqual(Array.from(solver.options).map(option => [option.value, option.textContent]),
        [["adaptive", "Adaptive horizon"], ["infinite", "Infinite-horizon / steady-state"], ["finite", "Finite-horizon (fixed policy)"]]);
      assert.strictEqual(solver.value, "adaptive", "the option the engine says stands is shown");
      const weights = root.querySelector<HTMLTextAreaElement>("[data-state-design-field=\"stateWeights\"]")!;
      weights.value = "[[1]]";
      fire(weights, "input");
      const sent = (posts[posts.length - 1].payload?.tuningPreferences as { stateDesignDraft: Record<string, string> }).stateDesignDraft;
      assert.strictEqual(sent.solverMode, "adaptive", "the draft names the option the control shows");
      // A later answer that moves the standing option moves a control the engineer left alone, and not one the engineer set.
      inputsReceiver.result?.(root, { stateFields: [{ ...adaptive, selected: "finite" }, { field: "stateWeights", label: "State weights (Q)" }] }, ports);
      assert.strictEqual(solver.value, "finite", "the engine's standing option follows its answer");
      solver.value = "infinite";
      fire(solver, "change");
      inputsReceiver.result?.(root, { stateFields: [{ ...adaptive, selected: "finite" }, { field: "stateWeights", label: "State weights (Q)" }] }, ports);
      assert.strictEqual(solver.value, "infinite", "the option the engineer chose stays");
    });
  });

  test("changing the solver sends the draft through the study's inputs, and the answer's list adds or removes the horizon fields", () => {
    withStatePage(({ root, ports, posts }) => {
      inputsReceiver.result?.(root, { stateFields: FIELDS.LQR }, ports);
      const solver = root.querySelector<HTMLSelectElement>("select[data-state-design-mode]")!;
      assert.deepStrictEqual(Array.from(solver.options).map(option => [option.value, option.textContent]),
        [["infinite", "Infinite-horizon / steady-state"], ["finite", "Finite-horizon (fixed policy)"]], "the solver offers the engine's two choices, in its words");
      assert.strictEqual(root.querySelector("textarea[data-state-design-field=\"solverMode\"]"), null, "the solver is a choice, not text");
      const sent = (): Record<string, string> => (posts[posts.length - 1].payload?.tuningPreferences as { stateDesignDraft: Record<string, string> }).stateDesignDraft;
      solver.value = "finite";
      fire(solver, "change");
      assert.strictEqual(sent().solverMode, "finite", "the draft the study receives names the finite solver");
      inputsReceiver.result?.(root, { stateFields: FIELDS["LQR finite"] }, ports);
      assert.deepStrictEqual(controlsOf(root), FIELDS["LQR finite"].map(item => item.field), "the answer adds the horizon fields, in its order");
      assert.strictEqual(root.querySelector("select[data-state-design-mode]"), solver, "the solver stays the same control, so the engineer keeps focus");
      const steps = root.querySelector<HTMLInputElement>("[data-state-design-field=\"horizonSteps\"]")!;
      steps.value = "20";
      fire(steps, "input");
      solver.value = "infinite";
      fire(solver, "change");
      assert.strictEqual(sent().horizonSteps, "20", "the draft keeps the horizon text the solver hides");
      inputsReceiver.result?.(root, { stateFields: FIELDS.LQR }, ports);
      assert.strictEqual(root.querySelector("[data-state-design-field=\"horizonSteps\"]"), null, "the answer removes the horizon fields");
      solver.value = "finite";
      fire(solver, "change");
      inputsReceiver.result?.(root, { stateFields: FIELDS["LQR finite"] }, ports);
      assert.strictEqual(root.querySelector<HTMLInputElement>("[data-state-design-field=\"horizonSteps\"]")?.value, "20", "the horizon text comes back with its field");
    });
  });
});
