import * as assert from "assert";
import { acquisitionPreferences, editExperimentLimits, type StudioExperimentLimits } from "../../controlStudio/webview/threePageLimits";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

suite("Control Studio acquisition limits", () => {
  /** The engine's default limits of the multichannel loop (B3.2), which the Limits editor starts from. */
  function multichannelLimits(): StudioExperimentLimits {
    return { durationSeconds: "20 s",
      inputs: [{ symbol: "P1.MV", unit: "%", minimum: 10, maximum: 80 }, { symbol: "P1.MV2", unit: "%", minimum: 20, maximum: 70 }],
      outputs: [{ symbol: "P1.PV", unit: "degC", minimum: 30, maximum: 70 }, { symbol: "P1.PV2", unit: "degC", minimum: 35, maximum: 65 }],
      amplitude: 10 };
  }

  test("renders every selected channel with its signal and unit and commits ordered limits", () => {
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
    try {
      let committed: any;
      editExperimentLimits(multichannelLimits(), next => { committed = next; });
      const fields = [...(dom.window.document as Document).querySelectorAll<HTMLInputElement>("[data-studio-limit]")];
      assert.strictEqual(fields.filter(field => field.dataset.studioSignal === "P1.MV").length, 2);
      assert.strictEqual(fields.filter(field => field.dataset.studioSignal === "P1.MV2").length, 2);
      assert.strictEqual(fields.filter(field => field.dataset.studioSignal === "P1.PV").length, 2);
      assert.strictEqual(fields.filter(field => field.dataset.studioSignal === "P1.PV2").length, 2);
      assert.match(dom.window.document.body.textContent ?? "", /P1\.MV2.*%/s);
      assert.match(dom.window.document.body.textContent ?? "", /P1\.PV2.*°C/s);
      const done = (dom.window.document as Document).querySelector<HTMLButtonElement>("[data-studio-editor-done]")!; done.click();
      assert.deepStrictEqual(committed.inputs.map((signal: { symbol: string }) => signal.symbol), ["P1.MV", "P1.MV2"]);
      assert.deepStrictEqual(committed.outputs.map((signal: { symbol: string }) => signal.symbol), ["P1.PV", "P1.PV2"]);
    } finally {
      dom.window.HTMLDialogElement.prototype.showModal = showModal;
      dom.window.HTMLDialogElement.prototype.close = close;
      (globalThis as any).document = previousDocument;
      (globalThis as any).window = previousWindow;
      (globalThis as any).HTMLElement = previousHTMLElement;
      dom.window.close();
    }
  });

  test("shows the engine's words for the duration and sends back what was typed: 89.999 s is never stored as 90 s (B9.9)", () => {
    const dom = new JSDOM("<main></main>");
    const document = dom.window.document as Document;
    const previous = { document: (globalThis as any).document, window: (globalThis as any).window, HTMLElement: (globalThis as any).HTMLElement };
    Object.assign(globalThis, { document, window: dom.window, HTMLElement: dom.window.HTMLElement });
    const showModal = dom.window.HTMLDialogElement.prototype.showModal;
    const close = dom.window.HTMLDialogElement.prototype.close;
    dom.window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
    dom.window.HTMLDialogElement.prototype.close = function () { this.open = false; this.dispatchEvent(new dom.window.Event("close")); };
    try {
      const duration = () => document.querySelector<HTMLInputElement>("[data-studio-limit='durationSeconds']")!;
      const edit = (limits: StudioExperimentLimits, type?: string): StudioExperimentLimits => {
        let committed: any;
        editExperimentLimits(limits, next => { committed = next; });
        if (type !== undefined) duration().value = type;
        const shown = duration().value;
        document.querySelector<HTMLButtonElement>("[data-studio-editor-done]")!.click();
        assert.strictEqual(shown, type ?? limits.durationSeconds, "the duration is shown as the page holds it");
        return committed;
      };
      // The engine's words read back as exactly the duration (Rust: the_limits_answer_writes_each_duration_in_words_that_read_back_exactly):
      // left alone, they go back as they are.
      const engine: StudioExperimentLimits = { ...multichannelLimits(), durationSeconds: "89.999 s" };
      assert.strictEqual(edit(engine).durationSeconds, "89.999 s");
      // What the engineer types goes back as typed, and the editor shows it as typed when it opens again. Wrong behaviour caught: a page that
      // formatted the plain digits again showed "90 s" here and sent it, which stored 90 where the engineer typed 89.999.
      const typed = edit(engine, "89.999");
      assert.strictEqual(typed.durationSeconds, "89.999");
      assert.strictEqual(edit(typed).durationSeconds, "89.999");
      assert.strictEqual(acquisitionPreferences("automatic", typed).durationSeconds, "89.999");
    } finally {
      dom.window.HTMLDialogElement.prototype.showModal = showModal;
      dom.window.HTMLDialogElement.prototype.close = close;
      Object.assign(globalThis, previous);
      dom.window.close();
    }
  });

  test("commits a blank entry as the text typed, and the request sends it so: the engine refuses it under its field", () => {
    const dom = new JSDOM("<main></main>");
    const document = dom.window.document as Document;
    const previous = { document: (globalThis as any).document, window: (globalThis as any).window, HTMLElement: (globalThis as any).HTMLElement };
    Object.assign(globalThis, { document, window: dom.window, HTMLElement: dom.window.HTMLElement });
    const showModal = dom.window.HTMLDialogElement.prototype.showModal;
    const close = dom.window.HTMLDialogElement.prototype.close;
    dom.window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
    dom.window.HTMLDialogElement.prototype.close = function () { this.open = false; this.dispatchEvent(new dom.window.Event("close")); };
    try {
      let committed: any;
      editExperimentLimits(multichannelLimits(), next => { committed = next; });
      document.querySelector<HTMLInputElement>("[data-studio-limit='inputMinimum:P1.MV']")!.value = "";
      document.querySelector<HTMLInputElement>("[data-studio-limit='durationSeconds']")!.value = "";
      document.querySelector<HTMLButtonElement>("[data-studio-editor-done]")!.click();
      assert.ok(committed, "the editor refuses no blank entry itself");
      assert.strictEqual(committed.inputs.find((signal: { symbol: string }) => signal.symbol === "P1.MV").minimum, "", "the blank bound is committed as typed");
      assert.strictEqual(committed.inputs.find((signal: { symbol: string }) => signal.symbol === "P1.MV").maximum, 80, "a bound that reads stays a number");
      assert.strictEqual(committed.durationSeconds, "", "the blank duration is committed as typed");
      const request = acquisitionPreferences("automatic", committed);
      assert.deepStrictEqual(request.inputRanges!["P1.MV"], { minimum: "", maximum: 80 }, "and sent to the engine as typed");
      assert.strictEqual(request.durationSeconds, "");
    } finally {
      dom.window.HTMLDialogElement.prototype.showModal = showModal;
      dom.window.HTMLDialogElement.prototype.close = close;
      Object.assign(globalThis, previous);
      dom.window.close();
    }
  });

  test("opens with no sentence of its own for a missing duration, and shows the sentence a paused run asked with under the duration (B8)", () => {
    const dom = new JSDOM("<main></main>");
    const document = dom.window.document as Document;
    const previous = { document: (globalThis as any).document, window: (globalThis as any).window, HTMLElement: (globalThis as any).HTMLElement };
    Object.assign(globalThis, { document, window: dom.window, HTMLElement: dom.window.HTMLElement });
    const showModal = dom.window.HTMLDialogElement.prototype.showModal;
    const close = dom.window.HTMLDialogElement.prototype.close;
    dom.window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
    dom.window.HTMLDialogElement.prototype.close = function () { this.open = false; this.dispatchEvent(new dom.window.Event("close")); };
    try {
      const { durationSeconds: _duration, ...withoutDuration } = multichannelLimits();
      // The editor of a loop that declares no duration: nothing marks the duration until the engine's sentence arrives. The page wrote
      // "Enter the maximum experiment duration." itself when a run was waiting for the limits.
      editExperimentLimits(withoutDuration, () => undefined);
      const duration = () => document.querySelector<HTMLInputElement>("[data-studio-limit='durationSeconds']")!;
      assert.strictEqual(duration().getAttribute("aria-invalid"), null, "no sentence of the page's own marks the missing duration");
      document.querySelector<HTMLButtonElement>("[data-studio-editor-cancel]")!.click();
      // A run that waits for the limits asks in the engine's sentence and names the field (RunResult.needsMessage, needsField).
      const asked = "Set the maximum experiment duration in Limits before identification.";
      editExperimentLimits(withoutDuration, () => undefined, undefined, { field: "durationSeconds", message: asked });
      assert.strictEqual(duration().getAttribute("aria-invalid"), "true", "the field the engine names is marked");
      assert.strictEqual(document.getElementById(duration().getAttribute("aria-describedby") ?? "")?.textContent, asked, "under the engine's sentence");
    } finally {
      dom.window.HTMLDialogElement.prototype.showModal = showModal;
      dom.window.HTMLDialogElement.prototype.close = close;
      Object.assign(globalThis, previous);
      dom.window.close();
    }
  });

  test("posts an out-of-range limit, and shows the engine's refusal under the field it names", () => {
    const dom = new JSDOM("<main></main>");
    const document = dom.window.document as Document;
    const previousDocument = (globalThis as any).document;
    const previousWindow = (globalThis as any).window;
    const previousHTMLElement = (globalThis as any).HTMLElement;
    (globalThis as any).document = document;
    (globalThis as any).window = dom.window;
    (globalThis as any).HTMLElement = dom.window.HTMLElement;
    const showModal = dom.window.HTMLDialogElement.prototype.showModal;
    const close = dom.window.HTMLDialogElement.prototype.close;
    dom.window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
    dom.window.HTMLDialogElement.prototype.close = function () { this.open = false; this.dispatchEvent(new dom.window.Event("close")); };
    const values = multichannelLimits();
    try {
      let committed: any;
      editExperimentLimits(values, next => { committed = next; }, undefined, undefined);
      // The engine-served editor checks no range: the manipulated variable's maximum, declared 0 to 100 %, is committed as typed.
      const maximum = document.querySelector<HTMLInputElement>("[data-studio-limit='inputMaximum:P1.MV']")!;
      assert.strictEqual(maximum.hasAttribute("max"), false, "the field carries no range of its own");
      maximum.value = "150";
      document.querySelector<HTMLButtonElement>("[data-studio-editor-done]")!.click();
      assert.strictEqual(committed.inputs.find((signal: { symbol: string }) => signal.symbol === "P1.MV").maximum, 150, "the value beyond the declared range is committed");
      assert.strictEqual(acquisitionPreferences("automatic", committed).inputRanges!["P1.MV"].maximum, 150, "and posted to the engine");
      // The engine refuses that value under the field it names (B3.2); the dialog reopens with its sentence under that field.
      editExperimentLimits(committed, () => undefined, undefined,
        { field: "inputMaximum:P1.MV", message: "Experiment range for P1.MV must stay within its configured limits (0 to 100)." });
      const refused = document.querySelector<HTMLInputElement>("[data-studio-limit='inputMaximum:P1.MV']")!;
      assert.strictEqual(refused.value, "150", "the refused value stays in the field");
      assert.strictEqual(refused.getAttribute("aria-invalid"), "true", "the refused field is marked");
      assert.strictEqual(document.getElementById(refused.getAttribute("aria-describedby") ?? "")?.textContent,
        "Experiment range for P1.MV must stay within its configured limits (0 to 100).");
    } finally {
      dom.window.HTMLDialogElement.prototype.showModal = showModal;
      dom.window.HTMLDialogElement.prototype.close = close;
      (globalThis as any).document = previousDocument;
      (globalThis as any).window = previousWindow;
      (globalThis as any).HTMLElement = previousHTMLElement;
      dom.window.close();
    }
  });
});
