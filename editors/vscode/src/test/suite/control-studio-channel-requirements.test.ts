import * as assert from "assert";
import { editControlRequirements } from "../../controlStudio/webview/threePageRequirements";
import { setOpenedOffers } from "../../controlStudio/webview/openedOffers";
import type { LoopDefinition } from "../../controlStudio/generated/OpenResult";

const { JSDOM } = require("jsdom");

function multichannelLoop(): LoopDefinition {
  // A partial loop: the page reads these fields; the generated type lists every field the engine sends.
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
  } as unknown as LoopDefinition;
}

/** The signals the engine names for the loop's channels (`liveSides` of the open answer, B3.6): each output row takes its variable and unit from them. */
const LIVE_SIDES = { inputs: [{ signalId: "manipulatedVariable", symbol: "P1.MV", unit: "%" }],
  outputs: [{ signalId: "processVariable", symbol: "P1.PV", unit: "degC" }, { signalId: "processVariable2", symbol: "P1.PV2", unit: "bar" }] };

/**
 * The controlled outputs the engine lists for the loop (`requirementOutputs` of the open answer, B4.3, C7), each with what its rows start from:
 * the first output's requirements are the loop's declared ones for that channel, and the second output has none.
 */
const REQUIREMENT_OUTPUTS = [
  { channelId: "output-1", signalId: "processVariable", requirements: { referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8 } },
  { channelId: "output-2", signalId: "processVariable2" },
];

suite("Control Studio per-output requirements", () => {
  setup(() => setOpenedOffers({ liveSides: LIVE_SIDES, requirementOutputs: REQUIREMENT_OUTPUTS }));
  teardown(() => setOpenedOffers({}));

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
      // Each output has its own settling time, overshoot, steady-state error and test setpoint change rows (specification 35, B9.9).
      assert.strictEqual(fields.filter(field => field.dataset.studioSignal === "P1.PV").length, 5);
      assert.strictEqual(fields.filter(field => field.dataset.studioSignal === "P1.PV2").length, 5);
      assert.ok(fields.filter(field => !["durationSeconds", "transferTolerance", "constantInputDisturbance"].includes(field.dataset.studioRequirement ?? "")).every(field => field.dataset.studioSignal === "P1.PV" || field.dataset.studioSignal === "P1.PV2"),
        "every per-output requirement names its own output");
      // The role and unit are the visible text; the symbol is the hover text (specification 35, B9.9).
      assert.ok(document.querySelector('span[title="P1.PV2"]'), "the second output's symbol is its hover text");
      assert.match(document.querySelector('span[title="P1.PV2"]')?.textContent ?? "", /Measurement 2 \(bar\)/);
      assert.match(document.body.textContent ?? "", /bar/);
      assert.strictEqual(document.querySelector<HTMLInputElement>("[data-studio-requirement='referenceChange:output-2']")?.value, "", "second output starts blank");
      // The first output's rows start from what the engine offered for it (its declared requirements), not from a copy the page makes.
      assert.strictEqual(document.querySelector<HTMLInputElement>("[data-studio-requirement='settlingBand:output-1']")?.value, "0.2");
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
      // The page sends what was typed: the rows of each output under its channel, in the editor's order. A duration the engineer changed goes as
      // the text typed, which the engine reads (specification 35, Durations and B4.3).
      assert.deepStrictEqual(committed?.outputRequirements, [
        { channel: "output-1", referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: "8" },
        { channel: "output-2", referenceChange: 2, maximumError: 0.5, settlingBand: 0.1, settlingTimeSeconds: "6" },
      ]);
      // The shared values travel beside them.
      assert.strictEqual(committed.durationSeconds, "16");
      assert.strictEqual(committed.constantInputDisturbance, 3);
      assert.strictEqual(committed.measurementNoiseBand, 0.1);
      assert.strictEqual(committed.gainUncertaintyFraction, 0.2);
      assert.strictEqual(committed.transferTolerance, 0.05);
      // The engine assembles the record (B4.3): the page sends no record of each output, no copy of the first output and no channel-suffixed row name.
      assert.strictEqual(committed.channelRequirements, undefined, "the engine writes each output's record");
      assert.strictEqual(committed.referenceChange, undefined, "the engine writes the first output's copy");
      assert.deepStrictEqual(Object.keys(committed).filter(key => key.includes(":")), [], "the commit carries no channel-suffixed key");
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
      // The engine says that a declared notch needs the band (B4.3); the editor shows the band when it is told so.
      editControlRequirements(loop, { referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8, durationSeconds: 16 }, () => undefined, undefined, undefined, { measurementNoiseBandRequired: true });
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
      // The page knows no output span: it posts a tolerance beyond it, and the engine refuses it under its field (B4.3).
      assert.strictEqual(edit(base, "150").transferTolerance, 150, "a tolerance beyond the output span is posted for the engine to refuse");
      assert.strictEqual(document.body.textContent?.includes("Use 100 or less"), false, "the page shows no span refusal of its own");
    } finally {
      dom.window.HTMLDialogElement.prototype.showModal = showModal;
      dom.window.HTMLDialogElement.prototype.close = close;
      (globalThis as { document?: Document }).document = previousDocument;
      (globalThis as { window?: Window }).window = previousWindow;
      (globalThis as any).HTMLElement = previousHTMLElement;
      dom.window.close();
    }
  });

  test("posts an out-of-range output value, and shows the engine's refusal under the field it names", () => {
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
    const base = { referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8, durationSeconds: 16 };
    try {
      let committed: any;
      editControlRequirements(multichannelLoop(), base, next => { committed = next; });
      const values: Record<string, string> = {
        "referenceChange:output-1": "5", "maximumError:output-1": "1", "settlingBand:output-1": "0.2", "settlingTimeSeconds:output-1": "8",
        "referenceChange:output-2": "2", "maximumError:output-2": "0.5", "settlingBand:output-2": "-0.1", "settlingTimeSeconds:output-2": "6", durationSeconds: "16",
      };
      for (const [key, value] of Object.entries(values)) document.querySelector<HTMLInputElement>(`[data-studio-requirement='${key}']`)!.value = value;
      document.querySelector<HTMLButtonElement>("[data-studio-editor-done]")!.click();
      // The page checks no band's range, so the negative band is posted with the second output's rows, for the engine to refuse (B4.3).
      assert.deepStrictEqual(committed.outputRequirements[1], { channel: "output-2", referenceChange: 2, maximumError: 0.5, settlingBand: -0.1, settlingTimeSeconds: "6" });
      // The engine's refusal of that value reopens the editor, and its sentence shows under the field the refusal names. The page still holds
      // the change it posted, so the editor shows the rows as they were typed.
      editControlRequirements(multichannelLoop(), committed, () => undefined, undefined, undefined,
        { refusal: { field: "settlingBand:output-2", message: "Enter a positive settling band." } });
      const band = document.querySelector<HTMLInputElement>("[data-studio-requirement='settlingBand:output-2']")!;
      assert.strictEqual(band.getAttribute("aria-invalid"), "true", "the refused field is marked");
      assert.strictEqual(document.getElementById(band.getAttribute("aria-describedby") ?? "")?.textContent, "Enter a positive settling band.");
      assert.strictEqual(band.value, "-0.1", "the refused value stays in its field for the engineer to correct");
      assert.strictEqual(document.querySelector<HTMLInputElement>("[data-studio-requirement='referenceChange:output-2']")?.value, "2");
    } finally {
      dom.window.HTMLDialogElement.prototype.showModal = showModal;
      dom.window.HTMLDialogElement.prototype.close = close;
      (globalThis as { document?: Document }).document = previousDocument;
      (globalThis as { window?: Window }).window = previousWindow;
      (globalThis as any).HTMLElement = previousHTMLElement;
      dom.window.close();
    }
  });

  test("starts each output's rows from the record the engine answered, else from what it offered for the output", () => {
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
      const row = (key: string) => document.querySelector<HTMLInputElement>(`[data-studio-requirement='${key}']`)?.value;
      // The engine recorded the second output's rows: they win over the page's offer for that output, and the first output, which the
      // engine's record does not name, starts from what the engine offered for it.
      editControlRequirements(multichannelLoop(), {
        referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8, durationSeconds: 16,
        channelRequirements: { "output-2": { referenceChange: 2, maximumError: 0.5, settlingBand: 0.1, settlingTimeSeconds: 6, maximumOvershootPercent: 4 } },
      }, () => undefined);
      assert.strictEqual(row("settlingBand:output-2"), "0.1", "the engine's record of the second output");
      assert.strictEqual(row("settlingTimeSeconds:output-2"), "6 s");
      assert.strictEqual(row("maximumOvershootPercent:output-2"), "4");
      assert.strictEqual(row("settlingBand:output-1"), "0.2", "the first output starts from the engine's offer");
      // Without an offer for an output and without a record, its rows are blank: the page copies no other output's values into them.
      setOpenedOffers({ liveSides: LIVE_SIDES, requirementOutputs: [{ channelId: "output-1", signalId: "processVariable" }, { channelId: "output-2", signalId: "processVariable2" }] });
      editControlRequirements(multichannelLoop(), { referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8, durationSeconds: 16 }, () => undefined);
      assert.strictEqual(row("settlingBand:output-1"), "");
      assert.strictEqual(row("settlingBand:output-2"), "");
    } finally {
      dom.window.HTMLDialogElement.prototype.showModal = showModal;
      (globalThis as { document?: Document }).document = previousDocument;
      (globalThis as { window?: Window }).window = previousWindow;
      (globalThis as any).HTMLElement = previousHTMLElement;
      dom.window.close();
    }
  });

  test("lists the outputs the engine names, so an output it does not list has no rows", () => {
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
      // The engine lists the second output alone (the first measures only): the editor shows its rows as the rows of the one controlled output.
      setOpenedOffers({ liveSides: LIVE_SIDES, requirementOutputs: [{ channelId: "output-2", signalId: "processVariable2" }] });
      editControlRequirements(multichannelLoop(), { referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8, durationSeconds: 16 }, () => undefined);
      const keys = [...document.querySelectorAll<HTMLInputElement>("input[data-studio-requirement]")].map(input => input.dataset.studioRequirement);
      assert.ok(keys.includes("settlingBand") && !keys.some(key => key?.includes(":")), `one controlled output has plain rows: ${keys.join(", ")}`);
      assert.ok(document.querySelector(".trust-compact-editor-no-signal"), "one controlled output has no Signal column");
      // The engine lists none (a loop without channels): the editor shows the loop's own rows.
      setOpenedOffers({ liveSides: LIVE_SIDES, requirementOutputs: [] });
      editControlRequirements(multichannelLoop(), { referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8, durationSeconds: 16 }, () => undefined);
      assert.strictEqual(document.querySelector<HTMLInputElement>("[data-studio-requirement='settlingBand']")?.value, "0.2");
    } finally {
      dom.window.HTMLDialogElement.prototype.showModal = showModal;
      (globalThis as { document?: Document }).document = previousDocument;
      (globalThis as { window?: Window }).window = previousWindow;
      (globalThis as any).HTMLElement = previousHTMLElement;
      dom.window.close();
    }
  });
});
