import * as assert from "assert";
import { ControlChartWorkspace } from "../../controlStudio/webview/chartWorkspace";
import type { AvailableLoopSample } from "../../controlStudio/loopMonitor";

suite("Control Studio recorded signal values", () => {
  const live: AvailableLoopSample = {
    availability: "available", source: "Simulation", loopId: "heater", loopRevision: "r1",
    controller: { instance: "P1.Controller", revision: "c1", mode: "Manual" },
    scan: 20, observedAtMs: 2000, recording: false, saturation: "minimum",
    outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {},
    setpoint: { symbol: "P1.SP", address: "%ML0", unit: "°C", value: 35, quality: "good" },
    processVariable: { symbol: "P1.PV", address: "%IL0", unit: "°C", value: 33, quality: "good" },
    manipulatedVariable: { symbol: "P1.Applied", address: "%IL8", unit: "%", value: 0, quality: "good" },
    additional: [{ id: "request", channel: "heater", role: "state", displayName: "Requested power",
      symbol: "P1.Request", address: "%ML8", unit: "%", value: 0, quality: "good" }],
  };
  const recorded = {
    sp: 35, pv: 35, appliedMv: 75, units: { sp: "°C", pv: "°C", mv: "%" }, quality: "good",
    additional: [{ id: "request", channel: "heater", role: "state", unit: "%", value: 80, quality: "forced" }],
  };

  function withWorkspace(run: (owner: any, rows: () => string[]) => void): void {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM('<main><div data-control-signals></div></main>');
    const original = { window: globalThis.window, document: globalThis.document };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document });
    try {
      const owner = Object.assign(Object.create(ControlChartWorkspace.prototype), {
        root: dom.window.document.querySelector("main"), evidence: { samples: false },
        selectedSignals: new Set(["sp", "pv", "mv", "request"]),
        adapter: { setSignalSelected: () => {}, setOutputLimits: () => {} }, renderTabs: () => {},
      });
      const rows = () => [...dom.window.document.querySelectorAll(".signal-row")]
        .map((row: any) => row.textContent);
      run(owner, rows);
    } finally { Object.assign(globalThis, original); dom.window.close(); }
  }

  test("uses recorded readings and quality with matching live binding metadata", () => withWorkspace((owner, rows) => {
    owner.setMonitor(live);
    owner.setRecordedSample?.(recorded);
    assert.ok(rows().some(row => row.includes("P1.PV") && row.includes("35 °C")), "show recorded PV");
    assert.ok(rows().some(row => row.includes("P1.Applied") && row.includes("75 %")), "show recorded applied MV");
    assert.ok(rows().some(row => row.includes("P1.Request") && row.includes("80 %") && row.includes("forced")),
      "retain the recorded auxiliary value and its own quality");
    assert.strictEqual(recorded.additional[0].value, 80, "presentation must not mutate the saved record");
  }));

  test("keeps explicit open-loop signal names consistent in live and recorded views", () => withWorkspace((owner, rows) => {
    owner.root.dataset.loopTopology = "open-loop";
    owner.setMonitor({ ...live, setpoint: undefined });
    assert.ok(rows().some(row => row.startsWith("Measured output") && row.includes("33 °C")));
    assert.ok(rows().some(row => row.startsWith("Applied input") && row.includes("0 %")));
    assert.ok(!rows().some(row => row.includes("Setpoint")));
    owner.setRecordedSample({ ...recorded, sp: null });
    assert.ok(rows().some(row => row.startsWith("Measured output") && row.includes("35 °C")));
    assert.ok(rows().some(row => row.startsWith("Applied input") && row.includes("75 %")));
    assert.strictEqual(rows().length, 3, "two process channels plus the declared auxiliary channel");
  }));

  test("renders recorded channels without a current runtime", () => withWorkspace((owner, rows) => {
    owner.setRecordedSample?.(recorded);
    assert.strictEqual(rows().length, 4, "saved primary and auxiliary readings remain inspectable offline");
    assert.ok(rows().some(row => row.includes("75 %")));
    assert.ok(rows().some(row => row.includes("80 %") && row.includes("forced")));
  }));

  test("clears historical readings when current observation resumes", () => withWorkspace((owner, rows) => {
    owner.setRecordedSample?.(recorded);
    assert.ok(rows().some(row => row.includes("75 %")), "establish the historical view first");
    owner.setMonitor(live);
    assert.ok(rows().some(row => row.includes("P1.PV") && row.includes("33 °C")));
    assert.ok(rows().some(row => row.includes("P1.Applied") && row.includes("0 %")));
    assert.ok(!rows().some(row => row.includes("forced")), "do not retain old recorded quality on live values");
  }));

  test("does not fill an empty saved result with current readings", () => withWorkspace((owner, rows) => {
    owner.setMonitor(live);
    owner.setRecordedSample?.(undefined);
    assert.deepStrictEqual(rows(), [], "an empty historical result must not borrow live readings");
  }));
});
