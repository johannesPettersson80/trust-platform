import * as assert from "assert";
import { evaluateRecordedRun } from "../../controlStudio/runValidation";
import { renderValidationResults } from "../../controlStudio/webview/threePageResults";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

/** Applied command 75 → 80 → 78 → 78 % sampled every 0.1 s. */
function commandRun(): any {
  const commands = [75, 80, 78, 78];
  return { id: "trial", evidence: "ST-executed", parameters: {}, channels: [{ id: "P1.AppliedPowerPct", unit: "%" }],
    samples: commands.map((value, sequence) => ({ timeSeconds: sequence / 10, sequence, values: { "P1.AppliedPowerPct": { value, quality: "good" } } })),
    conditions: { initialState: {}, events: [], durationSeconds: 0.3, sampleTimesSeconds: [0, 0.1, 0.2, 0.3], seed: 1,
      modelRevision: "model", programRevision: "program", parameterSchemaRevision: "schema" } };
}
const activity = { id: "command-activity", kind: "activity", channel: "P1.AppliedPowerPct", window: { start: 0, end: 0.3 } };

function evaluate(requirement: Record<string, unknown>): any {
  try { return evaluateRecordedRun(commandRun(), [requirement]).requirements[0]; } catch (error) { return { error: String(error) }; }
}

suite("Control Studio command activity", () => {
  test("measures the total applied-command movement in percentage points", () => {
    const measured = evaluate(activity);
    assert.strictEqual(measured.error, undefined, `Command activity must be a supported measurement: ${measured.error}`);
    assert.strictEqual(measured.value, 7, "75 → 80 → 78 → 78 moves the command 5 + 2 + 0 = 7 points");
    assert.strictEqual(measured.unit, "pp", "Activity of a percentage command is counted in percentage points");
    assert.strictEqual(measured.limit, undefined, "Without a declared maximum the activity is a measurement");
    assert.strictEqual(measured.status, "Pass", "A measurement alone does not fail a trial");
    assert.strictEqual(evaluate({ ...activity, maximum: 6 }).status, "Fail", "A declared maximum makes it a requirement");
  });

  test("shows command activity as a measured row", () => {
    const run = commandRun();
    const evaluation = { status: "Pass", run, specification: [activity], scope: "test",
      requirements: [{ id: "command-activity", kind: "activity", status: "Pass", value: 7, unit: "pp", reason: "Measured command movement." }] };
    const dom = new JSDOM(`<main data-studio-loop="null"><table data-studio-validation-result hidden><thead></thead><tbody></tbody></table></main>`);
    const previousDocument = (globalThis as any).document, previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document; (globalThis as any).window = dom.window;
    try {
      const root = dom.window.document.querySelector("main");
      renderValidationResults(root, { status: "verified", scenarioResults: [
        { id: "setpoint-trial", result: { evaluation } }, { id: "load-trial", result: { evaluation } }] });
      const row = [...root.querySelectorAll("tbody tr")].map((item: any) => [...item.cells].map((cell: any) => cell.textContent))[0];
      assert.deepStrictEqual(row, ["Command activity · P1.AppliedPowerPct · setpoint trial", "7 percentage points", "—", "Measured"]);
    } finally { (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); }
  });
});
