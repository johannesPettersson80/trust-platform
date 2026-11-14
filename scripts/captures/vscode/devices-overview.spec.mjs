import { expect, test } from "@playwright/test";
import { ensureParent, publicImagePath } from "../lib/paths.mjs";
import { dismissCodeServerChrome } from "./helpers.mjs";

// A shipped project with a runtime and simulated I/O, as the page "Devices & Connections" describes:
// one host, one runtime, one endpoint.
const PROJECT = "/workspaces/trust-platform/examples/memory_marker_counter";
// The graph's footer, which counts what it draws once the language server and the runtime answered.
const SUMMARY = /\d+ hosts? · \d+ runtimes? · \d+ endpoints?/;

/** The frame of the Devices & Connections webview once its graph is drawn. */
async function graphFrame(page, timeout) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    for (const frame of page.frames()) {
      if ((await frame.getByText(SUMMARY).count().catch(() => 0)) > 0) {
        return frame;
      }
    }
    await page.waitForTimeout(1_000);
  }
  throw new Error("Devices & Connections drew no graph");
}

test("capture Devices & Connections of a project with a runtime", async ({ page }) => {
  const output = publicImagePath("vscode/vscode-devices-overview.png");
  ensureParent(output);

  const response = await page.goto(`http://127.0.0.1:8080/?folder=${PROJECT}`, {
    waitUntil: "domcontentloaded"
  });
  expect(response?.ok()).toBeTruthy();
  await page.locator(".monaco-workbench").waitFor({ timeout: 120_000 });
  await dismissCodeServerChrome(page);

  await page.keyboard.press("Control+Shift+P");
  const quickInput = page.locator(".quick-input-widget");
  await expect(quickInput).toBeVisible({ timeout: 30_000 });
  await quickInput.locator("input").fill(">truST: Open Devices & Connections");
  await expect(quickInput).toContainText("Open Devices & Connections", { timeout: 30_000 });
  await page.keyboard.press("Enter");

  const graph = await graphFrame(page, 120_000);
  await expect(graph.getByText(SUMMARY)).toHaveText("1 host · 1 runtime · 1 endpoint");
  await expect(graph.getByText("Stopped").first()).toBeVisible();
  await expect(graph.getByText("Simulated").first()).toBeVisible();
  await dismissCodeServerChrome(page);
  await page.screenshot({ path: output });
});
