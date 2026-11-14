import { expect, test } from "@playwright/test";
import { ensureParent, publicImagePath } from "../lib/paths.mjs";
import { DEVICES_SUMMARY, dismissCodeServerChrome, openDevicesAndConnections } from "./helpers.mjs";

// A shipped project with a runtime and simulated I/O, as the page "Devices & Connections" describes:
// one host, one runtime, one endpoint.
const PROJECT = "/workspaces/trust-platform/examples/memory_marker_counter";

test("capture Devices & Connections of a project with a runtime", async ({ page }) => {
  const output = publicImagePath("vscode/vscode-devices-overview.png");
  ensureParent(output);

  const response = await page.goto(`http://127.0.0.1:8080/?folder=${PROJECT}`, {
    waitUntil: "domcontentloaded"
  });
  expect(response?.ok()).toBeTruthy();
  await page.locator(".monaco-workbench").waitFor({ timeout: 120_000 });
  await dismissCodeServerChrome(page);

  const graph = await openDevicesAndConnections(page);
  await expect(graph.getByText(DEVICES_SUMMARY)).toHaveText("1 host · 1 runtime · 1 endpoint");
  await expect(graph.getByText("Stopped").first()).toBeVisible();
  await expect(graph.getByText("Simulated").first()).toBeVisible();
  await dismissCodeServerChrome(page);
  await page.screenshot({ path: output });
});
