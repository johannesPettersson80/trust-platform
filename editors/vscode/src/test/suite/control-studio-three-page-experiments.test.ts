import * as assert from "assert";
import { ThreePageLiveSignals } from "../../controlStudio/webview/threePageLiveSignals";
import { recordedSignalResponses } from "../../controlStudio/webview/threePageRecordedSignals";

import { renderThreePageExperimentTemplate } from "../../controlStudio/threePageExperimentTemplate";
import { ThreePageExperiments } from "../../controlStudio/webview/threePageExperiments";
import type { ControlExperimentPlan } from "../../controlStudio/experiment";

suite("Control Studio three-page project experiments", () => {
  test("restores the recording outcome without replacing an active runtime operation", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageExperimentTemplate(projectLoop(), value => value)}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      let recordings = 0;
      dom.window.addEventListener("trust-control-studio-experiment-recording", () => recordings++);
      const root = dom.window.document.querySelector("main");
      const experiments = new ThreePageExperiments(root, projectLoop(), () => assert.fail("Restoring a view cannot write"));
      const history = { recording: { rawSamples: [] }, experiment: { status: "aborted", restoration: { status: "confirmed" } } };
      experiments.receive("recording", history);
      assert.match(root.querySelector("[data-three-page-experiment-status]").textContent, /Aborted recording/);
      assert.strictEqual(experiments.busy, false);
      experiments.receive("running", { experiment_id: "new-run" });
      experiments.receive("recording", history);
      assert.strictEqual(recordings, 1, "Old history must not replace the active experiment chart");
      assert.strictEqual(experiments.busy, true);
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, "Test running…");
    } finally { Object.assign(globalThis, original); dom.window.close(); }
  });

  test("starts a fresh live chart once per runtime experiment, without replaying status polls", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageExperimentTemplate(projectLoop(), value => value)}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const starts: string[] = [];
      dom.window.addEventListener("trust-control-studio-experiment-started", (event: any) => starts.push(event.detail.experimentId));
      const experiments = new ThreePageExperiments(dom.window.document.querySelector("main"), projectLoop(), () => assert.fail("Observing runtime status cannot write"));
      experiments.receive("running", { experiment_id: "first" });
      experiments.receive("running", { experiment_id: "first" });
      assert.deepStrictEqual(starts, ["first"], "Only a new acquisition should replace the retained chart");
      experiments.receive("terminal", { status: "completed", restoration: "restored" });
      experiments.receive("running", { experiment_id: "second" });
      assert.deepStrictEqual(starts, ["first", "second"]);
    } finally { Object.assign(globalThis, original); dom.window.close(); }
  });

  test("projects the selected setpoint and diagnostics from actual live readings", () => {
    const loop = projectLoop();
    loop.signals.setpoint = { ...loop.signals.processVariable, symbol: "P1.Setpoint", address: "%ML0" };
    const live = new ThreePageLiveSignals(loop);
    for (const scan of [10, 11]) live.receive({ availability: "available", source: "Simulation", loopId: loop.id,
      loopRevision: loop.revision, scan, observedAtMs: scan * 100, runtimeSessionId: "runtime-1",
      controller: { instance: "P1.Controller", revision: "r1", mode: "Auto" },
      setpoint: { ...loop.signals.setpoint, value: 35, quality: "good" },
      processVariable: { ...loop.signals.processVariable, value: 34 + scan / 100, quality: "good" },
      manipulatedVariable: { ...loop.signals.manipulatedVariable, value: 75, quality: "good" },
      additional: [{ ...loop.signals.additional[0], displayName: "Requested power", value: 110,
        quality: scan === 10 ? "good" : "bad" }],
      outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, saturation: "none", recording: false,
    });
    const responses = Reflect.apply(live.responses, live,
      [[loop.signals.manipulatedVariable.symbol], [loop.signals.processVariable.symbol], ["sp", "pv", "request"]]);
    const series = responses.flatMap((group: any) => group.series);
    assert.deepStrictEqual(series.map((item: any) => item.name), ["P1.Setpoint", "P1.ProcessValue", "P1.RequestedHeaterPower"],
      "The experiment selection must determine the live chart, including SP and diagnostics");
    assert.deepStrictEqual(series[0].points, [[0, 35], [0.1, 35]]);
    assert.deepStrictEqual(series[2].points, [[0, 110], [0.1, null]], "Bad-quality diagnostics must remain gaps");
    assert.strictEqual(series[0].dashed, true, "Setpoint must stay distinct from coincident process value");
    const recorded = recordedSignalResponses(loop, [{ elapsedSeconds: 0, sp: 35, pv: 34.1,
      requestedMv: 75, appliedMv: 75, quality: "good", additional: [{ id: "request", channel: "request", role: "MV",
        unit: "%", value: 110, quality: "good" }] }] as any, ["sp", "pv", "request"]).flatMap(group => group.series);
    assert.deepStrictEqual(recorded.map(item => [item.name, item.points[0]]), series.map((item: any) => [item.name, item.points[0]]),
      "Live and recorded views must retain the same signal identity and value");
  });
  test("keeps project instructions compact and hides the reviewed plan while running", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageExperimentTemplate(projectLoop(), value => value)}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const root = dom.window.document.querySelector("main");
      const experiments = new ThreePageExperiments(root, projectLoop(), () => undefined);
      experiments.setSelectedMethod(true);
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").hidden, true,
        "The review button already explains the first action");
      experiments.receive("review", { kind: "accepted", plan: projectPlan() });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-plan]").hidden, false);
      assert.match(root.querySelector("[data-three-page-experiment-restoration]").textContent, /P1.ExperimentEnabled = FALSE/,
        "Show the actual restored signal value rather than ambiguous Restoration Disabled");
      experiments.receive("running", { experiment_id: "run-1" });
      assert.strictEqual(root.querySelector("[data-three-page-experiment-plan]").hidden, true);
      assert.strictEqual(root.querySelector("[data-three-page-experiment-status]").textContent, "Test running…");
      assert.strictEqual(experiments.primaryActionLabel, "Abort test");
      experiments.receive("terminal", { status: "completed", restoration: "pending" });
      assert.match(root.querySelector("[data-three-page-experiment-status]").textContent, /restoration remains pending/,
        "Unresolved runtime restoration must remain visible");
    } finally { Object.assign(globalThis, original); dom.window.close(); }
  });

  test("renders a contextual project experiment workspace with declared signals", () => {
    const loop = projectLoop();
    const html = renderThreePageExperimentTemplate(loop, value => value);
    assert.match(html, /data-three-page-experiment/);
    assert.match(html, /data-three-page-experiment-signals/);
    assert.match(html, /P1\.ExperimentEnabled/);
    assert.match(html, /P1\.RequestedHeaterPower/);
  });

  test("provides the shared parameter field host only for an experiment binding", () => {
    const html = renderThreePageExperimentTemplate(experimentParameterLoop(), value => value);
    assert.match(html, /data-three-page-experiment-parameters/);
    assert.match(html, /data-three-page-experiment-parameter-fields/);
    assert.doesNotMatch(renderThreePageExperimentTemplate(projectLoop(), value => value), /data-three-page-experiment-parameters/);
  });

  test("reviews and starts the declared project sequence, then aborts the same runtime experiment", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main data-studio-three-page>${renderThreePageExperimentTemplate(projectLoop(), value => value)}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const messages: Array<{ kind: string; payload: Record<string, unknown> }> = [];
      const experiments = new ThreePageExperiments(dom.window.document.querySelector("main") as HTMLElement, projectLoop(),
        (kind, payload) => messages.push({ kind, payload: payload ?? {} }));
      experiments.setSelectedMethod(true);
      assert.strictEqual(experiments.primaryAction(), true);
      assert.deepStrictEqual(messages[0], {
        kind: "control-studio.experiment.review",
        payload: { kind: "observation", amplitude: 0, durationSeconds: 30.1, physicalTopologyReviewed: false },
      });
      experiments.receive("review", { kind: "accepted", plan: projectPlan() });
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
    const dom = new JSDOM(`<main data-studio-three-page>${renderThreePageExperimentTemplate(projectLoop(), value => value)}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const events: string[][] = [];
      dom.window.addEventListener("trust-control-studio-experiment-signals", (event: Event) => {
        events.push((event as CustomEvent<{ signalIds: string[] }>).detail.signalIds);
      });
      const experiments = new ThreePageExperiments(dom.window.document.querySelector("main") as HTMLElement, projectLoop(), () => undefined);
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

  test("reviews and applies exact experiment parameters before project-plan review", () => {
    const { JSDOM } = require("jsdom");
    const loop = experimentParameterLoop();
    const dom = new JSDOM(`<main data-studio-three-page>${renderThreePageExperimentTemplate(loop, value => value)}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const messages: Array<{ kind: string; payload: Record<string, unknown> }> = [];
      const experiments = new ThreePageExperiments(dom.window.document.querySelector("main") as HTMLElement, loop,
        (kind, payload) => messages.push({ kind, payload: payload ?? {} }));
      experiments.setSelectedMethod(true);
      assert.strictEqual(experiments.primaryAction(), true);
      assert.deepStrictEqual(messages[0], { kind: "control-studio.parameters.inspect", payload: {} });
      experiments.receive("parameters", { status: "inspected", values: { "P1.FlowDeviation": 0.2 }, binding: loop.parameterBinding });
      const field = dom.window.document.querySelector('[data-parameter-symbol="P1.FlowDeviation"]') as HTMLInputElement;
      assert.strictEqual(field.value, "0.2", "The inspected value reaches the existing shared numeric field");
      assert.match(dom.window.document.querySelector(`label[for="${field.id}"]`)!.textContent!, /Flow deviation/i);
      assert.match(dom.window.document.getElementById(field.getAttribute("aria-describedby")!)!.textContent!, /L\/s/);
      assert.strictEqual(experiments.primaryAction(), true);
      assert.deepStrictEqual(messages[1], { kind: "control-studio.parameters.review", payload: { json: JSON.stringify({ "P1.FlowDeviation": 0.2 }, null, 2) } });
      experiments.receive("parameters", { status: "reviewed", reviewId: "review-1", values: { "P1.FlowDeviation": 0.2 }, observed: { values: { "P1.FlowDeviation": 0.1 } }, source: "simulation", binding: loop.parameterBinding });
      assert.strictEqual(experiments.primaryAction(), true);
      assert.deepStrictEqual(messages[2], { kind: "control-studio.parameters.apply", payload: { reviewId: "review-1", physicalApproved: false } });
      experiments.receive("parameters", { status: "applied", observed: { values: { "P1.FlowDeviation": 0.2 } } });
      assert.strictEqual(experiments.primaryAction(), true);
      assert.strictEqual(messages[3].kind, "control-studio.experiment.review");
      assert.strictEqual(messages[3].payload.durationSeconds, 30.1);
      experiments.receive("review", { kind: "accepted", plan: projectPlan() });
      experiments.primaryAction();
      experiments.receive("running", { experiment_id: "test-run" });
      experiments.primaryAction();
      experiments.receive("terminal", { status: "completed", restoration: "restored" });
      assert.strictEqual(experiments.primaryActionLabel, "Review parameter values");
      assert.strictEqual((dom.window.document.querySelector("[data-three-page-experiment-parameter-fields] input") as HTMLInputElement).disabled, false);
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });
});

function projectPlan(): ControlExperimentPlan {
  return {
    id: "test-plan", source: "simulation", kind: "project_sequence", affectedSignal: "P1.ExperimentEnabled",
    address: "%MX24.1", valueType: "BOOL", from: false, to: true, unit: "authority", minimum: 0, maximum: 1,
    durationMs: 30100, expectedScan: 1, stopOnStaleMs: 500, stopConditions: ["Abort"], restorationValue: false,
    requiresPhysicalApproval: false, trajectory: [],
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

function experimentParameterLoop() {
  return {
    ...projectLoop(),
    parameterPurpose: "experiment",
    parameterBinding: {
      loop_id: "control-lesson-04", task_name: "MainTask",
      mode: { symbol: "P1.ExperimentEnabled", manual_value: false },
      parameters: [{ symbol: "P1.FlowDeviation", iec_type: "LREAL", unit: "L/s", convention: "Flow deviation for the declared trial", minimum: -1, maximum: 1 }],
    },
  } as any;
}
