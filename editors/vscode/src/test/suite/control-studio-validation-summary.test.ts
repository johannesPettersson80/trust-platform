import * as assert from "assert";
import { validateReceiver } from "../../controlStudio/webview/engineValidate";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

/** One row of the engine's Validate table in the names the page reads (the generated ValidateRow). */
function engineRow(requirement: string, title: string, measured: string, limit: string, verdictText: string): Record<string, unknown> {
  const verdictClass = verdictText.split(" ")[0].toLowerCase();
  return { title, symbol: "", source: "", measured, limit, verdict: verdictClass, verdictText, verdictClass, requirement };
}

/** The page's Validate receiver writes the engine's table into the page; the test reads the rows it wrote. */
function showEngineTable(rows: Record<string, unknown>[]) {
  const dom = new JSDOM(`<main data-studio-loop="null"><table data-studio-validation-result hidden><thead></thead><tbody></tbody></table></main>`);
  const root = dom.window.document.querySelector("main");
  validateReceiver.result!(root, { table: rows, applyAvailable: false }, { receive: () => undefined } as any);
  return {
    window: dom.window,
    rows: (): string[][] => [...root.querySelectorAll("tbody tr")].map((row: any) => [...row.cells].map((cell: any) => cell.textContent)),
    close: () => dom.window.close(),
  };
}

suite("Control Studio validation summary", () => {
  test("keeps the validation evidence when a later Apply fails and recovers", () => {
    const table = showEngineTable([engineRow("response-deadline", "Maximum settling time · Measurement (°C)", "8.5 s", "≤ 12 s", "Pass")]);
    try {
      const validated = table.rows();
      // A failed Apply reaches the page as an engineer payload; the page follows only the stages that leave the validation not current.
      table.window.dispatchEvent(new table.window.CustomEvent("trust-control-studio-engineer", { detail: { status: "failed",
        commissioning: { status: "failed", recoveryConfirmed: true },
        reason: "Auto commissioning failed: first controller output violates reviewed transfer tolerance. Recovery confirmed in Manual." } }));
      assert.deepStrictEqual(table.rows(), validated, "an Apply outcome must not replace the validated rows with 'did not complete'");
    } finally { table.close(); }
  });

  test("shows an allowed-levels limit as the engine words it", () => {
    const table = showEngineTable([engineRow("valve-levels", "Allowed output levels · Output (%)", "—", "12.345, 0.25, 7",
      "Fail · The output is not at an allowed level.")]);
    try {
      assert.strictEqual(table.rows().find(row => row[3].startsWith("Fail"))?.[2], "12.345, 0.25, 7");
    } finally { table.close(); }
  });

  test("keeps every failing check visible as its own row and counts only the passing checks", () => {
    const table = showEngineTable([
      engineRow("controller-state:P1.Controller.StateValue.Setpoint", "State range · Setpoint", "—", "0–40 °C",
        "Fail · The state left its declared range."),
      engineRow("limit-and-controller-checks", "Ranges, states and transfer", "6 checks", "Within declared limits", "Pass"),
    ]);
    try {
      const rows = table.rows();
      const failing = rows.filter(row => row[3].startsWith("Fail"));
      assert.strictEqual(failing.length, 1, "The failed state check stays visible");
      assert.match(failing[0][0], /State range · Setpoint/);
      assert.deepStrictEqual(rows.find(row => row[0] === "Ranges, states and transfer")?.slice(1), ["6 checks", "Within declared limits", "Pass"],
        "The summary counts only the checks that passed");
    } finally { table.close(); }
  });

  test("shows a group's one-setpoint scenario under the name the engine gives it", () => {
    const table = showEngineTable([engineRow("target-error", "Maximum steady-state error · P1.TemperatureC · P1.Zone1C setpoint alone",
      "0.04 °C", "≤ 0.1 °C", "Pass")]);
    try {
      const titles = table.rows().map(row => row[0]);
      assert.ok(titles.some(title => title.endsWith(" · P1.Zone1C setpoint alone")), JSON.stringify(titles));
      assert.ok(!titles.some(title => /independent reference/i.test(title)), `internal scenario ids stay internal: ${JSON.stringify(titles)}`);
    } finally { table.close(); }
  });

  test("shows a switching output's minimum ON and OFF times in the engine's words", () => {
    const table = showEngineTable([
      engineRow("switching-on-dwell", "Minimum ON time · P1.CompressorPct", "75 s", "≥ 20 s", "Pass"),
      engineRow("switching-off-dwell", "Minimum OFF time · P1.CompressorPct", "75 s", "≥ 20 s", "Pass"),
    ]);
    try {
      assert.deepStrictEqual(table.rows().filter(row => /minimum (on|off) time/i.test(row[0])).map(row => row[0]),
        ["Minimum ON time · P1.CompressorPct", "Minimum OFF time · P1.CompressorPct"]);
    } finally { table.close(); }
  });

  test("adds no row of its own after a failed validation, so the transfer run is never listed as missing", () => {
    const table = showEngineTable([engineRow("target-error", "Maximum steady-state error · P1.TemperatureC", "0.247 °C", "≤ 0.2 °C",
      "Fail · The error is above its limit.")]);
    try {
      const rows = table.rows();
      assert.strictEqual(rows.length, 1, "The page writes the engine's rows and no required-scenario row of its own");
      assert.ok(!rows.some(row => /controller transfer/i.test(row[0])), `The transfer run only follows a passing campaign: ${JSON.stringify(rows.map(row => row[0]))}`);
    } finally { table.close(); }
  });
});
