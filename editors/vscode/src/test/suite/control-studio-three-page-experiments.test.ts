import * as assert from "assert";
import type { PlotSignal } from "../../controlStudio/generated/OpenResult";
import { setOpenedOffers } from "../../controlStudio/webview/openedOffers";
import { ThreePageLiveSignals } from "../../controlStudio/webview/threePageLiveSignals";
import { recordedSignalResponses } from "../../controlStudio/webview/threePageRecordedSignals";

import { fillProjectTest, renderThreePageAnalysisParameterTemplate, renderThreePageExperimentTemplate } from "../../controlStudio/threePageExperimentTemplate";
import { ThreePageExperiments } from "../../controlStudio/webview/threePageExperiments";

suite("Control Studio three-page project experiments", () => {
  test("publishes the kept recording's samples to the chart without changing the state of the test", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${projectTestMarkup(projectLoop())}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      let recordings = 0;
      dom.window.addEventListener("trust-control-studio-experiment-recording", () => recordings++);
      const root = dom.window.document.querySelector("main");
      const experiments = new ThreePageExperiments(root, answeredForm(projectLoop()), () => assert.fail("A recording cannot write"));
      // The engine keeps the samples of a finished test (`control-studio.recording.evidence`); the chart draws the signals it selects.
      experiments.receive("recording", { recording: { rawSamples: [] } });
      assert.strictEqual(recordings, 1, "the kept recording reaches the chart");
      assert.strictEqual(experiments.busy, false, "a recording starts no test");
      experiments.receive("running", { experiment_id: "new-run" });
      assert.strictEqual(experiments.busy, true);
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, "Test running…");
    } finally { Object.assign(globalThis, original); dom.window.close(); }
  });

  test("starts a fresh live chart once per runtime experiment, without replaying status polls", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${projectTestMarkup(projectLoop())}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const starts: string[] = [];
      dom.window.addEventListener("trust-control-studio-experiment-started", (event: any) => starts.push(event.detail.experimentId));
      const experiments = new ThreePageExperiments(dom.window.document.querySelector("main"), answeredForm(projectLoop()), () => assert.fail("Observing runtime status cannot write"));
      experiments.receive("running", { experiment_id: "first" });
      experiments.receive("running", { experiment_id: "first" });
      assert.deepStrictEqual(starts, ["first"], "Only a new acquisition should replace the retained chart");
      experiments.receive("engine-ended", { status: "finished", restoration: "restored" });
      experiments.receive("running", { experiment_id: "second" });
      assert.deepStrictEqual(starts, ["first", "second"]);
    } finally { Object.assign(globalThis, original); dom.window.close(); }
  });

  /**
   * The signals the engine lists for the project loop's plots (`liveSides.plotSignals`, crates/trust-control/src/messages/plot_signals.rs):
   * the identifier of the Signals selector, the variable, the unit and the width of the declared range.
   */
  const PLOT_SIGNALS: PlotSignal[] = [
    { id: "sp", kind: "setpoint", symbol: "P1.Setpoint", unit: "degC", span: 100 },
    { id: "pv", kind: "measurement", symbol: "P1.ProcessValue", unit: "degC", span: 100 },
    { id: "mv", kind: "appliedOutput", symbol: "P1.AppliedHeaterPower", unit: "%", span: 50 },
    { id: "request", kind: "additional", symbol: "P1.RequestedHeaterPower", unit: "%", span: 500 },
  ];

  /** One available live sample of the project loop, as the engine marks it: each value says whether the plot draws it. */
  const liveSample = (loop: any, scan: number, requestedDrawn: boolean): any => ({
    availability: "available", source: "Simulation", loopId: loop.id, loopRevision: loop.revision, scan, observedAtMs: scan * 100,
    runtimeSessionId: "runtime-1", controller: { instance: "P1.Controller", revision: "r1", mode: "Auto" },
    setpoint: { symbol: "P1.Setpoint", unit: "degC", value: 35, quality: "good", plotted: true },
    processVariable: { symbol: "P1.ProcessValue", unit: "degC", value: 34 + scan / 100, quality: "good", plotted: true },
    manipulatedVariable: { symbol: "P1.AppliedHeaterPower", unit: "%", value: 75, quality: "good", plotted: true },
    additional: [{ id: "request", channel: "request", role: "MV", displayName: "Requested power", symbol: "P1.RequestedHeaterPower", unit: "%",
      value: 110, quality: requestedDrawn ? "good" : "forced", plotted: requestedDrawn }],
    outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, saturation: "none", recording: false,
  });

  test("projects the selected setpoint and diagnostics from actual live readings", () => {
    const loop = projectLoop();
    setOpenedOffers({ liveSides: { inputs: [], outputs: [], plotSignals: PLOT_SIGNALS } });
    try {
      const live = new ThreePageLiveSignals();
      // The second sample's requested power is a forced value: the engine marks it as no plot point, and the page draws what it marks.
      for (const scan of [10, 11]) live.receive(liveSample(loop, scan, scan === 10));
      const responses = Reflect.apply(live.responses, live,
        [[loop.signals.manipulatedVariable.symbol], [loop.signals.processVariable.symbol], ["sp", "pv", "request"]]);
      const series = responses.flatMap((group: any) => group.series);
      assert.deepStrictEqual(series.map((item: any) => item.name), ["P1.Setpoint", "P1.ProcessValue", "P1.RequestedHeaterPower"],
        "The experiment selection must determine the live chart, including SP and diagnostics");
      assert.deepStrictEqual(series.map((item: any) => item.signalId), ["sp", "pv", "request"], "each trace carries the engine's identifier");
      assert.deepStrictEqual(series[0].points, [[0, 35], [0.1, 35]]);
      assert.deepStrictEqual(series[2].points, [[0, 110], [0.1, null]], "A value the engine does not mark as drawn must remain a gap");
      assert.strictEqual(series[0].dashed, true, "Setpoint must stay distinct from coincident process value");
      // The engine names each recorded value by the same identifiers, and marks a gap with null (RecordedChartSample.plotted).
      const recorded = recordedSignalResponses(PLOT_SIGNALS, [{ elapsedSeconds: 0, plotted: { sp: 35, pv: 34.1, mv: 75, request: 110 } }],
        ["sp", "pv", "request"]).flatMap(group => group.series);
      assert.deepStrictEqual(recorded.map(item => [item.name, item.points[0]]), series.map((item: any) => [item.name, item.points[0]]),
        "Live and recorded views must retain the same signal identity and value");
    } finally { setOpenedOffers({}); }
  });

  test("draws a recorded sample's gaps as null and the engine's values as given, in the engine's signal order", () => {
    // `plotted` is the engine's: null marks a missing, bad or forced value (B9.6), and the page applies no rule of its own to the rest.
    const samples = [
      { elapsedSeconds: 0, plotted: { sp: 35, pv: 34, mv: 75, request: 110 } },
      { elapsedSeconds: 0.1, plotted: { sp: 35, pv: null, mv: 75, request: 0 } },
    ];
    const groups = recordedSignalResponses(PLOT_SIGNALS, samples, ["request", "pv", "mv"]);
    assert.deepStrictEqual(groups.map(group => [group.unit, group.name, group.series.map(item => item.signalId)]),
      [["degC", "P1.ProcessValue", ["pv"]], ["%", "Response", ["mv", "request"]]],
      "the signals are drawn in the engine's order, one group for each unit, and a group of several signals is a Response");
    assert.deepStrictEqual(groups[0].series[0].points, [[0, 34], [0.1, null]], "a null value is a gap, never zero");
    assert.deepStrictEqual(groups[1].series[1].points, [[0, 110], [0.1, 0]], "a zero the engine gives is drawn as zero");
  });

  test("draws the sides the engine names, framed by the range width it lists", () => {
    const loop = projectLoop();
    setOpenedOffers({ liveSides: {
      inputs: [{ signalId: "manipulatedVariable", symbol: "P1.AppliedHeaterPower", unit: "%" }],
      outputs: [{ signalId: "processVariable", symbol: "P1.ProcessValue", unit: "degC" }],
      plotSignals: PLOT_SIGNALS,
    } });
    try {
      const live = new ThreePageLiveSignals();
      live.receive(liveSample(loop, 10, true));
      const groups = live.responses(["P1.AppliedHeaterPower"], ["P1.ProcessValue"]);
      // The measured outputs come first, then the plant inputs; each group is framed by one hundredth of its signal's range width.
      assert.deepStrictEqual(groups.map(group => [group.unit, group.zeroSpan, group.series.map(item => item.signalId)]),
        [["degC", 1, ["pv"]], ["%", 0.5, ["mv"]]]);
      assert.deepStrictEqual(groups.flatMap(group => group.series.map(item => item.label)), ["Measurement: 34.1 °C", "Output: 75 %"]);
      assert.deepStrictEqual(live.responses([], ["P1.ProcessValue"]).map(group => group.series.length), [1],
        "only the sides the selectors name are drawn");
      setOpenedOffers({});
      assert.deepStrictEqual(live.responses(["P1.AppliedHeaterPower"], ["P1.ProcessValue"]), [], "without the engine's open answer nothing is drawn");
    } finally { setOpenedOffers({}); }
  });

  test("keeps project instructions compact and hides the reviewed plan while running", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${projectTestMarkup(projectLoop())}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const root = dom.window.document.querySelector("main");
      const experiments = new ThreePageExperiments(root, answeredForm(projectLoop()), () => undefined);
      experiments.setSelectedMethod(true);
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").hidden, true,
        "The review button already explains the first action");
      experiments.receive("review", engineReview());
      assert.strictEqual(root.querySelector("[data-three-page-experiment-plan]").hidden, false);
      assert.match(root.querySelector("[data-three-page-experiment-restoration]").textContent, /P1.ExperimentEnabled = FALSE/,
        "Show the actual restored signal value rather than ambiguous Restoration Disabled");
      experiments.receive("running", { experiment_id: "run-1" });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-plan]").hidden, true);
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, "Test running…");
      assert.strictEqual(experiments.primaryActionLabel, "Abort test");
      // The engine ends a campaign whose restoration it could not confirm as a failure, and the page shows the engine's words.
      experiments.receive("engine-ended", { status: "failed", reason: "The activation was not confirmed restored." });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, "The activation was not confirmed restored.",
        "Unresolved runtime restoration must remain visible");
      experiments.receive("running", { experiment_id: "run-2" });
      experiments.receive("engine-ended", { status: "finished", reason: "Project test completed. Outputs restored.", restoration: "restored" });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, "Project test completed. Outputs restored.");
    } finally { Object.assign(globalThis, original); dom.window.close(); }
  });

  test("renders a contextual project experiment workspace with declared signals", () => {
    const loop = projectLoop();
    const html = projectTestMarkup(loop);
    assert.match(html, /data-three-page-experiment/);
    assert.match(html, /data-three-page-experiment-signals/);
    assert.match(html, /P1\.ExperimentEnabled/);
    assert.match(html, /P1\.RequestedHeaterPower/);
  });

  test("renders the declared signals' native list hidden and out of the tab order before the page gives it its dropdown", () => {
    // Identify page specification, Signals: multiple selections use a checkbox dropdown, so no native multi-select is shown or reached with
    // Tab, also before the engine fills the project test (the page makes the dropdown then). Catches a native list left visible and focusable.
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageExperimentTemplate()}</main>`);
    try {
      const select = dom.window.document.querySelector("[data-three-page-experiment-signals]") as HTMLSelectElement;
      assert.strictEqual(select.multiple, true);
      assert.strictEqual(select.hidden, true, "the native list is hidden");
      assert.strictEqual(select.tabIndex, -1, "the native list is out of the tab order");
    } finally { dom.window.close(); }
  });

  test("provides the shared parameter field host only for the tool the engine's form names", () => {
    const html = projectTestMarkup(analysisParameterLoop(), "analysis");
    assert.match(html, /data-three-page-experiment-parameters/);
    assert.match(html, /data-three-page-experiment-parameter-fields/);
    assert.match(html, /data-three-page-experiment-parameter-action/, "the tool carries its own step");
    assert.doesNotMatch(projectTestMarkup(projectLoop()), /data-three-page-experiment-parameters/, "a form that names no tool fills none");
  });

  test("reviews and starts the declared project sequence, then aborts the same runtime experiment", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main data-studio-three-page>${projectTestMarkup(projectLoop())}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const messages: Array<{ kind: string; payload: Record<string, unknown> }> = [];
      const experiments = new ThreePageExperiments(dom.window.document.querySelector("main") as HTMLElement, answeredForm(projectLoop()),
        (kind, payload) => messages.push({ kind, payload: payload ?? {} }));
      experiments.setSelectedMethod(true);
      assert.strictEqual(experiments.primaryAction(), true);
      assert.deepStrictEqual(messages[0], {
        kind: "control-studio.experiment.review",
        payload: {},
      }, "the review names nothing of its own: the engine reviews the declared test");
      experiments.receive("review", engineReview());
      assert.strictEqual(experiments.primaryAction(), true);
      assert.deepStrictEqual(messages[1], {
        kind: "control-studio.experiment.approve",
        payload: { planId: "test-plan", physicalApproved: false },
      });
      experiments.receive("running", { experiment_id: "test-run" });
      assert.strictEqual(experiments.primaryAction(), true);
      assert.deepStrictEqual(messages[2], {
        kind: "control-studio.experiment.abort",
        payload: { experimentId: "test-run" },
      });
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });

  test("publishes only declared signal identities when the plot selection changes", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main data-studio-three-page>${projectTestMarkup(projectLoop())}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const events: string[][] = [];
      dom.window.addEventListener("trust-control-studio-experiment-signals", (event: Event) => {
        events.push((event as CustomEvent<{ signalIds: string[] }>).detail.signalIds);
      });
      const experiments = new ThreePageExperiments(dom.window.document.querySelector("main") as HTMLElement, answeredForm(projectLoop()), () => undefined);
      experiments.setSelectedMethod(true);
      const select = dom.window.document.querySelector("[data-three-page-experiment-signals]") as HTMLSelectElement;
      for (const option of Array.from(select.options)) option.selected = option.value === "pv" || option.value === "request";
      select.dispatchEvent(new dom.window.Event("change", { bubbles: true }));
      assert.deepStrictEqual(events, [["pv", "request"]]);
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });

  test("steps the analysis parameters on the Tune page with the tool's own button as the engine offers each step, apart from the project test", () => {
    withAnalysisTool((page) => {
      const { root, experiments, step, field, messages, loop } = page;
      const status = () => root.querySelector("[data-three-page-experiment-parameter-status]")!.textContent;
      assert.strictEqual(step.textContent, "Inspect experiment parameters", "before the engine answers, the tool's first step reads the record");
      assert.strictEqual(experiments.primaryActionLabel, "Review project test", "the Identify button is the project test's, whatever the tool holds");
      step.click();
      assert.deepStrictEqual(messages[0], { kind: "control-studio.parameters.inspect", payload: {} });
      experiments.receive("parameters", { status: "inspected", values: { "P1.FlowDeviation": 0.2 }, binding: loop.parameterBinding, reason: INSPECTED, offer: FREE });
      const shown = field();
      assert.strictEqual(shown.value, "0.2", "The inspected value reaches the existing shared numeric field");
      assert.match(root.ownerDocument.querySelector(`label[for="${shown.id}"]`)!.textContent!, /Flow deviation/i);
      assert.match(root.ownerDocument.getElementById(shown.getAttribute("aria-describedby")!)!.textContent!, /L\/s/);
      assert.strictEqual(status(), INSPECTED, "the sentence is the engine's");
      assert.strictEqual(step.textContent, "Review parameter values", "the engine offers the review next");
      step.click();
      assert.deepStrictEqual(messages[1], { kind: "control-studio.parameters.review", payload: { json: JSON.stringify({ "P1.FlowDeviation": 0.2 }, null, 2) } });
      experiments.receive("parameters", { status: "reviewed", reviewId: "review-1", values: { "P1.FlowDeviation": 0.2 }, offer: REVIEWED });
      assert.strictEqual(step.textContent, "Apply reviewed values");
      step.click();
      assert.deepStrictEqual(messages[2], { kind: "control-studio.parameters.apply", payload: { reviewId: "review-1", physicalApproved: false } });
      experiments.receive("parameters", { status: "applied", values: { "P1.FlowDeviation": 0.2 }, reason: APPLIED, offer: FREE });
      assert.strictEqual(status(), APPLIED, "the write's outcome is the engine's sentence");
      assert.strictEqual(step.textContent, "Review parameter values", "after the write the engine offers a review again");
      assert.strictEqual(field().disabled, false);
      // The project test is the Identify button's: review, start and Abort. A review holds nothing in the page, and the page's own running
      // test holds the tool's controls.
      assert.strictEqual(experiments.primaryAction(), true);
      assert.strictEqual(messages[3].kind, "control-studio.experiment.review");
      assert.deepStrictEqual(messages[3].payload, {});
      experiments.receive("review", engineReview());
      assert.strictEqual(field().disabled, false, "the engine binds the settings to the test's review (C12), so the page locks nothing");
      experiments.primaryAction();
      experiments.receive("running", { experiment_id: "test-run" });
      assert.strictEqual(step.disabled, true, "the tool waits while the page's own test runs");
      assert.strictEqual(field().disabled, true);
      experiments.primaryAction();
      assert.strictEqual(messages[messages.length - 1].kind, "control-studio.experiment.abort");
      experiments.receive("engine-ended", { status: "cancelled", reason: "Identification cancelled." });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]")!.textContent, "Identification cancelled.");
      assert.strictEqual(step.disabled, false, "the test has ended");
      assert.strictEqual(step.textContent, "Review parameter values", "the tool keeps the engine's last offer");
      assert.strictEqual(status(), APPLIED, "an end without the engine's release leaves the tool's sentence as it was");
    });
  });

  test("a test's end shows the engine's sentence that the settings are released, and the tool keeps the engine's offer", () => {
    withAnalysisTool((page) => {
      const { root, experiments, step, field } = page;
      applyValues(page);
      experiments.primaryAction();
      experiments.receive("review", engineReview());
      experiments.primaryAction();
      experiments.receive("running", { experiment_id: "test-run" });
      assert.strictEqual(step.disabled, true, "the tool waits while the test runs");
      experiments.receive("engine-ended", { status: "finished", reason: "Project test completed. Outputs restored.", restoration: "restored", settingsReleased: SETTINGS_RELEASED });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-parameter-status]")!.textContent, SETTINGS_RELEASED, "the sentence is the engine's");
      assert.strictEqual(step.disabled, false, "the tool's step is offered again");
      assert.strictEqual(step.textContent, "Review parameter values", "the step is the engine's last offer");
      assert.strictEqual(field().disabled, false, "the values can be edited again");
    });
  });

  test("an end without the engine's release leaves the tool as it was, and the page decides no release of its own", () => {
    withAnalysisTool((page) => {
      const { root, experiments, step } = page;
      applyValues(page);
      experiments.primaryAction();
      experiments.receive("review", engineReview());
      experiments.primaryAction();
      experiments.receive("running", { experiment_id: "test-run" });
      experiments.receive("engine-ended", { status: "failed", reason: "The activation was not confirmed restored." });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-parameter-status]")!.textContent, APPLIED, "the tool's sentence is unchanged");
      assert.strictEqual(step.textContent, "Review parameter values", "and so is the engine's offer");
    });
    withAnalysisTool((page) => {
      const { root, experiments, step } = page;
      experiments.primaryAction();
      experiments.receive("review", engineReview());
      experiments.primaryAction();
      experiments.receive("running", { experiment_id: "test-run" });
      experiments.receive("engine-ended", { status: "finished", reason: "Project test completed. Outputs restored.", restoration: "restored", settingsReleased: SETTINGS_RELEASED });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-parameter-status]")!.textContent, SETTINGS_RELEASED,
        "the engine decides when the release applies; the page shows its sentence");
      assert.strictEqual(step.textContent, "Inspect experiment parameters", "the tool has no answer yet, so its step is still the first");
    });
  });

  test("a test's end takes the tool's offer that the engine answers with it, and a write's progress shows the engine's words while it runs (C12)", () => {
    withAnalysisTool((page) => {
      const { root, experiments, step } = page;
      const status = () => root.querySelector("[data-three-page-experiment-parameter-status]")!.textContent;
      applyValues(page);
      experiments.primaryAction();
      experiments.receive("review", engineReview());
      experiments.primaryAction();
      experiments.receive("running", { experiment_id: "test-run" });
      experiments.receive("engine-ended", { status: "cancelled", reason: "Identification cancelled.", offer: FIRST });
      assert.strictEqual(step.textContent, "Inspect experiment parameters", "the tool takes the offer the engine answers after the test");
      step.click();
      experiments.receive("parameters", { status: "inspected", values: { "P1.FlowDeviation": 0.2 }, offer: FREE });
      step.click();
      experiments.receive("parameters", { status: "reviewed", reviewId: "review-2", values: { "P1.FlowDeviation": 0.2 }, offer: REVIEWED });
      step.click();
      assert.strictEqual(status(), "", "the page writes no progress sentence of its own");
      assert.strictEqual(step.textContent, "Applying values…", "the button names the request in flight");
      experiments.receive("parameters", { status: "running", reason: "Waiting for the owning task and observed values…" });
      assert.strictEqual(status(), "Waiting for the owning task and observed values…", "the write's progress is the engine's");
      assert.strictEqual(step.textContent, "Applying values…", "the write is still in flight");
      experiments.receive("parameters", { status: "applied", values: { "P1.FlowDeviation": 0.2 }, reason: APPLIED, offer: FREE });
      assert.strictEqual(status(), APPLIED);
      assert.strictEqual(step.textContent, "Review parameter values");
    });
  });

  test("provides the experiment settings' tool inside the project test when the engine's form names it, and leaves the Tune page's slot empty", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${projectTestMarkup(experimentSettingsLoop(), "experiment")}</main>`);
    try {
      const root = dom.window.document.querySelector("main") as HTMLElement;
      const tool = root.querySelector("[data-three-page-experiment] [data-three-page-experiment-parameters]");
      assert.ok(tool, "the settings sit inside the project test, on the Identify page");
      assert.match(tool!.querySelector("summary")!.textContent!, /Declared experiment settings/);
      assert.ok(tool!.querySelector("[data-three-page-experiment-parameter-fields]"), "the shared field host");
      assert.ok(tool!.querySelector("[data-three-page-experiment-parameter-action]"), "the tool carries its own step");
      assert.ok(tool!.querySelector("[data-three-page-experiment-restore]"), "and its Restore review");
      assert.strictEqual(root.querySelector("[data-three-page-analysis-parameter-slot]")!.children.length, 0, "an analysis's slot on the Tune page stays empty");
      assert.doesNotMatch(projectTestMarkup(experimentSettingsLoop()), /data-three-page-experiment-parameters/, "a form that names no tool fills none");
    } finally { dom.window.close(); }
  });

  test("shows the experiment settings from the form with their own bounds and steps them as the engine offers, apart from the project test the Identify button reviews", () => {
    withExperimentTool((page) => {
      const { root, experiments, step, field, messages, window } = page;
      const fields = () => Array.from(root.querySelectorAll<HTMLInputElement>("[data-parameter-symbol]")).map(input => [input.dataset.parameterSymbol, input.min, input.max]);
      assert.deepStrictEqual(fields(), [["P1.SelectedStepPoints", "5", "15"], ["P1.HoldSeconds", "20", "40"]],
        "the form lists the loop's whole binding, so the fields stand before any answer");
      assert.strictEqual(root.querySelector("[data-three-page-experiment-parameter-mode]")!.textContent, "Activation: P1.ExperimentEnabled = false");
      assert.strictEqual(step.textContent, "Inspect experiment parameters");
      assert.strictEqual(experiments.primaryActionLabel, "Review project test", "the Identify button is the project test's, whatever the tool holds");
      step.click();
      assert.deepStrictEqual(messages[0], { kind: "control-studio.parameters.inspect", payload: {} });
      experiments.receive("parameters", { status: "inspected", values: { "P1.SelectedStepPoints": 10, "P1.HoldSeconds": 30 }, reason: INSPECTED, offer: FREE });
      assert.strictEqual(field("P1.HoldSeconds").value, "30");
      assert.strictEqual(step.textContent, "Review parameter values");
      // An edit spends any review: the page asks the engine, which answers the sentence the page shows and the tool's offer.
      field("P1.HoldSeconds").value = "35";
      field("P1.HoldSeconds").dispatchEvent(new window.Event("input", { bubbles: true }));
      assert.deepStrictEqual(messages[1], { kind: "control-studio.parameters.invalidate", payload: {} });
      experiments.receive("parameters", { status: "edited", reason: "Values changed. Review again before applying.", offer: FREE });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-parameter-status]")!.textContent, "Values changed. Review again before applying.");
      step.click();
      assert.deepStrictEqual(messages[2], { kind: "control-studio.parameters.review", payload: { json: JSON.stringify(EXPERIMENT_VALUES, null, 2) } },
        "the text goes as the fields wrote it; the engine reads it");
      experiments.receive("parameters", { status: "reviewed", reviewId: "review-1", values: EXPERIMENT_VALUES, offer: REVIEWED });
      assert.strictEqual(step.textContent, "Apply reviewed values");
      step.click();
      assert.deepStrictEqual(messages[3], { kind: "control-studio.parameters.apply", payload: { reviewId: "review-1", physicalApproved: false } });
      experiments.receive("parameters", { status: "applied", values: EXPERIMENT_VALUES, reason: APPLIED, offer: FREE });
      assert.strictEqual(step.textContent, "Review parameter values", "the engine offers a review again after the write");
      assert.strictEqual(field("P1.HoldSeconds").disabled, false);
    });
  });

  test("the experiment settings wait only while the page's own test runs: a test's review holds nothing in the page (C12)", () => {
    withExperimentTool((page) => {
      const { root, experiments, step, field, messages } = page;
      const restore = root.querySelector("[data-three-page-experiment-restore]") as HTMLButtonElement;
      step.click();
      experiments.receive("parameters", { status: "inspected", values: { "P1.SelectedStepPoints": 10, "P1.HoldSeconds": 30 }, offer: FREE });
      assert.strictEqual(field("P1.HoldSeconds").disabled, false, "inspected settings can be edited");
      assert.strictEqual(restore.disabled, false, "and restored, as the engine offers");
      experiments.primaryAction();
      assert.strictEqual(messages[messages.length - 1].kind, "control-studio.experiment.review");
      experiments.receive("review", engineReview());
      assert.strictEqual(field("P1.HoldSeconds").disabled, false, "the engine binds the settings to the review, so the page locks nothing");
      assert.strictEqual(step.disabled, false);
      experiments.primaryAction();
      experiments.receive("running", { experiment_id: "test-run" });
      assert.strictEqual(field("P1.HoldSeconds").disabled, true, "the page's own test runs");
      assert.strictEqual(restore.disabled, true);
      assert.strictEqual(step.disabled, true);
      experiments.receive("engine-ended", { status: "finished", reason: "Project test completed. Outputs restored.", restoration: "restored" });
      assert.strictEqual(field("P1.HoldSeconds").disabled, false, "the end frees the controls");
      assert.strictEqual(restore.disabled, false);
    });
  });

  test("an aborted or a failed test shows the engine's release of the settings when it gives one, and the tool keeps the engine's offer", () => {
    for (const [status, released] of [
      ["cancelled", SETTINGS_RELEASED],
      ["failed", SETTINGS_RELEASED],
      ["cancelled", undefined],
      ["failed", undefined],
    ] as const) {
      withExperimentTool((page) => {
        const { root, experiments, step, field } = page;
        applyExperimentSettings(page);
        experiments.primaryAction();
        experiments.receive("review", engineReview());
        experiments.primaryAction();
        experiments.receive("running", { experiment_id: "test-run" });
        assert.strictEqual(step.disabled, true, "the tool waits while the test runs");
        experiments.receive("engine-ended", { status, reason: status === "failed" ? "The test stopped early." : "Identification cancelled.", ...(released ? { settingsReleased: released } : {}) });
        const label = `${status} ${released ? "with" : "without"} a release`;
        assert.strictEqual(step.disabled, false, label);
        assert.strictEqual(field("P1.HoldSeconds").disabled, false, label);
        assert.strictEqual(step.textContent, "Review parameter values", `${label}: the engine's last offer`);
        assert.strictEqual(root.querySelector("[data-three-page-experiment-parameter-status]")!.textContent, released ?? APPLIED, `${label}: the sentence is the engine's`);
      });
    }
  });
});

/** The sentence the engine words for a released tool (`SETTINGS_RELEASED` in crates/trust-control/src/identification/project_test.rs). */
const SETTINGS_RELEASED = "The previous project test restored its declared values. Edit and review them before the next trial.";
/** The engine's offers of the parameter tool (C12, `parameter_tool_offer` in crates/trust-control/src/messages/parameter_tool.rs). */
const FREE = { nextStep: "review", available: ["inspect", "review", "restore"] };
const REVIEWED = { nextStep: "apply", available: ["inspect", "review", "apply", "restore"] };
/** The tool's first offer, which the open answer's form names (`first_tool_offer`, C12), and its offer while a test holds the settings. */
const FIRST = { nextStep: "inspect", available: ["inspect"] };
/** The engine's sentences of an inspection and of a confirmed write (C12, analysis_parameters_action.rs and parameter_application.rs). */
const INSPECTED = "Current values loaded. Edit them, then review the exact change.";
const APPLIED = "Reviewed values observed after ST execution.";

interface AnalysisToolPage {
  root: HTMLElement;
  experiments: ThreePageExperiments;
  /** The tool's own step button on the Tune page. */
  step: HTMLButtonElement;
  /** The field of the loop's one parameter. */
  field: () => HTMLInputElement;
  messages: Array<{ kind: string; payload: Record<string, unknown> }>;
  loop: any;
}

/** The analysis parameter tool (B6.1, C12) on a DOM with the project test's section, for a form that names the tool. */
function withAnalysisTool(exercise: (page: AnalysisToolPage) => void): void {
  const { JSDOM } = require("jsdom");
  const loop = analysisParameterLoop();
  const dom = new JSDOM(`<main data-studio-three-page>${projectTestMarkup(loop, "analysis")}</main>`);
  const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
  Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
  try {
    const root = dom.window.document.querySelector("main") as HTMLElement;
    const messages: AnalysisToolPage["messages"] = [];
    const experiments = new ThreePageExperiments(root, answeredForm(loop, "analysis"), (kind, payload) => messages.push({ kind, payload: payload ?? {} }));
    experiments.setSelectedMethod(true);
    const step = root.querySelector("[data-three-page-experiment-parameter-action]") as HTMLButtonElement;
    const field = () => root.querySelector('[data-parameter-symbol="P1.FlowDeviation"]') as HTMLInputElement;
    exercise({ root, experiments, step, field, messages, loop });
  } finally {
    Object.assign(globalThis, original);
    dom.window.close();
  }
}

/** Takes the tool through inspect, review and apply, as the engine answers each step. */
function applyValues(page: AnalysisToolPage): void {
  const { experiments, step, loop } = page;
  step.click();
  experiments.receive("parameters", { status: "inspected", values: { "P1.FlowDeviation": 0.2 }, binding: loop.parameterBinding, offer: FREE });
  step.click();
  experiments.receive("parameters", { status: "reviewed", reviewId: "review-1", values: { "P1.FlowDeviation": 0.2 }, offer: REVIEWED });
  step.click();
  experiments.receive("parameters", { status: "applied", values: { "P1.FlowDeviation": 0.2 }, reason: APPLIED, offer: FREE });
}

/**
 * The project test's form the engine answers for a fixture loop (`projectTestForm`, B6.1, C12): the signals it offers with the labels the
 * engine gives them, and the duration the loop declares. The fixture stands for the engine's answer; the Rust form has its own tests.
 */
function answeredForm(loop: any, parameterTool?: "analysis" | "experiment"): any {
  const signals = loop.signals;
  const applied = signals.actuator ?? signals.manipulatedVariable;
  return {
    signals: [
      ...(signals.setpoint ? [{ id: "sp", label: `${signals.setpoint.symbol} · Setpoint`, unit: signals.setpoint.unit, selected: true }] : []),
      { id: "pv", label: `${signals.processVariable.symbol} · Process value`, unit: signals.processVariable.unit, selected: true },
      { id: "mv", label: `${applied.symbol} · Applied output`, unit: signals.manipulatedVariable.unit, selected: true },
      ...(signals.actuator ? [{ id: "requestedMv", label: `${signals.manipulatedVariable.symbol} · Requested output`, unit: signals.manipulatedVariable.unit, selected: false }] : []),
      ...(signals.additional ?? []).map((signal: any) => ({ id: signal.id, label: `${signal.symbol} · ${signal.role}`, unit: signal.unit || undefined,
        selected: signal.role === "SP" && !signals.setpoint })),
    ],
    durationSeconds: loop.experiment?.durationSeconds ?? 30,
    parameterTool,
    // The tool of a declared analysis starts with the binding that lists no parameter: the engine's answers name the ones it may write.
    // The experiment's settings are the loop's whole binding (`tool_binding` in crates/trust-control/src/identification/project_test.rs).
    ...(parameterTool === "analysis" ? { parameterBinding: { ...loop.parameterBinding, parameters: [] } } : {}),
    ...(parameterTool === "experiment" ? { parameterBinding: loop.parameterBinding } : {}),
    // The tool's first offer, which the engine's form names (`first_tool_offer`, C12): Inspect, which reads only.
    ...(parameterTool ? { parameterOffer: FIRST } : {}),
  };
}

/** The project test's markup as the page leaves it once the engine's form is filled in (B6.1, C12). */
function projectTestMarkup(loop: any, parameterTool?: "analysis" | "experiment"): string {
  const { JSDOM } = require("jsdom");
  const dom = new JSDOM(`<main>${renderThreePageExperimentTemplate()}${renderThreePageAnalysisParameterTemplate()}</main>`);
  const root = dom.window.document.querySelector("main")!;
  fillProjectTest(root, answeredForm(loop, parameterTool));
  return root.innerHTML;
}

/**
 * The engine's review of the project test as the route posts it (`controlStudio/identify` with `review`, C12): the identity the start
 * confirms and what the start will run. The fixture stands for the engine's answer; the Rust review has its own tests.
 */
function engineReview() {
  return {
    kind: "reviewed", planId: "test-plan",
    review: {
      maximumDurationSeconds: 30.1,
      projectScripted: { activation: { symbol: "P1.ExperimentEnabled" }, restoration: { activationRestoredTo: false }, baselineSeconds: 5, stopConditions: ["Abort"] },
    },
  };
}

function projectLoop() {
  return {
    id: "control-lesson-04",
    projectId: "project:control-lesson-04",
    runtimeId: "simulator",
    revision: "sha256:loop",
    plantId: "proportional-water-heater",
    controller: { instance: "P1.Controller", type: "Control.PID", design: "project", revision: "sha256:controller", action: "direct", taskPeriodMs: 100 },
    signals: {
      processVariable: { symbol: "P1.ProcessValue", address: "%IL0", iecType: "LREAL", direction: "read", unit: "degC", range: { minimum: 0, maximum: 80 } },
      manipulatedVariable: { symbol: "P1.AppliedHeaterPower", address: "%QL0", iecType: "LREAL", direction: "read", unit: "%", range: { minimum: 0, maximum: 100 } },
      additional: [
        { id: "request", channel: "request", role: "MV", symbol: "P1.RequestedHeaterPower", address: "%ML40", iecType: "LREAL", direction: "read", unit: "%", range: { minimum: -200, maximum: 300 } },
        { id: "experiment", channel: "experiment", role: "state", symbol: "P1.ExperimentEnabled", address: "%MX24.1", iecType: "BOOL", direction: "read", unit: "", range: { minimum: 0, maximum: 1 } },
      ],
    },
    limits: { outputMinimum: 0, outputMaximum: 100 },
    observedSampleTiming: { nominalPeriodMs: 100 },
    experiment: { execution: "project-scripted", kind: "observation", amplitude: 0, durationSeconds: 30.1, seed: 1,
      activation: { symbol: "P1.ExperimentEnabled", address: "%MX24.1", iecType: "BOOL", direction: "write" } },
  } as any;
}

/** The settings of an experiment that declares `parameterPurpose: "experiment"` (lesson 21): the step and the hold, as the engine reads them. */
const EXPERIMENT_VALUES = { "P1.SelectedStepPoints": 10, "P1.HoldSeconds": 35 };

interface ExperimentToolPage {
  root: HTMLElement;
  experiments: ThreePageExperiments;
  /** The tool's own step button inside the project test. */
  step: HTMLButtonElement;
  /** The field of one of the loop's settings. */
  field: (symbol: string) => HTMLInputElement;
  messages: Array<{ kind: string; payload: Record<string, unknown> }>;
  /** The DOM window, for the events a field raises. */
  window: any;
}

/** The experiment's settings tool (B6.1, C12) on a DOM with the project test's section, for a form that names it. */
function withExperimentTool(exercise: (page: ExperimentToolPage) => void): void {
  const { JSDOM } = require("jsdom");
  const loop = experimentSettingsLoop();
  const dom = new JSDOM(`<main data-studio-three-page>${projectTestMarkup(loop, "experiment")}</main>`);
  const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
  Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
  try {
    const root = dom.window.document.querySelector("main") as HTMLElement;
    const messages: ExperimentToolPage["messages"] = [];
    const experiments = new ThreePageExperiments(root, answeredForm(loop, "experiment"), (kind, payload) => messages.push({ kind, payload: payload ?? {} }));
    experiments.setSelectedMethod(true);
    const step = root.querySelector("[data-three-page-experiment-parameter-action]") as HTMLButtonElement;
    const field = (symbol: string) => root.querySelector(`[data-parameter-symbol="${symbol}"]`) as HTMLInputElement;
    exercise({ root, experiments, step, field, messages, window: dom.window });
  } finally {
    Object.assign(globalThis, original);
    dom.window.close();
  }
}

/** Takes the experiment's settings through inspect, review and apply, as the engine answers each step. */
function applyExperimentSettings(page: ExperimentToolPage): void {
  const { experiments, step } = page;
  step.click();
  experiments.receive("parameters", { status: "inspected", values: { "P1.SelectedStepPoints": 10, "P1.HoldSeconds": 30 }, offer: FREE });
  step.click();
  experiments.receive("parameters", { status: "reviewed", reviewId: "review-1", values: EXPERIMENT_VALUES, offer: REVIEWED });
  step.click();
  experiments.receive("parameters", { status: "applied", values: EXPERIMENT_VALUES, reason: APPLIED, offer: FREE });
}

function analysisParameterLoop() {
  return {
    ...projectLoop(),
    analysisParameterSymbols: { Flow: "P1.FlowDeviation" },
    parameterBinding: {
      loop_id: "control-lesson-04", task_name: "MainTask",
      mode: { symbol: "P1.ExperimentEnabled", manual_value: false },
      parameters: [{ symbol: "P1.FlowDeviation", iec_type: "LREAL", unit: "L/s", convention: "Flow deviation for the declared trial", minimum: -1, maximum: 1 }],
    },
  } as any;
}

/** A project-scripted loop that declares the experiment's own settings (`parameterPurpose: "experiment"`) and maps no analysis. */
function experimentSettingsLoop() {
  return {
    ...projectLoop(),
    parameterPurpose: "experiment",
    parameterBinding: {
      loop_id: "control-lesson-04", task_name: "MainTask",
      mode: { symbol: "P1.ExperimentEnabled", manual_value: false },
      parameters: [
        { symbol: "P1.SelectedStepPoints", iec_type: "REAL", unit: "percentage-point", convention: "positive step above the baseline", minimum: 5, maximum: 15 },
        { symbol: "P1.HoldSeconds", iec_type: "REAL", unit: "s", convention: "held step duration", minimum: 20, maximum: 40 },
      ],
    },
  } as any;
}
