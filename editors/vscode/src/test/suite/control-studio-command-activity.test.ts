import * as assert from "assert";
import { validateReceiver } from "../../controlStudio/webview/engineValidate";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

suite("Control Studio command activity", () => {
  test("shows command activity as a measured row", () => {
    // The engine's Validate table holds the row (ValidateResult.table); the page writes the words the engine gives it.
    const dom = new JSDOM(`<main data-studio-loop="null"><table data-studio-validation-result hidden><thead></thead><tbody></tbody></table></main>`);
    try {
      const root = dom.window.document.querySelector("main");
      validateReceiver.result!(root, { applyAvailable: false, table: [{ title: "Command activity · P1.AppliedPowerPct · setpoint trial",
        symbol: "P1.AppliedPowerPct", source: "", measured: "7 percentage points", limit: "—", verdict: "measured", verdictText: "Measured",
        verdictClass: "", requirement: "command-activity" }] }, { receive: () => undefined } as any);
      const row = [...root.querySelectorAll("tbody tr")].map((item: any) => [...item.cells].map((cell: any) => cell.textContent))[0];
      assert.deepStrictEqual(row, ["Command activity · P1.AppliedPowerPct · setpoint trial", "7 percentage points", "—", "Measured"]);
    } finally { dom.window.close(); }
  });
});
