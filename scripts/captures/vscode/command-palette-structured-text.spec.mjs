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
  const notificationInput = page.locator(".quick-input-widget input");
  await notificationInput.fill(">Notifications: Clear All Notifications");
  await expect(page.locator(".quick-input-widget")).toContainText(
    "Notifications: Clear All Notifications"
  );
  await page.keyboard.press("Enter");
  await page.waitForTimeout(500);

  await page.keyboard.press("Control+Shift+P");

  const quickInput = page.locator(".quick-input-widget");
  await expect(quickInput).toBeVisible({ timeout: 30_000 });
  const input = quickInput.locator("input");
  await input.fill(">truST:");
  await expect(input).toHaveValue(">truST:");
  await expect(quickInput).toContainText("truST: Create Project", {
    timeout: 30_000
  });
  await expect(quickInput).toContainText("truST: Import PLCopen XML", {
    timeout: 30_000
  });
  await expect(quickInput).toContainText("truST: Move Structured Text Namespace", {
    timeout: 30_000
  });

  await page.screenshot({ path: output });
});
