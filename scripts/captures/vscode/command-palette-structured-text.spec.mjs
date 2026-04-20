import { expect, test } from "@playwright/test";
import { ensureParent, publicImagePath } from "../lib/paths.mjs";
import {
  dismissCodeServerChrome,
  openSmokeMainFile,
  smokeMainEditorLines,
  waitForStructuredTextMode
} from "./helpers.mjs";

test("capture code-server structured text command palette", async ({ page }) => {
  const output = publicImagePath("install/command-palette-structured-text.png");
  ensureParent(output);

  const response = await page.goto("http://127.0.0.1:8080", {
    waitUntil: "domcontentloaded"
  });

  expect(response?.ok()).toBeTruthy();
  await page.locator(".monaco-workbench").waitFor({ timeout: 120_000 });
  await dismissCodeServerChrome(page);
  await openSmokeMainFile(page);
  await waitForStructuredTextMode(page);
  await expect(smokeMainEditorLines(page)).toContainText("PROGRAM Main");
  await page.locator(".monaco-workbench").click();
  await page.keyboard.press("Control+Shift+P");

  const quickInput = page.locator(".quick-input-widget");
  await expect(quickInput).toBeVisible({ timeout: 30_000 });
  const input = quickInput.locator("input");
  await input.fill(">Structured Text:");
  await expect(input).toHaveValue(">Structured Text:");
  await expect(quickInput).toContainText("Structured Text: Attach Debugger", {
    timeout: 30_000
  });
  await expect(quickInput).toContainText("Structured Text: New Project", {
    timeout: 30_000
  });
  await expect(quickInput).toContainText("Structured Text: Open HMI Preview", {
    timeout: 30_000
  });

  await page.screenshot({ path: output });
});
