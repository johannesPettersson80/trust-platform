import * as assert from "assert";
import { editControlRequirements } from "../../controlStudio/webview/threePageRequirements";
import { setOpenedOffers } from "../../controlStudio/webview/openedOffers";
import type { LoopDefinition } from "../../controlStudio/generated/OpenResult";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

const loop = {
  schemaVersion: 1, id: "scenario-loop", name: "Scenario loop", revision: "r", projectId: "p", runtimeId: "sim",
  controller: { instance: "P1.Controller", type: "Control.PID", revision: "r", action: "direct", taskPeriodMs: 100 },
  signals: {
    manipulatedVariable: { symbol: "P1.ManipulatedValue", address: "%QW0", iecType: "REAL", direction: "write", unit: "%", range: { minimum: 0, maximum: 100 } },
    processVariable: { symbol: "P1.ProcessValue", address: "%IW0", iecType: "REAL", direction: "read", unit: "°C", range: { minimum: 0, maximum: 80 } },
  },
  limits: { outputMinimum: 0, outputMaximum: 100 }, observedSampleTiming: { nominalPeriodMs: 100 },
} as unknown as LoopDefinition;

const scenarios = [{ id: "load-step", initialValues: { "P1.ManipulatedValue": 20 }, events: [{ atMs: 1000, symbol: "P1.ManipulatedValue", value: 35 }], requirements: [{ id: "pv-bounds", kind: "bounds", channel: "P1.ProcessValue", window: { start: 0, end: 20 }, minimum: 0, maximum: 80 }], effects: [{ id: "load-bias", target: "process_input", channel: 0, mode: "add", startSeconds: 2, generator: "constant", value: 2 }] }];

function fixture() {
  const dom = new JSDOM("<main></main>");
  const previousDocument = (globalThis as any).document;
  const previousWindow = (globalThis as any).window;
  const previousHTMLElement = (globalThis as any).HTMLElement;
  (globalThis as any).document = dom.window.document;
  (globalThis as any).window = dom.window;
  (globalThis as any).HTMLElement = dom.window.HTMLElement;
  const showModal = dom.window.HTMLDialogElement.prototype.showModal;
  const close = dom.window.HTMLDialogElement.prototype.close;
  dom.window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  dom.window.HTMLDialogElement.prototype.close = function () { this.open = false; this.dispatchEvent(new dom.window.Event("close")); };
  const posts: Array<{ kind: string; payload: Record<string, unknown> }> = [];
  return { dom, posts, restore() { dom.window.HTMLDialogElement.prototype.showModal = showModal; dom.window.HTMLDialogElement.prototype.close = close; (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; (globalThis as any).HTMLElement = previousHTMLElement; dom.window.close(); } };
}

const REQUIREMENTS = { referenceChange: 5, maximumError: 1, settlingBand: .2, settlingTimeSeconds: 8, durationSeconds: 20 };

/** The scenario the engine's open answer gives the page to add (`scenarioTemplate`, B4.3): one bounded check on PV, with no name. */
const SCENARIO_TEMPLATE = { id: "", initialValues: {}, events: [],
  requirements: [{ id: "requirement-1", kind: "bounds", channel: "P1.ProcessValue", window: { start: 0, end: 20 }, minimum: 0, maximum: 80 }] };

suite("Control Studio three-page scenario requirements", () => {
  teardown(() => setOpenedOffers({}));

  test("shows conditional scenario fields inside the existing Requirements dialog", () => {
    const view = fixture();
    try {
      (editControlRequirements as any)(loop, { referenceChange: 5, maximumError: 1, settlingBand: .2, settlingTimeSeconds: 8, durationSeconds: 20 }, () => undefined, { scenarioDefinitions: scenarios, processSymbol: "P1.ProcessValue", samplePeriodMs: 100 });
      const document: Document = view.dom.window.document;
      assert.ok(document.querySelector("dialog[open]"));
      assert.strictEqual(document.querySelector<HTMLInputElement>("[data-engineer-scenario-id]")?.value, "load-step");
      assert.strictEqual(document.querySelectorAll("[data-engineer-scenario-event-time]").length, 1);
      assert.strictEqual(document.querySelectorAll("[data-engineer-scenario-initial]").length, 1);
      assert.strictEqual(document.querySelectorAll("[data-engineer-scenario-minimum]").length, 1);
      // The bounds rows are named by the channel of the engine's requirement; a page that named a channel of its own would show another word.
      assert.strictEqual(document.querySelector("[data-engineer-scenario] legend")?.textContent, "Bounds: P1.ProcessValue");
      assert.ok(document.querySelector("[data-engineer-scenario-advanced]"));
    } finally { view.restore(); }
  });

  test("Done commits edited scenarioDefinitions and Cancel leaves the host untouched", () => {
    const view = fixture();
    try {
      const holder: { committed?: Record<string, unknown> } = {};
      (editControlRequirements as any)(loop, { referenceChange: 5, maximumError: 1, settlingBand: .2, settlingTimeSeconds: 8, durationSeconds: 20 }, (next: Record<string, unknown>) => { holder.committed = next; }, { scenarioDefinitions: scenarios, processSymbol: "P1.ProcessValue", samplePeriodMs: 100 });
      const document: Document = view.dom.window.document;
      const id = document.querySelector<HTMLInputElement>("[data-engineer-scenario-id]")!;
      id.value = "edited"; id.dispatchEvent(new view.dom.window.Event("input", { bubbles: true }));
      (document.querySelector("[data-studio-editor-cancel]") as HTMLButtonElement).click();
      assert.strictEqual(holder.committed, undefined);
      (editControlRequirements as any)(loop, { referenceChange: 5, maximumError: 1, settlingBand: .2, settlingTimeSeconds: 8, durationSeconds: 20 }, (next: Record<string, unknown>) => { holder.committed = next; }, { scenarioDefinitions: scenarios, processSymbol: "P1.ProcessValue", samplePeriodMs: 100 });
      const second = (view.dom.window.document as Document).querySelector<HTMLInputElement>("[data-engineer-scenario-id]")!;
      second.value = "edited"; second.dispatchEvent(new view.dom.window.Event("input", { bubbles: true }));
      const advanced = (view.dom.window.document as Document).querySelector<HTMLTextAreaElement>("[data-engineer-scenario-advanced]");
      assert.ok(advanced, "advanced scenario fields remain available in the same editor");
      const typedText = JSON.stringify({ requirements: scenarios[0].requirements, effects: scenarios[0].effects, customTag: "retained" });
      advanced!.value = typedText;
      advanced!.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
      (view.dom.window.document.querySelector("[data-studio-editor-done]") as HTMLButtonElement).click();
      const saved = (holder as { committed?: Record<string, unknown> }).committed;
      assert.strictEqual((saved?.scenarioDefinitions as Array<{ id: string }>)[0].id, "edited");
      assert.strictEqual((saved?.scenarioDefinitions as Array<Record<string, unknown>>)[0].advanced, typedText, "the advanced text goes to the engine as typed");
      assert.strictEqual((saved?.scenarioDefinitions as Array<Record<string, unknown>>)[0].customTag, undefined, "the page merges nothing of it into the scenario");
    } finally { view.restore(); }
  });

  test("asks requirements only for the controlled output, not for an auxiliary measured output", () => {
    const view = fixture();
    try {
      const measured = { ...loop, schemaVersion: 2,
        signals: { ...loop.signals,
          setpoint: { symbol: "P1.Setpoint", address: "%ML0", iecType: "LREAL", direction: "read", unit: "°C", range: { minimum: 0, maximum: 80 } },
          additional: [{ id: "upstream", channel: "upstream", role: "state", symbol: "P1.Upstream", address: "%IW2", iecType: "REAL", direction: "read", unit: "°C", range: { minimum: 0, maximum: 80 } }] },
        channels: { inputs: [{ modelChannelId: "u", signalId: "manipulatedVariable", actuator: { kind: "continuous", minimum: 0, maximum: 100 } }],
          outputs: [{ modelChannelId: "upstream", signalId: "upstream" }, { modelChannelId: "downstream", signalId: "processVariable", setpointSignalId: "setpoint" }] } } as unknown as LoopDefinition;
      // The engine names the signal of each output channel (`liveSides`, B3.6): the auxiliary output and the controlled one. It lists the
      // controlled output alone as the editor's output (`requirementOutputs`, B4.3, C7): the page decides which outputs have requirements.
      setOpenedOffers({ liveSides: { inputs: [{ signalId: "manipulatedVariable", symbol: "P1.ManipulatedValue", unit: "%" }],
        outputs: [{ signalId: "upstream", symbol: "P1.Upstream", unit: "°C" }, { signalId: "processVariable", symbol: "P1.ProcessValue", unit: "°C" }] },
      requirementOutputs: [{ channelId: "downstream", signalId: "processVariable" }] });
      const holder: { committed?: Record<string, unknown> } = {};
      (editControlRequirements as any)(measured, { referenceChange: 5, maximumError: 1, settlingBand: .2, settlingTimeSeconds: 8, durationSeconds: 20 }, (next: Record<string, unknown>) => { holder.committed = next; });
      const document: Document = view.dom.window.document;
      const keys = [...document.querySelectorAll<HTMLInputElement>("input[data-studio-requirement]")].map(input => input.dataset.studioRequirement);
      // The visible rows are settling time, overshoot, steady-state error and the input disturbance (specification 35, B9.9);
      // the test design (setpoint change, duration, Auto-entry tolerance) sits under More….
      assert.deepStrictEqual(keys, ["settlingTimeSeconds", "settlingBand", "maximumOvershootPercent", "maximumError", "constantInputDisturbance", "referenceChange", "durationSeconds", "transferTolerance"],
        "no row for the auxiliary measured output; the input disturbance and Auto-entry tolerance belong to the controller output");
      (document.querySelector("[data-studio-editor-done]") as HTMLButtonElement).click();
      assert.strictEqual(holder.committed?.referenceChange, 5);
      assert.strictEqual(holder.committed?.channelRequirements, undefined);
    } finally { view.restore(); }
  });

  test("a blank scenario name is sent as typed, and the page shows no check and gives no name of its own", () => {
    const view = fixture();
    try {
      const holder: { committed?: Record<string, unknown> } = {};
      (editControlRequirements as any)(loop, { referenceChange: 5, maximumError: 1, settlingBand: .2, settlingTimeSeconds: 8, durationSeconds: 20 }, (next: Record<string, unknown>) => { holder.committed = next; }, { scenarioDefinitions: scenarios, processSymbol: "P1.ProcessValue", samplePeriodMs: 100 });
      const document: Document = view.dom.window.document;
      const id = document.querySelector<HTMLInputElement>("[data-engineer-scenario-id]")!;
      id.value = ""; id.dispatchEvent(new view.dom.window.Event("input", { bubbles: true }));
      (document.querySelector("[data-studio-editor-done]") as HTMLButtonElement).click();
      assert.strictEqual(document.querySelector("dialog[open]"), null, "Done committed: the page stopped nothing, because the engine names a scenario whose name is blank and the page does not");
      assert.strictEqual((holder.committed?.scenarioDefinitions as Array<{ id: string }>)[0].id, "", "the list goes to the engine as typed");
    } finally { view.restore(); }
  });

  test("adds the engine's template as it is, so the page names no scenario (B4.3)", () => {
    const view = fixture();
    setOpenedOffers({ scenarioTemplate: SCENARIO_TEMPLATE } as any);
    try {
      const holder: { committed?: Record<string, unknown> } = {};
      (editControlRequirements as any)(loop, REQUIREMENTS, (next: Record<string, unknown>) => { holder.committed = next; }, { scenarioDefinitions: [] });
      const document: Document = view.dom.window.document;
      const add = document.querySelector<HTMLButtonElement>("[data-engineer-scenario-add]")!;
      add.click(); add.click();
      assert.deepStrictEqual(Array.from(document.querySelectorAll<HTMLInputElement>("[data-engineer-scenario-id]"), input => input.value), ["", ""],
        "neither new scenario has a name: the engine names a scenario whose name is blank when the list changes");
      (document.querySelector("[data-studio-editor-done]") as HTMLButtonElement).click();
      const sent = holder.committed?.scenarioDefinitions as Array<Record<string, unknown>>;
      assert.deepStrictEqual(sent.map(item => item.id), ["", ""]);
      assert.deepStrictEqual(sent[0].requirements, SCENARIO_TEMPLATE.requirements, "each scenario is the engine's template");
    } finally { view.restore(); }
  });

  test("adds nothing until the engine's open answer names a template (B4.3)", () => {
    const view = fixture();
    try {
      (editControlRequirements as any)(loop, REQUIREMENTS, () => undefined, { scenarioDefinitions: [] });
      const document: Document = view.dom.window.document;
      document.querySelector<HTMLButtonElement>("[data-engineer-scenario-add]")!.click();
      assert.strictEqual(document.querySelectorAll("[data-engineer-scenario-id]").length, 0, "the page makes no scenario of its own");
    } finally { view.restore(); }
  });

  test("starts a new initial value and a new timed change at the signal the engine names, without checking it (B4.3)", () => {
    const view = fixture();
    setOpenedOffers({ scenarioRowSignal: "P1.Setpoint" });
    try {
      (editControlRequirements as any)(loop, REQUIREMENTS, () => undefined, { scenarioDefinitions: [{ ...scenarios[0], initialValues: {}, events: [] }] });
      const document: Document = view.dom.window.document;
      const values = () => Array.from(document.querySelectorAll<HTMLInputElement>("[data-engineer-scenario-initial-symbol]"), input => input.value);
      document.querySelector<HTMLButtonElement>("[data-engineer-scenario-add-value]")!.click();
      document.querySelector<HTMLButtonElement>("[data-engineer-scenario-add-value]")!.click();
      assert.deepStrictEqual(values(), ["P1.Setpoint", "P1.Setpoint"], "every new row starts at the engine's signal; the engine refuses a repeated one when the list changes");
      const addEvent = () => Array.from(document.querySelectorAll<HTMLButtonElement>("button")).find(button => button.textContent === "Add event")!;
      addEvent().click();
      const event = ["time", "symbol", "value"].map(part => document.querySelector<HTMLInputElement>(`[data-engineer-scenario-event-${part}]`)!.value);
      assert.deepStrictEqual(event, ["0", "P1.Setpoint", "0"]);
    } finally { view.restore(); }
  });

  test("leaves the signal of a new row blank when the engine names none (B4.3)", () => {
    const view = fixture();
    setOpenedOffers({ scenarioRowSignal: null });
    try {
      (editControlRequirements as any)(loop, REQUIREMENTS, () => undefined, { scenarioDefinitions: [{ ...scenarios[0], initialValues: {}, events: [] }] });
      const document: Document = view.dom.window.document;
      document.querySelector<HTMLButtonElement>("[data-engineer-scenario-add-value]")!.click();
      assert.deepStrictEqual(Array.from(document.querySelectorAll<HTMLInputElement>("[data-engineer-scenario-initial-symbol]"), input => input.value), [""]);
    } finally { view.restore(); }
  });

  test("sends the advanced text as typed, and the bounds rows give way to it (B4.3)", () => {
    const view = fixture();
    try {
      const holder: { committed?: Record<string, unknown> } = {};
      (editControlRequirements as any)(loop, REQUIREMENTS, (next: Record<string, unknown>) => { holder.committed = next; }, { scenarioDefinitions: scenarios });
      const document: Document = view.dom.window.document;
      const advanced = () => document.querySelector<HTMLTextAreaElement>("[data-engineer-scenario-advanced]")!;
      assert.strictEqual(advanced().value, JSON.stringify({ requirements: scenarios[0].requirements, effects: scenarios[0].effects }, null, 2),
        "the text starts as the scenario's other fields, formatted");
      assert.strictEqual(document.querySelectorAll("[data-engineer-scenario-minimum]").length, 1, "the bounds rows show until advanced text is typed");
      advanced().value = "{ not json";
      advanced().dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
      assert.strictEqual(advanced().value, "{ not json", "the typed text stays as it was typed");
      assert.strictEqual(document.querySelectorAll("[data-engineer-scenario-minimum]").length, 0, "the typed text replaces the bounds, so their rows are not shown");
      assert.strictEqual(document.querySelector("[data-engineer-scenario-error]")!.textContent, "", "the page does not read the text: the engine refuses one that is no JSON object");
      (document.querySelector("[data-studio-editor-done]") as HTMLButtonElement).click();
      assert.strictEqual((holder.committed?.scenarioDefinitions as Array<Record<string, unknown>>)[0].advanced, "{ not json");
    } finally { view.restore(); }
  });

  test("opens the Scenarios and More… sections to show the engine's refusal of the list (B4.3)", () => {
    const view = fixture();
    try {
      const sentence = "Scenario load-step writes an event at 150 ms, which is not a multiple of the 100 ms task period.";
      (editControlRequirements as any)(loop, REQUIREMENTS, () => undefined, { scenarioDefinitions: scenarios }, undefined, { refusal: { field: "scenarios", message: sentence } });
      const document: Document = view.dom.window.document;
      const error = document.querySelector<HTMLElement>("[data-engineer-scenario-error]")!;
      assert.strictEqual(error.textContent, sentence);
      assert.strictEqual(error.hidden, false);
      assert.ok(error.closest("details[data-studio-scenarios]")?.hasAttribute("open"), "the Scenarios section is open");
      assert.ok(error.closest("details.trust-compact-editor-more")?.hasAttribute("open"), "the More… section is open");
    } finally { view.restore(); }
  });

  /** Opens the Requirements dialog on `list`, and gives the dialog's document with a `done` that returns what Done sent. */
  function openOn(view: ReturnType<typeof fixture>, list: unknown[]) {
    const holder: { committed?: Record<string, unknown> } = {};
    (editControlRequirements as any)(loop, REQUIREMENTS, (next: Record<string, unknown>) => { holder.committed = next; }, { scenarioDefinitions: list });
    const document: Document = view.dom.window.document;
    const type = (field: Element, value: string) => { (field as HTMLInputElement).value = value; field.dispatchEvent(new view.dom.window.Event("input", { bubbles: true })); };
    const done = (): Array<Record<string, any>> => {
      (document.querySelector("[data-studio-editor-done]") as HTMLButtonElement).click();
      return holder.committed?.scenarioDefinitions as Array<Record<string, any>>;
    };
    return { document, type, done };
  }

  test("removes a scenario through its own control, and the list the engine receives no longer holds it", () => {
    const view = fixture();
    try {
      const { document, done } = openOn(view, [scenarios[0], { ...scenarios[0], id: "second" }]);
      assert.strictEqual(document.querySelectorAll("[data-engineer-scenario]").length, 2);
      document.querySelectorAll<HTMLButtonElement>("[data-engineer-scenario-remove]")[0].click();
      assert.deepStrictEqual(Array.from(document.querySelectorAll<HTMLInputElement>("[data-engineer-scenario-id]"), input => input.value), ["second"]);
      assert.deepStrictEqual(done().map(item => item.id), ["second"]);
    } finally { view.restore(); }
  });

  test("sends a repeated scenario name as typed, for the engine to refuse under scenarios (B4.3)", () => {
    const view = fixture();
    try {
      const { document, done } = openOn(view, [scenarios[0], { ...scenarios[0] }]);
      assert.strictEqual(document.querySelector<HTMLElement>("[data-engineer-scenario-error]")!.hidden, true, "the page names no repeated scenario; the engine does");
      assert.deepStrictEqual(done().map(item => item.id), ["load-step", "load-step"], "both scenarios go to the engine with the name the engineer typed");
    } finally { view.restore(); }
  });

  test("keeps focus and multi-character names while typing", () => {
    const view = fixture();
    try {
      const { document, type, done } = openOn(view, [scenarios[0]]);
      const name = document.querySelector<HTMLInputElement>("[data-engineer-scenario-id]")!;
      name.focus();
      for (const typed of ["l", "lo", "load", "load-2"]) type(name, typed);
      assert.strictEqual(document.querySelector("[data-engineer-scenario-id]"), name, "typing does not draw the card again");
      assert.strictEqual(document.activeElement, name, "the field keeps the focus");
      assert.strictEqual(name.value, "load-2");
      assert.deepStrictEqual(done().map(item => item.id), ["load-2"]);
    } finally { view.restore(); }
  });

  test("adds an initial value at the engine's signal beside the existing one, and sends the rows as the engineer typed them (B4.3)", () => {
    const view = fixture();
    setOpenedOffers({ scenarioRowSignal: "P1.Setpoint" });
    try {
      const { document, type, done } = openOn(view, [{ ...scenarios[0], events: [] }]);
      const symbols = () => Array.from(document.querySelectorAll<HTMLInputElement>("[data-engineer-scenario-initial-symbol]"), input => input.value);
      const numbers = () => Array.from(document.querySelectorAll<HTMLInputElement>("[data-engineer-scenario-initial]"), input => input.value);
      document.querySelector<HTMLButtonElement>("[data-engineer-scenario-add-value]")!.click();
      assert.deepStrictEqual(symbols(), ["P1.ManipulatedValue", "P1.Setpoint"], "the new row is added beside the existing one");
      assert.deepStrictEqual(numbers(), ["20", "0"], "the existing value is not overwritten");
      type(document.querySelectorAll("[data-engineer-scenario-initial-symbol]")[1], "P1.Other");
      type(document.querySelectorAll("[data-engineer-scenario-initial]")[1], "1e-3");
      assert.deepStrictEqual(done()[0].initialValues, [{ symbol: "P1.ManipulatedValue", value: "20" }, { symbol: "P1.Other", value: "1e-3" }],
        "a renamed signal keeps its row, and a number is sent as the text typed");
    } finally { view.restore(); }
  });

  test("renaming an initial value to an existing symbol sends both rows, for the engine to refuse (B4.3)", () => {
    const view = fixture();
    setOpenedOffers({ scenarioRowSignal: "P1.Setpoint" });
    try {
      const { document, type, done } = openOn(view, [{ ...scenarios[0], events: [] }]);
      document.querySelector<HTMLButtonElement>("[data-engineer-scenario-add-value]")!.click();
      type(document.querySelectorAll("[data-engineer-scenario-initial-symbol]")[1], "P1.ManipulatedValue");
      assert.deepStrictEqual(done()[0].initialValues, [{ symbol: "P1.ManipulatedValue", value: "20" }, { symbol: "P1.ManipulatedValue", value: "0" }],
        "the page overwrites no value; both rows reach the engine");
    } finally { view.restore(); }
  });

  test("sends an empty number as it is typed (B4.3)", () => {
    const view = fixture();
    try {
      const { document, type, done } = openOn(view, [scenarios[0]]);
      type(document.querySelector("[data-engineer-scenario-initial]")!, "");
      type(document.querySelector("[data-engineer-scenario-event-time]")!, "");
      const sent = done()[0];
      assert.deepStrictEqual(sent.initialValues, [{ symbol: "P1.ManipulatedValue", value: "" }]);
      assert.deepStrictEqual(sent.events, [{ atMs: "", symbol: "P1.ManipulatedValue", value: "35" }]);
    } finally { view.restore(); }
  });
});
