/// <reference lib="dom" />
import * as assert from "assert";
const { JSDOM } = require("jsdom");
import { initializeEngineerEvidencePresentation } from "../../controlStudio/webview/engineerEvidencePresentation";

suite("Control Studio engineer evidence presentation", () => {
  test("does not carry an old independent fit into a replacement model without fit evidence", () => {
    const dom = new JSDOM("<div data-engineer-evidence></div>");
    const original = (global as unknown as { document?: Document }).document;
    (global as unknown as { document: Document }).document = dom.window.document;
    try {
      const evidence = dom.window.document.querySelector("[data-engineer-evidence]") as HTMLElement;
      const render = initializeEngineerEvidencePresentation(evidence, { render: () => undefined, renderScenario: () => undefined, renderCommissioning: () => undefined });
      render({ status: "model-ready", model: { source: "identified", revision: "model-a" },
        identificationPresentation: { equation: "old fitted representation", validation: { normalizedError: 0.028 } } });
      assert.match(evidence.textContent ?? "", /Independent fit error/);
      render({ status: "candidate" });
      assert.match(evidence.textContent ?? "", /Independent fit error/, "ordinary progress retains the current model fit");
      render({ status: "model-ready", model: { source: "identified", revision: "model-b" } });
      assert.doesNotMatch(evidence.querySelector("[data-engineer-evidence-stage=model]")?.textContent ?? "", /Independent fit error|old fitted representation/,
        "replacement models cannot inherit another model's validation or equation");
    } finally {
      (global as unknown as { document?: Document }).document = original;
      dom.window.close();
    }
  });
  test("reviews each changed parameter in aligned rows and retains unchanged settings behind disclosure", () => {
    const dom = new JSDOM("<div data-engineer-evidence></div>");
    const previousDocument = (global as unknown as { document?: Document }).document;
    (global as unknown as { document: Document }).document = dom.window.document;
    try {
      const evidence = dom.window.document.querySelector("[data-engineer-evidence]") as HTMLElement;
      const render = initializeEngineerEvidencePresentation(evidence, { render: () => undefined, renderScenario: () => undefined, renderCommissioning: () => undefined });
      const previous = { "P1.Parameters.ProportionalGain": 1, "P1.Parameters.IntegralGain": 0.2, "P1.Parameters.OutputMaximum": 100 };
      const next = { ...previous, "P1.Parameters.ProportionalGain": 0.93615763, "P1.Parameters.IntegralGain": 0.1717162 };
      render({ status: "review", previous, next, parameterDescriptors: [
        { symbol: "P1.Parameters.ProportionalGain", unit: "%/°C" },
        { symbol: "P1.Parameters.IntegralGain", unit: "%/(°C*s)" },
        { symbol: "P1.Parameters.OutputMaximum", unit: "%" },
      ] });
      const rows = evidence.querySelectorAll<HTMLTableRowElement>("[data-parameter-comparison] tbody tr");
      assert.strictEqual(rows.length, 3, "every bound parameter has its own review row");
      const proportional = Array.from(rows).find(row => row.dataset.parameterSymbol === "P1.Parameters.ProportionalGain")!;
      assert.ok(proportional && !proportional.closest("details"), "changed settings are immediately visible");
      assert.ok(!proportional.textContent?.includes("P1.Parameters"), "qualified binding paths do not clutter displayed labels");
      assert.match(proportional.textContent ?? "", /%\/°C/);
      assert.ok(proportional.querySelector('[title="P1.Parameters.ProportionalGain"]'), "retain the exact binding for inspection");
      const unchanged = Array.from(rows).find(row => row.dataset.parameterSymbol === "P1.Parameters.OutputMaximum")!;
      assert.ok(unchanged.closest("details") && !unchanged.closest("details")!.open);
      assert.strictEqual(next["P1.Parameters.ProportionalGain"], 0.93615763, "display formatting does not change adopted values");
    } finally {
      if (previousDocument) (global as unknown as { document: Document }).document = previousDocument; else delete (global as unknown as { document?: Document }).document;
      dom.window.close();
    }
  });
  test("formats reviewed numerical settings without raw floating point tails", () => {
    const dom = new JSDOM("<div data-engineer-evidence></div>");
    const previousDocument = (global as unknown as { document?: Document }).document;
    (global as unknown as { document: Document }).document = dom.window.document;
    try {
      const evidence = dom.window.document.querySelector("[data-engineer-evidence]") as HTMLElement;
      const render = initializeEngineerEvidencePresentation(evidence, { render: () => undefined, renderScenario: () => undefined, renderCommissioning: () => undefined });
      const next = { "Main.Kp": 11.7110775862811 };
      render({ status: "review", previous: { "Main.Kp": 1 }, next });
      const text = evidence.querySelector("[data-engineer-evidence-stage=apply]")?.textContent ?? "";
      assert.ok(!text.includes("11.7110775862811"), "Review uses shared engineering precision");
      assert.match(text, /11\.71\b/);
      assert.strictEqual(next["Main.Kp"], 11.7110775862811, "Formatting cannot change the reviewed value");
    } finally {
      (global as unknown as { document?: Document }).document = previousDocument;
      dom.window.close();
    }
  });
  test("groups evidence by stage, retains the selected model, and keeps scenario actions", () => {
    const dom = new JSDOM("<div data-engineer-evidence></div>");
    const previousDocument = (global as unknown as { document?: Document }).document;
    (global as unknown as { document: Document }).document = dom.window.document;
    try {
      const evidence = dom.window.document.querySelector("[data-engineer-evidence]") as HTMLElement;
      let scenario: string | undefined;
      const responses = {
        render: () => undefined,
        renderScenario: (label: string) => { scenario = label; },
        renderCommissioning: () => undefined,
      };
      const render = initializeEngineerEvidencePresentation(evidence, responses);
      render({ status: "model-ready", model: { source: "fitted", revision: "private-revision", inputs: [{ id: "MV", unit: "%" }], outputs: [{ id: "PV", unit: "°C" }] } });
      render({ status: "candidate", scenarioResults: [{ id: "step", result: { evaluation: {
        status: "Pass", run: { evidence: "ST-executed", channels: [], samples: [] },
        specification: [{ id: "error", kind: "error", channel: "PV", maximum: 1, window: { start: 0, end: 1 } }],
      } } }] });
      assert.deepStrictEqual(Array.from(evidence.querySelectorAll<HTMLElement>("[data-engineer-evidence-stage]")).map(node => node.dataset.engineerEvidenceStage), ["model", "tune", "verify", "apply"]);
      assert.match(evidence.querySelector("[data-engineer-evidence-stage=model]")?.textContent ?? "", /fitted/);
      assert.match(evidence.querySelector("[data-engineer-evidence-stage=model]")?.textContent ?? "", /MV \(%\)/);
      assert.strictEqual(evidence.querySelectorAll("[data-engineer-evidence-stage=model] details").length, 1, "revision identity stays behind disclosure");
      (evidence.querySelector("[data-engineer-evidence-stage=verify] button") as HTMLButtonElement).click();
      assert.strictEqual(scenario, "step");
      assert.match(evidence.querySelector("[data-engineer-evidence-stage=verify]")?.textContent ?? "", /error/);
    } finally {
      if (previousDocument) (global as unknown as { document: Document }).document = previousDocument; else delete (global as unknown as { document?: Document }).document;
    }
  });

  test("keeps recovery and observed application records visible", () => {
    const dom = new JSDOM("<div data-engineer-evidence></div>");
    const previousDocument = (global as unknown as { document?: Document }).document;
    (global as unknown as { document: Document }).document = dom.window.document;
    try {
      const evidence = dom.window.document.querySelector("[data-engineer-evidence]") as HTMLElement;
      const render = initializeEngineerEvidencePresentation(evidence, { render: () => undefined, renderScenario: () => undefined, renderCommissioning: () => undefined });
      render({ status: "review", previous: { gain: 1 }, next: { gain: 2 }, observed: { values: { PV: 21 } }, initialization: { status: "Pass" }, commissioning: { status: "recovery-required", target: 20, recoveryConfirmed: false, recording: { rawSamples: [1, 2] } } });
      const text = evidence.querySelector("[data-engineer-evidence-stage=apply]")?.textContent ?? "";
      assert.match(text, /Current \/ reviewed/);
      assert.match(text, /observed values/i);
      assert.match(text, /Transfer verification.*Pass/);
      assert.match(text, /Recovery required/);
    } finally {
      if (previousDocument) (global as unknown as { document: Document }).document = previousDocument; else delete (global as unknown as { document?: Document }).document;
    }
  });

  test("retains design and verification while later payloads omit them, then marks edits historical", () => {
    const dom = new JSDOM("<div data-engineer-evidence></div>");
    const previousDocument = (global as unknown as { document?: Document }).document;
    (global as unknown as { document: Document }).document = dom.window.document;
    try {
      const evidence = dom.window.document.querySelector("[data-engineer-evidence]") as HTMLElement;
      const render = initializeEngineerEvidencePresentation(evidence, { render: () => undefined, renderScenario: () => undefined, renderCommissioning: () => undefined });
      const evaluation = { status: "Pass", run: { evidence: "ST-executed", channels: [], samples: [] }, requirements: [{ id: "error", status: "Pass", value: 0.2, limit: 1, unit: "°C", reason: "Within limit" }] };
      render({ status: "candidate", design: { candidate: { proportionalGain: 2 } }, candidate: evaluation, baseline: evaluation });
      render({ status: "verified" });
      render({ status: "review", previous: { gain: 1 }, next: { gain: 2 } });
      let verifyText = evidence.querySelector("[data-engineer-evidence-stage=verify]")?.textContent ?? "";
      assert.match(verifyText, /ST-executed/);
      assert.match(verifyText, /error.*Pass/);
      assert.match(evidence.querySelector("[data-engineer-evidence-stage=tune]")?.textContent ?? "", /Unapplied P gain/);
      assert.match(evidence.querySelector("[data-engineer-evidence-stage=apply]")?.textContent ?? "", /Current \/ reviewed/);
      render({ status: "edited" });
      verifyText = evidence.querySelector("[data-engineer-evidence-stage=verify]")?.textContent ?? "";
      assert.match(verifyText, /Historical verification retained/);
      assert.match(verifyText, /invalid/);
      render({ status: "cancelled" });
      assert.match(evidence.querySelector("[data-engineer-evidence-stage=tune]")?.textContent ?? "", /historical/i);
    } finally {
      (global as unknown as { document?: Document }).document = previousDocument;
    }
  });
});
