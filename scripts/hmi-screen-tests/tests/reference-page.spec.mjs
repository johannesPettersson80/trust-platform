// The reference page (specification 39, §18 phase 3; §19.4): displays, controls and equipment in
// fixed states, drawn by the drawing bundle the runtime serves, in both themes, compared with the
// approved screenshots in tests/screens. `npm run update-screens` approves a new look.
import { expect, test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, "../../..");
const bundle = fs.readFileSync(path.join(repo, "crates/trust-runtime/src/web/ui/modules/hmi-drawing.js"), "utf8");
const font = `data:font/woff2;base64,${fs.readFileSync(path.join(repo, "crates/trust-runtime/src/web/ui/assets/fonts/InterVariable.woff2")).toString("base64")}`;
const reference = JSON.parse(fs.readFileSync(path.join(here, "reference-page.json"), "utf8"));

for (const theme of ["dark", "light"]) {
  test(`the reference page looks as approved in the ${theme} theme`, async ({ page }) => {
    await page.setViewportSize({ width: reference.size[0], height: reference.size[1] });
    await page.setContent("<!doctype html><html><head></head><body style=\"margin:0\"></body></html>");
    await page.addScriptTag({ content: bundle });
    await page.evaluate(({ reference, font, theme }) => {
      const drawing = window.TrustHmiDrawing;
      const style = document.createElement("style");
      style.textContent = drawing.tokensCss({}, font);
      document.head.append(style);
      const holder = document.createElement("div");
      holder.className = `hmi-theme-${theme}`;
      holder.innerHTML = drawing.renderToString(drawing.drawPage(reference.page, { size: reference.size, values: reference.values, history: reference.history, nowMs: reference.nowMs }));
      document.body.append(holder);
    }, { reference, font, theme });
    await page.evaluate(() => document.fonts.ready);
    await expect(page).toHaveScreenshot(`reference-${theme}.png`);
  });
}
