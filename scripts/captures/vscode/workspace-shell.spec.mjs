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

  await page.screenshot({ path: output });
});
