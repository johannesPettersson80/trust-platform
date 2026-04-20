import { expect, test } from "@playwright/test";
import { ensureParent, publicImagePath } from "../lib/paths.mjs";
import {
  dismissCodeServerChrome,
  openSmokeMainFile,
  smokeMainEditorLines,
  waitForStructuredTextMode
} from "./helpers.mjs";

test("capture code-server workspace shell", async ({ page }) => {
  const output = publicImagePath("vscode/workspace-shell.png");
  ensureParent(output);

  const response = await page.goto("http://127.0.0.1:8080", {
    waitUntil: "domcontentloaded"
  });

  expect(response?.ok()).toBeTruthy();
  await page.locator(".monaco-workbench").waitFor({ timeout: 120_000 });
  await expect(page.locator(".monaco-workbench")).toContainText("trust-lsp-smoke (Workspace)");
  await dismissCodeServerChrome(page);
  await openSmokeMainFile(page);
  await waitForStructuredTextMode(page);
  await expect(page.locator(".tabs-container")).toContainText("Main.st");
  await expect(smokeMainEditorLines(page)).toContainText("PROGRAM Main");
  await expect(smokeMainEditorLines(page)).toContainText("result := AddOne");
  await page.locator(".monaco-workbench").click();
  await page.keyboard.press("Control+Shift+P");
  const quickInput = page.locator(".quick-input-widget");
  await expect(quickInput).toBeVisible({ timeout: 20_000 });
  await quickInput.locator("input").fill(">Structured Text: Open Runtime Panel");
  await expect(quickInput).toContainText("Structured Text: Open Runtime Panel", {
    timeout: 20_000,
  });
  await page.keyboard.press("Enter");
  await expect(page.getByRole("tab", { name: /Structured Text Runtime/i })).toBeVisible(
    { timeout: 60_000 }
  );
  await page.keyboard.press("Escape");
  await expect(quickInput).toBeHidden({ timeout: 20_000 });

  await page.screenshot({ path: output });
});
