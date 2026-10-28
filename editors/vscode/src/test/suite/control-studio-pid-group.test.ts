import * as assert from "assert";

import { declaredPidGroup, mimoLoop } from "./control-studio-pid-group-fixtures";
import { renderThreePageStudioHtml } from "../../controlStudio/threePageTemplate";

suite("Control Studio declared PID groups", () => {
  test("names every PID member in the one Controller selector", () => {
    const loop = mimoLoop(); loop.controller.group = declaredPidGroup();
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(renderThreePageStudioHtml({ loopDefinition: loop } as any,
      value => value.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;")));
    try {
      const selectors = dom.window.document.querySelectorAll("[data-studio-controller]");
      assert.strictEqual(selectors.length, 1);
      assert.match(selectors[0].selectedOptions[0].textContent, /Main.*Main\.Secondary/,
        "The group entry must identify every actual member rather than just its anchor");
    } finally { dom.window.close(); }
  });
});
