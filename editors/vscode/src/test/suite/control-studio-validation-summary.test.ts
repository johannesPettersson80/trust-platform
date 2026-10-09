import * as assert from "assert";
import { renderValidationResults } from "../../controlStudio/webview/threePageResults";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

type Row = { id: string; kind: string; channel: string; status: "Pass" | "Fail" | "Inconclusive"; value?: number; limit?: number; unit?: string; spec?: Record<string, unknown> };

function evaluation(rows: Row[]) {
  const samples = [0, 1].map(time => ({ timeSeconds: time, sequence: time, values: Object.fromEntries(rows.map(row => [row.channel, { value: 1, quality: "good" }])) }));
  return {
    status: rows.some(row => row.status === "Fail") ? "Fail" : "Pass",
    run: { id: "run", evidence: "ST-executed", parameters: {}, conditions: {}, channels: [], samples },
    specification: rows.map(row => ({ id: row.id, kind: row.kind, channel: row.channel, window: { start: 0, end: 1 }, minimum: 0, maximum: 2, ...row.spec })),
    requirements: rows.map(row => ({ id: row.id, kind: row.kind, status: row.status, value: row.value, limit: row.limit, unit: row.unit ?? "1", reason: row.status === "Pass" ? "" : "Outside the declared limit." })),
    scope: "test",
  };
}

const performance: Row[] = [
  { id: "response-deadline", kind: "settling", channel: "P1.TemperatureC", status: "Pass", value: 8.5, limit: 12, unit: "s" },
  { id: "target-error", kind: "error", channel: "P1.TemperatureC", status: "Pass", value: 0.04, limit: 0.1, unit: "degC" },
];
const integrity = (prefix: string, failing?: string): Row[] => [
  { id: `${prefix}process-bounds`, kind: "bounds", channel: "P1.TemperatureC", status: "Pass" },
  { id: `${prefix}output-bounds`, kind: "bounds", channel: "P1.AppliedPowerPct", status: "Pass" },
  { id: `${prefix}output-rate-up`, kind: "rate", channel: "P1.AppliedPowerPct", status: "Pass", value: 6.8, limit: 25, unit: "%/s", spec: { direction: "up" } },
  { id: `controller-state:${prefix}P1.Controller.StateValue.Setpoint`, kind: "state-bounds", channel: "P1.Controller.StateValue.Setpoint", status: failing === "state" ? "Fail" : "Pass" },
  { id: `controller-state:${prefix}P1.Controller.StateValue.NominalOutput`, kind: "state-bounds", channel: "P1.Controller.StateValue.NominalOutput", status: "Pass" },
  { id: `fixed-parameter:${prefix}P1.Parameters.ProportionalGain`, kind: "bounds", channel: "P1.Parameters.ProportionalGain", status: "Pass" },
  { id: `fixed-parameter:${prefix}P1.Parameters.IntegralGain`, kind: "bounds", channel: "P1.Parameters.IntegralGain", status: "Pass" },
];

function render(payload: Record<string, unknown>): string[][] {
  const dom = new JSDOM(`<main data-studio-loop="null"><table data-studio-validation-result hidden><thead></thead><tbody></tbody></table></main>`);
  const previousDocument = (globalThis as any).document, previousWindow = (globalThis as any).window;
  (globalThis as any).document = dom.window.document; (globalThis as any).window = dom.window;
  try {
    const root = dom.window.document.querySelector("main");
    renderValidationResults(root, payload);
    return [...root.querySelectorAll("tbody tr")].map((row: any) => [...row.cells].map((cell: any) => cell.textContent));
  } finally { (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); }
}

suite("Control Studio validation summary", () => {
  test("keeps the validation evidence when a later Apply fails and recovers", () => {
    const dom = new JSDOM(`<main data-studio-loop="null"><table data-studio-validation-result hidden><thead></thead><tbody></tbody></table></main>`);
    const previousDocument = (globalThis as any).document, previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document; (globalThis as any).window = dom.window;
    try {
      const root = dom.window.document.querySelector("main");
      const rows = () => [...root.querySelectorAll("tbody tr")].map((row: any) => [...row.cells].map((cell: any) => cell.textContent));
      const required = ["target-response", "load-trial"];
      renderValidationResults(root, { status: "verified", requiredScenarios: required, initialization: evaluation(integrity("transfer-")), scenarioResults: [
        { id: "target-response", result: { evaluation: evaluation([...performance, ...integrity("")]) } },
        { id: "load-trial", result: { evaluation: evaluation(integrity("")) } }] });
      const validated = rows();
      renderValidationResults(root, { status: "failed", requiredScenarios: required, initialization: evaluation(integrity("transfer-")),
        commissioning: { status: "failed", recoveryConfirmed: true },
        reason: "Auto commissioning failed: first controller output violates reviewed transfer tolerance. Recovery confirmed in Manual." });
      assert.deepStrictEqual(rows(), validated, "an Apply outcome must not replace the validated scenarios with 'did not complete'");
    } finally { (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); }
  });

  test("shows the control requirements and one row for all passing limit and controller checks", () => {
    const rows = render({ status: "verified", scenarioResults: [{ id: "target-response", result: { evaluation: evaluation([...performance, ...integrity("")]) } }],
      initialization: evaluation(integrity("transfer-")) });
    assert.deepStrictEqual(rows.map(row => row[0]), ["Maximum settling time · P1.TemperatureC", "Maximum steady-state error · P1.TemperatureC", "Ranges, states and transfer"],
      "Internal range, state, parameter and transfer checks must not bury the requirements");
    assert.deepStrictEqual(rows[2].slice(1), ["14 checks", "Within declared limits", "Pass"]);
  });

  test("lists allowed levels with one precision", () => {
    const rows = render({ status: "verification-failed", scenarioResults: [{ id: "target-response", result: { evaluation: evaluation([
      { id: "valve-levels", kind: "levels", channel: "P1.ValvePct", status: "Fail", unit: "1", spec: { allowed: [12.345, 0.25, 7], tolerance: 0.01 } }]) } }] });
    assert.strictEqual(rows.find(row => row[3].startsWith("Fail"))?.[2], "12.345, 0.25, 7");
  });

  test("keeps every failing check visible as its own row", () => {
    const rows = render({ status: "verification-failed", scenarioResults: [{ id: "target-response", result: { evaluation: evaluation([...performance, ...integrity("", "state")]) } }] });
    const failing = rows.filter(row => row[3].startsWith("Fail"));
    assert.strictEqual(failing.length, 1, "The failed state check stays visible");
    assert.match(failing[0][0], /State range · Setpoint/);
    assert.deepStrictEqual(rows.find(row => row[0] === "Ranges, states and transfer")?.slice(1), ["6 checks", "Within declared limits", "Pass"],
      "The summary counts only the checks that passed");
  });

  test("keeps a lesson's declared scenario limits visible even when they pass", () => {
    const declared: Row[] = [
      { id: "load-minimum", kind: "bounds", channel: "P1.TemperatureC", status: "Pass", spec: { minimum: 33, maximum: 80 } },
      { id: "load-late-error", kind: "error", channel: "P1.TemperatureC", status: "Pass", value: 0.03, limit: 0.2, unit: "degC" },
    ];
    const rows = render({ status: "verified", scenarioResults: [
      { id: "target-response", result: { evaluation: evaluation([...performance, ...integrity("")]) } },
      { id: "load-trial", result: { evaluation: evaluation([...declared, ...integrity("")]) } }] });
    assert.ok(rows.some(row => row[0] === "Operating range · P1.TemperatureC · load trial"),
      `The declared minimum-temperature limit of the load trial must stay visible: ${JSON.stringify(rows.map(row => row[0]))}`);
    assert.deepStrictEqual(rows.find(row => row[0] === "Ranges, states and transfer")?.slice(1), ["14 checks", "Within declared limits", "Pass"]);
  });

  test("names an error measured from another reference after that reference, not a steady-state error", () => {
    // A loop always declares its manipulated variable (ControlLoopDefinition.signals), so the fixture declares one too.
    const loop = { signals: { setpoint: { symbol: "P1.SetpointRad" }, processVariable: { symbol: "P1.AngleRad" },
      manipulatedVariable: { symbol: "P1.ValvePct", address: "%QW0", iecType: "REAL", direction: "write", unit: "%", range: { minimum: 0, maximum: 100 } },
      additional: [{ id: "planned-angle", role: "state", symbol: "P1.ReferenceRad" }] }, verificationScenarios: [{ id: "planned-move" }] };
    const dom = new JSDOM(`<main><table data-studio-validation-result hidden><thead></thead><tbody></tbody></table></main>`);
    const previousDocument = (globalThis as any).document, previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document; (globalThis as any).window = dom.window;
    try {
      const root = dom.window.document.querySelector("main");
      root.dataset.studioLoop = JSON.stringify(loop);
      const following: Row[] = [{ id: "planned-move-following-error", kind: "error", channel: "P1.AngleRad", status: "Fail", value: 0.0922, limit: 0.03, unit: "rad",
        spec: { reference: { channel: "P1.ReferenceRad" } } }];
      renderValidationResults(root, { status: "verification-failed", scenarioResults: [
        { id: "target-response", result: { evaluation: evaluation([...performance, ...integrity("")]) } },
        { id: "planned-move", result: { evaluation: evaluation([...following, ...integrity("")]) } }] });
      const rows = [...root.querySelectorAll("tbody tr")].map((row: any) => row.cells[0].textContent);
      assert.ok(rows.includes("Maximum error from planned angle · Measurement · planned move"), JSON.stringify(rows));
      assert.ok(rows.includes("Maximum steady-state error · P1.TemperatureC"), "the task's own error check keeps its name");
    } finally { (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); }
  });

  test("names a group's one-setpoint scenarios after the setpoint they change", () => {
    const rows = render({ status: "verified", scenarioResults: [
      { id: "target-response", result: { evaluation: evaluation([...performance, ...integrity("")]) } },
      { id: "independent-reference:P1.Zone1C", result: { evaluation: evaluation([...performance, ...integrity("")]) } }] });
    const titles = rows.map(row => row[0]);
    assert.ok(titles.some(title => title.endsWith(" · P1.Zone1C setpoint alone")), JSON.stringify(titles));
    assert.ok(!titles.some(title => /independent reference/i.test(title)), `internal scenario ids stay internal: ${JSON.stringify(titles)}`);
  });

  test("names a switching output's minimum ON and OFF times", () => {
    const dwell: Row[] = [
      { id: "switching-on-dwell", kind: "dwell", channel: "P1.CompressorPct", status: "Pass", value: 75, limit: 20, unit: "s", spec: { state: "high", threshold: 50, minimumSeconds: 20 } },
      { id: "switching-off-dwell", kind: "dwell", channel: "P1.CompressorPct", status: "Pass", value: 75, limit: 20, unit: "s", spec: { state: "low", threshold: 50, minimumSeconds: 20 } },
    ];
    const rows = render({ status: "verified", scenarioResults: [{ id: "target-response", result: { evaluation: evaluation([...performance, ...dwell]) } }] });
    assert.deepStrictEqual(rows.filter(row => /minimum (on|off) time/i.test(row[0])).map(row => row[0]),
      ["Minimum ON time · P1.CompressorPct", "Minimum OFF time · P1.CompressorPct"], JSON.stringify(rows.map(row => row[0])));
  });

  test("does not list the transfer run as missing when validation already failed", () => {
    const failing: Row[] = [{ id: "load-late-error", kind: "error", channel: "P1.TemperatureC", status: "Fail", value: 0.247, limit: 0.2, unit: "degC" }];
    const rows = render({ status: "verification-failed", requiredScenarios: ["target-response", "load-trial", "controller-transfer"],
      scenarioResults: [{ id: "target-response", result: { evaluation: evaluation(performance) } }, { id: "load-trial", result: { evaluation: evaluation(failing) } }] });
    assert.ok(rows.some(row => row[3].startsWith("Fail")), "The failed requirement explains why Apply is unavailable");
    assert.ok(!rows.some(row => /controller transfer/i.test(row[0])),
      `The transfer run only follows a passing campaign, so it is not a missing check here: ${JSON.stringify(rows.map(row => row[0]))}`);
  });
});
