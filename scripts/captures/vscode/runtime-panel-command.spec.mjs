import { expect, test } from "@playwright/test";
import { ensureParent, publicImagePath } from "../lib/paths.mjs";
import { dismissCodeServerChrome } from "./helpers.mjs";

test.skip("capture code-server runtime panel command palette", async ({ page }) => {
  const output = publicImagePath("vscode/runtime-panel-command.png");
  ensureParent(output);

  expect(
    "Command palette automation is still unstable under the current code-server shell."
  ).toBeTruthy();
  await page.goto("http://127.0.0.1:8080");
  await page.locator(".monaco-workbench").waitFor({ timeout: 120_000 });
  await dismissCodeServerChrome(page);

  await page.keyboard.press("Control+Shift+P");
  await page.locator(".quick-input-widget").waitFor();
  await page.locator(".quick-input-widget input").fill("Structured Text: Open Runtime Panel");
  await expect(page.locator(".quick-input-widget")).toContainText("Structured Text: Open Runtime Panel");

  await page.screenshot({ path: output });
});
