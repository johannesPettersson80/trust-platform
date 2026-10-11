import { expect, test } from "@playwright/test";
import path from "node:path";

test("compiled WASM worker analyzes edits in the browser", async ({ page }, testInfo) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("http://127.0.0.1:14173/web/");
  await expect(page.locator("#statusPill")).toHaveAttribute("data-state", "running", {
    timeout: 30_000,
  });
  const valid = `PROGRAM Main
VAR
 Counter : INT;
END_VAR
Counter := Counter + 1;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
 TASK Cyclic(INTERVAL := T#10ms, PRIORITY := 1);
 PROGRAM MainInstance WITH Cyclic : Main;
END_RESOURCE
END_CONFIGURATION
`;
  await page.locator("#source").fill(valid);
  await expect(page.locator("#diagList")).toContainText("No diagnostics in current source.");
  await page.locator("#source").fill(valid.replace("Counter + 1", "MissingValue + 1"));
  await expect(page.locator("#diagList .diag-item")).not.toHaveCount(0);
  await expect(page.locator("#diagList")).toContainText("MissingValue");
  await page.screenshot({ path: testInfo.outputPath("wasm-diagnostics.png") });
  await page.locator("#source").fill(valid);
  await expect(page.locator("#diagList")).toContainText("No diagnostics in current source.");
  await expect(page.locator("#connectionBadge")).toHaveAttribute("data-state", "online");
  await page.screenshot({ path: testInfo.outputPath("wasm-clean.png") });
  expect(errors).toEqual([]);
  await testInfo.attach("compiled-wasm", {
    path: path.join(testInfo.outputDir, "wasm-clean.png"),
    contentType: "image/png",
  });
});
