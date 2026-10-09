import * as assert from "assert";
import { editControlRequirements } from "../../controlStudio/webview/threePageRequirements";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

const loop = {
  schemaVersion: 1, id: "scenario-loop", name: "Scenario loop", revision: "r", projectId: "p", runtimeId: "sim",
  controller: { instance: "P1.Controller", type: "Control.PID", revision: "r", action: "direct", taskPeriodMs: 100 },
  signals: {
    manipulatedVariable: { symbol: "P1.ManipulatedValue", address: "%QW0", iecType: "REAL", direction: "write", unit: "%", range: { minimum: 0, maximum: 100 } },
    processVariable: { symbol: "P1.ProcessValue", address: "%IW0", iecType: "REAL", direction: "read", unit: "°C", range: { minimum: 0, maximum: 80 } },
  },
  limits: { outputMinimum: 0, outputMaximum: 100 }, observedSampleTiming: { nominalPeriodMs: 100 },
} as unknown as ControlLoopDefinition;

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

suite("Control Studio three-page scenario requirements", () => {
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
      advanced!.value = JSON.stringify({ requirements: scenarios[0].requirements, effects: scenarios[0].effects, customTag: "retained" });
      advanced!.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
      (view.dom.window.document.querySelector("[data-studio-editor-done]") as HTMLButtonElement).click();
      const saved = (holder as { committed?: Record<string, unknown> }).committed;
      assert.strictEqual((saved?.scenarioDefinitions as Array<{ id: string }>)[0].id, "edited");
      assert.strictEqual((saved?.scenarioDefinitions as Array<Record<string, unknown>>)[0].customTag, "retained");
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
          outputs: [{ modelChannelId: "upstream", signalId: "upstream" }, { modelChannelId: "downstream", signalId: "processVariable", setpointSignalId: "setpoint" }] } } as unknown as ControlLoopDefinition;
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

  test("reuses one validation message when malformed scenario input is submitted repeatedly", () => {
    const view = fixture();
    try {
      (editControlRequirements as any)(loop, { referenceChange: 5, maximumError: 1, settlingBand: .2, settlingTimeSeconds: 8, durationSeconds: 20 }, () => undefined, { scenarioDefinitions: scenarios, processSymbol: "P1.ProcessValue", samplePeriodMs: 100 });
      const document: Document = view.dom.window.document;
      const id = document.querySelector<HTMLInputElement>("[data-engineer-scenario-id]")!;
      id.value = ""; id.dispatchEvent(new view.dom.window.Event("input", { bubbles: true }));
      const done = document.querySelector<HTMLButtonElement>("[data-studio-editor-done]")!;
      done.click(); done.click();
      assert.strictEqual(document.querySelectorAll("[data-compact-scenario-error]").length, 1);
      assert.ok(document.querySelector("[data-compact-scenario-error]")?.textContent);
    } finally { view.restore(); }
  });
});
