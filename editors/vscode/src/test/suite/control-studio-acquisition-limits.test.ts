import * as assert from "assert";
import { acquisitionSettings, configuredAcquisition } from "../../controlStudio/acquisitionSettings";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { editExperimentLimits } from "../../controlStudio/webview/threePageLimits";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

suite("Control Studio acquisition limits", () => {
  function loop(): ControlLoopDefinition {
    return {
      schemaVersion: 1, id: "limits", name: "Limits", revision: "1", projectId: "test", runtimeId: "test",
      controller: { instance: "P1.Controller", type: "Control.PID", revision: "1", action: "direct", taskPeriodMs: 100 },
      observedSampleTiming: { nominalPeriodMs: 100 },
      signals: {
        manipulatedVariable: { symbol: "P1.MV", address: "%QW0", iecType: "REAL", direction: "write", unit: "%", range: { minimum: 0, maximum: 100 } },
        processVariable: { symbol: "P1.PV", address: "%IW0", iecType: "REAL", direction: "read", unit: "degC", range: { minimum: 20, maximum: 80 } },
      },
      limits: { outputMinimum: 0, outputMaximum: 100 },
      experiment: { kind: "observation", execution: "control-studio", amplitude: 10, durationSeconds: 20, seed: 1 },
    };
  }
  const settings = (inputRange = { minimum: 10, maximum: 70 }, outputRange = { minimum: 25, maximum: 60 }) => ({
    method: "automatic", durationSeconds: 12, amplitude: 0, reuseRecordings: true, inputRange, outputRange,
  });
  test("constrains native experiment planning to the reviewed input and measured-output ranges", () => {
    const source = loop();
    const bounded = configuredAcquisition(source, acquisitionSettings(settings()));
    assert.deepStrictEqual(bounded.signals.manipulatedVariable.range, { minimum: 10, maximum: 70 });
    assert.deepStrictEqual(bounded.signals.processVariable.range, { minimum: 25, maximum: 60 });
    assert.deepStrictEqual(bounded.limits, { outputMinimum: 10, outputMaximum: 70 });
    assert.deepStrictEqual(source.signals.manipulatedVariable.range, { minimum: 0, maximum: 100 }, "A study limit must not change project limits");
  });
  test("rejects requested ranges outside the configured plant limits", () => {
    assert.throws(() => configuredAcquisition(loop(), acquisitionSettings(settings({ minimum: -1, maximum: 100 }))), /limit|range|bound/i);
    assert.throws(() => configuredAcquisition(loop(), acquisitionSettings(settings(undefined, { minimum: 20, maximum: 90 }))), /limit|range|bound/i);
  });
  test("rejects nonfinite or reversed range input before planning", () => {
    assert.strictEqual(acquisitionSettings(settings({ minimum: 50, maximum: 40 })), undefined);
    assert.strictEqual(acquisitionSettings(settings({ minimum: 0, maximum: Number.POSITIVE_INFINITY })), undefined);
  });
  test("requires a declared duration before planning an experiment", () => {
    const source = loop(); delete source.experiment;
    assert.throws(() => configuredAcquisition(source), /maximum.*duration|duration.*Limits/i);
  });

  function multichannelLoop(): ControlLoopDefinition {
    const source = loop();
    source.signals.additional = [
      { id: "manipulatedVariable2", channel: "input-2", role: "MV", symbol: "P1.MV2", address: "%QW1", iecType: "REAL", direction: "write", unit: "%", range: { minimum: 10, maximum: 90 } },
      { id: "processVariable2", channel: "output-2", role: "PV", symbol: "P1.PV2", address: "%IW1", iecType: "REAL", direction: "read", unit: "degC", range: { minimum: 25, maximum: 75 } },
    ];
    source.channels = {
      inputs: [
        { modelChannelId: "input-1", signalId: "manipulatedVariable", actuator: { kind: "continuous", minimum: 5, maximum: 95, rateUpPerS: 3, rateDownPerS: 2, minimumOnSeconds: 1, minimumOffSeconds: 2 } },
        { modelChannelId: "input-2", signalId: "manipulatedVariable2", actuator: { kind: "continuous", minimum: 15, maximum: 85, rateUpPerS: 4, rateDownPerS: 3, minimumOnSeconds: 1, minimumOffSeconds: 2 } },
      ],
      outputs: [{ modelChannelId: "output-1", signalId: "processVariable" }, { modelChannelId: "output-2", signalId: "processVariable2" }],
    };
    return source;
  }

  test("renders every selected channel with its signal and unit and commits ordered limits", () => {
    const source = multichannelLoop();
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
      const values = { durationSeconds: 20, inputMinimum: 10, inputMaximum: 80, outputMinimum: 30, outputMaximum: 70,
        inputRanges: { "P1.MV": { minimum: 10, maximum: 80 }, "P1.MV2": { minimum: 20, maximum: 70 } },
        outputRanges: { "P1.PV": { minimum: 30, maximum: 70 }, "P1.PV2": { minimum: 35, maximum: 65 } } } as any;
      let committed: any;
      editExperimentLimits(source, values, next => { committed = next; });
      const fields = [...(dom.window.document as Document).querySelectorAll<HTMLInputElement>("[data-studio-limit]")];
      assert.strictEqual(fields.filter(field => field.dataset.studioSignal === "P1.MV").length, 2);
      assert.strictEqual(fields.filter(field => field.dataset.studioSignal === "P1.MV2").length, 2);
      assert.strictEqual(fields.filter(field => field.dataset.studioSignal === "P1.PV").length, 2);
      assert.strictEqual(fields.filter(field => field.dataset.studioSignal === "P1.PV2").length, 2);
      assert.match(dom.window.document.body.textContent ?? "", /P1\.MV2.*%/s);
      assert.match(dom.window.document.body.textContent ?? "", /P1\.PV2.*°C/s);
      const done = (dom.window.document as Document).querySelector<HTMLButtonElement>("[data-studio-editor-done]")!; done.click();
      assert.deepStrictEqual(Object.keys(committed.inputRanges), ["P1.MV", "P1.MV2"]);
      assert.deepStrictEqual(Object.keys(committed.outputRanges), ["P1.PV", "P1.PV2"]);
    } finally {
      dom.window.HTMLDialogElement.prototype.showModal = showModal;
      dom.window.HTMLDialogElement.prototype.close = close;
      (globalThis as any).document = previousDocument;
      (globalThis as any).window = previousWindow;
      (globalThis as any).HTMLElement = previousHTMLElement;
      dom.window.close();
    }
  });

  test("narrows each configured channel without widening declared bounds or losing actuator constraints", () => {
    const source = multichannelLoop();
    const settings = settingsForChannels({ "P1.MV": { minimum: 20, maximum: 70 }, "P1.MV2": { minimum: 25, maximum: 65 } },
      { "P1.PV": { minimum: 35, maximum: 65 }, "P1.PV2": { minimum: 40, maximum: 60 } });
    const bounded = configuredAcquisition(source, acquisitionSettings(settings) as any);
    assert.deepStrictEqual(bounded.channels!.inputs.map(channel => [channel.actuator.minimum, channel.actuator.maximum]), [[20, 70], [25, 65]]);
    assert.strictEqual(bounded.channels!.inputs[0].actuator.rateUpPerS, 3);
    assert.strictEqual(bounded.channels!.inputs[0].actuator.minimumOffSeconds, 2);
    assert.deepStrictEqual(bounded.signals.additional?.filter(signal => signal.role === "PV").map(signal => signal.range), [{ minimum: 40, maximum: 60 }]);
    assert.throws(() => configuredAcquisition(source, acquisitionSettings(settingsForChannels({ "P1.MV": { minimum: 0, maximum: 70 }, "P1.MV2": { minimum: 25, maximum: 65 } }, {})) as any), /limit|range|bound/i);
  });

  function settingsForChannels(inputRanges: Record<string, { minimum: number; maximum: number }>, outputRanges: Record<string, { minimum: number; maximum: number }>) {
    return { ...settings(inputRanges["P1.MV"] ?? { minimum: 10, maximum: 70 }, outputRanges["P1.PV"] ?? { minimum: 25, maximum: 60 }), inputRanges, outputRanges } as any;
  }
  test("narrows the channel actuator together with the primary signal and preserves its rate constraints", () => {
    const source = loop();
    source.channels = { inputs: [{ modelChannelId: "power", signalId: "manipulatedVariable", actuator: { kind: "continuous", minimum: 15, maximum: 80, rateUpPerS: 3 } }],
      outputs: [{ modelChannelId: "temperature", signalId: "processVariable" }] };
    const bounded = configuredAcquisition(source, acquisitionSettings(settings({ minimum: 20, maximum: 70 })));
    assert.deepStrictEqual(bounded.channels!.inputs[0].actuator, { kind: "continuous", minimum: 20, maximum: 70, rateUpPerS: 3 });
    assert.deepStrictEqual(source.channels.inputs[0].actuator, { kind: "continuous", minimum: 15, maximum: 80, rateUpPerS: 3 });
    assert.throws(() => configuredAcquisition(source, acquisitionSettings(settings({ minimum: 10, maximum: 70 }))), /limit|range|bound/i);
  });
});
