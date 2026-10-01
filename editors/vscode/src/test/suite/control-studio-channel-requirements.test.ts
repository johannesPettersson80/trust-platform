import * as assert from "assert";
import { editControlRequirements } from "../../controlStudio/webview/threePageRequirements";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";

const { JSDOM } = require("jsdom");

function multichannelLoop(): ControlLoopDefinition {
  return {
    schemaVersion: 1, id: "requirements", name: "Requirements", revision: "1", projectId: "test", runtimeId: "test",
    controller: { instance: "P1.Controller", type: "Control.PID", revision: "1", action: "direct", taskPeriodMs: 100 },
    observedSampleTiming: { nominalPeriodMs: 100 },
    signals: {
      manipulatedVariable: { symbol: "P1.MV", address: "%QW0", iecType: "REAL", direction: "write", unit: "%", range: { minimum: 0, maximum: 100 } },
      processVariable: { symbol: "P1.PV", address: "%IW0", iecType: "REAL", direction: "read", unit: "degC", range: { minimum: 20, maximum: 80 } },
      setpoint: { symbol: "P1.SP", address: "%QW1", iecType: "REAL", direction: "write", unit: "degC", range: { minimum: 20, maximum: 80 } },
      additional: [
        { id: "processVariable2", channel: "output-2", role: "PV", symbol: "P1.PV2", address: "%IW1", iecType: "REAL", direction: "read", unit: "bar", range: { minimum: 2, maximum: 8 } },
        { id: "setpoint2", channel: "output-2", role: "SP", symbol: "P1.SP2", address: "%QW2", iecType: "REAL", direction: "write", unit: "bar", range: { minimum: 2, maximum: 8 } },
      ],
    },
    channels: {
      inputs: [{ modelChannelId: "input-1", signalId: "manipulatedVariable", actuator: { kind: "continuous", minimum: 0, maximum: 100 } }],
      outputs: [
        { modelChannelId: "output-1", signalId: "processVariable", setpointSignalId: "setpoint", requirements: { referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8 } },
        { modelChannelId: "output-2", signalId: "processVariable2", setpointSignalId: "setpoint2" },
      ],
    },
    limits: { outputMinimum: 0, outputMaximum: 100 },
    experiment: { kind: "observation", execution: "control-studio", amplitude: 10, durationSeconds: 20, seed: 1 },
  };
}

suite("Control Studio per-output requirements", () => {
  test("shows each output signal and unit without copying the first target", () => {
    const dom = new JSDOM("<main></main>");
    const document = dom.window.document as Document;
    const previousDocument = (globalThis as { document?: Document }).document;
    const previousHTMLElement = (globalThis as any).HTMLElement;
    (globalThis as any).HTMLElement = dom.window.HTMLElement;
    const previousWindow = (globalThis as { window?: Window }).window;
    (globalThis as { document?: Document }).document = document;
    (globalThis as { window?: Window }).window = dom.window;
    const showModal = dom.window.HTMLDialogElement.prototype.showModal;
    dom.window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
    try {
      editControlRequirements(multichannelLoop(), {
        referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8, durationSeconds: 16,
      }, () => undefined);
      const fields = [...document.querySelectorAll<HTMLInputElement>("[data-studio-requirement]")];
      assert.strictEqual(fields.filter(field => field.dataset.studioSignal === "P1.PV").length, 4);
      assert.strictEqual(fields.filter(field => field.dataset.studioSignal === "P1.PV2").length, 4);
      assert.ok(fields.filter(field => !["durationSeconds", "transferTolerance", "constantInputDisturbance"].includes(field.dataset.studioRequirement ?? "")).every(field => field.dataset.studioSignal === "P1.PV" || field.dataset.studioSignal === "P1.PV2"),
        "every per-output requirement names its own output");
      assert.match(document.body.textContent ?? "", /P1\.PV2/);
      assert.match(document.body.textContent ?? "", /bar/);
      assert.strictEqual(document.querySelector<HTMLInputElement>("[data-studio-requirement='referenceChange:output-2']")?.value, "", "second output starts blank");
    } finally {
      dom.window.HTMLDialogElement.prototype.showModal = showModal;
      (globalThis as { document?: Document }).document = previousDocument;
      (globalThis as { window?: Window }).window = previousWindow;
      (globalThis as any).HTMLElement = previousHTMLElement;
      dom.window.close();
    }
  });

  test("commits ordered channel requirements while preserving existing scalar constraints", () => {
    const dom = new JSDOM("<main></main>");
    const document = dom.window.document as Document;
    const previousDocument = (globalThis as { document?: Document }).document;
    const previousHTMLElement = (globalThis as any).HTMLElement;
    (globalThis as any).HTMLElement = dom.window.HTMLElement;
    const previousWindow = (globalThis as { window?: Window }).window;
    (globalThis as { document?: Document }).document = document;
    (globalThis as { window?: Window }).window = dom.window;
    const showModal = dom.window.HTMLDialogElement.prototype.showModal;
    const close = dom.window.HTMLDialogElement.prototype.close;
    dom.window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
    dom.window.HTMLDialogElement.prototype.close = function () { this.open = false; this.dispatchEvent(new dom.window.Event("close")); };
    try {
      const current = { referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8, durationSeconds: 16,
        constantInputDisturbance: 3, measurementNoiseBand: 0.1, gainUncertaintyFraction: 0.2, transferTolerance: 0.05, channelRequirements: {} } as any;
      let committed: any;
      editControlRequirements(multichannelLoop(), current, next => { committed = next; });
      const values: Record<string, string> = {
        "referenceChange:output-1": "5", "maximumError:output-1": "1", "settlingBand:output-1": "0.2", "settlingTimeSeconds:output-1": "8",
        "referenceChange:output-2": "2", "maximumError:output-2": "0.5", "settlingBand:output-2": "0.1", "settlingTimeSeconds:output-2": "6", durationSeconds: "16",
      };
      for (const [key, value] of Object.entries(values)) {
        const field = document.querySelector<HTMLInputElement>(`[data-studio-requirement='${key}']`);
        if (field) field.value = value;
      }
      document.querySelector<HTMLButtonElement>("[data-studio-editor-done]")!.click();
      assert.ok(committed?.channelRequirements, "Done retains each output requirement set");
      assert.deepStrictEqual(Object.keys(committed.channelRequirements), ["output-1", "output-2"]);
      assert.deepStrictEqual(committed.channelRequirements["output-2"], { referenceChange: 2, maximumError: 0.5, settlingBand: 0.1, settlingTimeSeconds: 6 });
      assert.strictEqual(committed.constantInputDisturbance, 3);
      assert.strictEqual(committed.measurementNoiseBand, 0.1);
      assert.strictEqual(committed.gainUncertaintyFraction, 0.2);
      assert.strictEqual(committed.transferTolerance, 0.05);
    } finally {
      dom.window.HTMLDialogElement.prototype.showModal = showModal;
      dom.window.HTMLDialogElement.prototype.close = close;
      (globalThis as { document?: Document }).document = previousDocument;
      (globalThis as { window?: Window }).window = previousWindow;
      (globalThis as any).HTMLElement = previousHTMLElement;
      dom.window.close();
    }
  });

  test("offers an optional input disturbance in output units, blank declaring no load trial", () => {
    const dom = new JSDOM("<main></main>");
    const document = dom.window.document as Document;
    const previousDocument = (globalThis as { document?: Document }).document;
    const previousHTMLElement = (globalThis as any).HTMLElement;
    (globalThis as any).HTMLElement = dom.window.HTMLElement;
    const previousWindow = (globalThis as { window?: Window }).window;
    (globalThis as { document?: Document }).document = document;
    (globalThis as { window?: Window }).window = dom.window;
    const showModal = dom.window.HTMLDialogElement.prototype.showModal;
    const close = dom.window.HTMLDialogElement.prototype.close;
    dom.window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
    dom.window.HTMLDialogElement.prototype.close = function () { this.open = false; this.dispatchEvent(new dom.window.Event("close")); };
    const edit = (current: any, disturbance: string) => {
      let committed: any;
      editControlRequirements(multichannelLoop(), current, next => { committed = next; });
      const values: Record<string, string> = {
        "referenceChange:output-1": "5", "maximumError:output-1": "1", "settlingBand:output-1": "0.2", "settlingTimeSeconds:output-1": "8",
        "referenceChange:output-2": "2", "maximumError:output-2": "0.5", "settlingBand:output-2": "0.1", "settlingTimeSeconds:output-2": "6", durationSeconds: "16",
        constantInputDisturbance: disturbance,
      };
      for (const [key, value] of Object.entries(values)) document.querySelector<HTMLInputElement>(`[data-studio-requirement='${key}']`)!.value = value;
      document.querySelector<HTMLButtonElement>("[data-studio-editor-done]")!.click();
      return committed;
    };
    try {
      const base = { referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8, durationSeconds: 16 };
      editControlRequirements(multichannelLoop(), base, () => undefined);
      const field = document.querySelector<HTMLInputElement>("[data-studio-requirement='constantInputDisturbance']");
      assert.ok(field, "Requirements offers an input disturbance although none is declared yet");
      assert.strictEqual(field!.dataset.studioSignal, "P1.MV", "the disturbance is added to the controller output's plant input");
      assert.strictEqual(field!.value, "");
      assert.strictEqual(field!.placeholder, "None");
      assert.match(document.querySelector(`label[for='${field!.id}']`)?.textContent ?? "", /^Input disturbance \(%\)$/);
      document.querySelector<HTMLButtonElement>("[data-studio-editor-cancel]")!.click();
      assert.strictEqual("constantInputDisturbance" in edit(base, ""), false, "blank declares no load trial");
      assert.strictEqual("constantInputDisturbance" in edit({ ...base, constantInputDisturbance: 3 }, ""), false, "clearing the field removes the load trial");
      assert.strictEqual(edit(base, "-2").constantInputDisturbance, -2, "a load can pull against the output");
    } finally {
      dom.window.HTMLDialogElement.prototype.showModal = showModal;
      dom.window.HTMLDialogElement.prototype.close = close;
      (globalThis as { document?: Document }).document = previousDocument;
      (globalThis as { window?: Window }).window = previousWindow;
      (globalThis as any).HTMLElement = previousHTMLElement;
      dom.window.close();
    }
  });

  test("offers the measurement noise a declared notch needs", () => {
    const dom = new JSDOM("<main></main>");
    const document = dom.window.document as Document;
    const previousDocument = (globalThis as { document?: Document }).document;
    const previousHTMLElement = (globalThis as any).HTMLElement;
    (globalThis as any).HTMLElement = dom.window.HTMLElement;
    const previousWindow = (globalThis as { window?: Window }).window;
    (globalThis as { document?: Document }).document = document;
    (globalThis as { window?: Window }).window = dom.window;
    const showModal = dom.window.HTMLDialogElement.prototype.showModal;
    dom.window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
    try {
      const loop: any = multichannelLoop();
      loop.controller = { ...loop.controller, project: { components: [{ kind: "notch", convention: "direct-form-minus-feedback",
        symbols: ["P1.B0", "P1.B1", "P1.B2", "P1.A1", "P1.A2"], frequenciesHz: [3], qualityFactors: [1] }] } };
      editControlRequirements(loop, { referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8, durationSeconds: 16 }, () => undefined);
      const field = document.querySelector<HTMLInputElement>("[data-studio-requirement='measurementNoiseBand']");
      assert.ok(field, "a loop with a notch needs a measurement-noise band, so Requirements must offer it");
      assert.match(document.querySelector(`label[for='${field!.id}']`)?.textContent ?? "", /^Measurement noise \(±°C\)$/);
    } finally {
      dom.window.HTMLDialogElement.prototype.showModal = showModal;
      (globalThis as { document?: Document }).document = previousDocument;
      (globalThis as { window?: Window }).window = previousWindow;
      (globalThis as any).HTMLElement = previousHTMLElement;
      dom.window.close();
    }
  });

  test("offers the Auto-entry tolerance as an optional reviewed limit in output units", () => {
    const dom = new JSDOM("<main></main>");
    const document = dom.window.document as Document;
    const previousDocument = (globalThis as { document?: Document }).document;
    const previousHTMLElement = (globalThis as any).HTMLElement;
    (globalThis as any).HTMLElement = dom.window.HTMLElement;
    const previousWindow = (globalThis as { window?: Window }).window;
    (globalThis as { document?: Document }).document = document;
    (globalThis as { window?: Window }).window = dom.window;
    const showModal = dom.window.HTMLDialogElement.prototype.showModal;
    const close = dom.window.HTMLDialogElement.prototype.close;
    dom.window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
    dom.window.HTMLDialogElement.prototype.close = function () { this.open = false; this.dispatchEvent(new dom.window.Event("close")); };
    const edit = (current: any, tolerance: string) => {
      let committed: any;
      editControlRequirements(multichannelLoop(), current, next => { committed = next; });
      const values: Record<string, string> = {
        "referenceChange:output-1": "5", "maximumError:output-1": "1", "settlingBand:output-1": "0.2", "settlingTimeSeconds:output-1": "8",
        "referenceChange:output-2": "2", "maximumError:output-2": "0.5", "settlingBand:output-2": "0.1", "settlingTimeSeconds:output-2": "6", durationSeconds: "16",
        transferTolerance: tolerance,
      };
      for (const [key, value] of Object.entries(values)) document.querySelector<HTMLInputElement>(`[data-studio-requirement='${key}']`)!.value = value;
      document.querySelector<HTMLButtonElement>("[data-studio-editor-done]")!.click();
      return committed;
    };
    try {
      const base = { referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8, durationSeconds: 16 };
      editControlRequirements(multichannelLoop(), base, () => undefined);
      const field = document.querySelector<HTMLInputElement>("[data-studio-requirement='transferTolerance']");
      assert.ok(field, "Requirements offers the Auto-entry tolerance");
      assert.strictEqual(field!.dataset.studioSignal, "P1.MV", "the tolerance applies to the controller output");
      assert.strictEqual(field!.value, "", "no tolerance is reviewed yet");
      assert.strictEqual(field!.placeholder, "Automatic");
      assert.match(document.querySelector(`label[for='${field!.id}']`)?.textContent ?? "", /^Auto-entry tolerance \(%\)$/);
      document.querySelector<HTMLButtonElement>("[data-studio-editor-cancel]")!.click();
      assert.strictEqual("transferTolerance" in edit(base, ""), false, "a blank tolerance keeps the automatic allowance");
      assert.strictEqual("transferTolerance" in edit({ ...base, transferTolerance: 0.05 }, ""), false, "clearing the field removes the reviewed tolerance");
      assert.strictEqual(edit(base, "0.5").transferTolerance, 0.5);
      assert.strictEqual(edit(base, "150"), undefined, "a tolerance beyond the 0-100 % output span is refused");
      assert.match(document.body.textContent ?? "", /Use 100 or less/);
    } finally {
      dom.window.HTMLDialogElement.prototype.showModal = showModal;
      dom.window.HTMLDialogElement.prototype.close = close;
      (globalThis as { document?: Document }).document = previousDocument;
      (globalThis as { window?: Window }).window = previousWindow;
      (globalThis as any).HTMLElement = previousHTMLElement;
      dom.window.close();
    }
  });
});
