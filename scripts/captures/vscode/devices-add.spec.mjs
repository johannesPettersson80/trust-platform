import fs from "node:fs";
import path from "node:path";
import { expect, test } from "@playwright/test";
import { ensureParent, publicImagePath, repoRoot } from "../lib/paths.mjs";
import { DEVICES_SUMMARY, dismissCodeServerChrome, openDevicesAndConnections } from "./helpers.mjs";

// "Add A Device" shows the add flow of Devices & Connections on the shipped project that the page
// "Devices & Connections" shows: one host with its runtime and simulated I/O. Each attempt opens a
// copy of its own, so that the shipped project stays as it is even if a Save ran, and code-server,
// which restores the last window of a folder, has nothing to restore into it.
const PROJECT = path.join(repoRoot, "examples/memory_marker_counter");
const COPIES = path.join(repoRoot, "scripts/captures/.cache/devices-add");
// The copies as the code-server container sees them: start-code-server.sh mounts the repository there.
const CONTAINER_COPIES = "/workspaces/trust-platform/scripts/captures/.cache/devices-add";
// The screenshots take CSS animations at their end, so that a drawer is never caught while it opens.
const FINISHED_ANIMATIONS = { animations: "disabled" };

test("capture the add flow of Devices & Connections", async ({ page }, testInfo) => {
  const picker = publicImagePath("vscode/vscode-add-device-picker.png");
  const form = publicImagePath("vscode/vscode-add-device-modbus-form.png");
  ensureParent(picker);

  const attempt = String(testInfo.retry);
  fs.rmSync(path.join(COPIES, attempt), { recursive: true, force: true });
  fs.cpSync(PROJECT, path.join(COPIES, attempt, "memory_marker_counter"), { recursive: true });
  const response = await page.goto(
    `http://127.0.0.1:8080/?folder=${CONTAINER_COPIES}/${attempt}/memory_marker_counter`,
    { waitUntil: "domcontentloaded" }
  );
  expect(response?.ok()).toBeTruthy();
  await page.locator(".monaco-workbench").waitFor({ timeout: 120_000 });
  await dismissCodeServerChrome(page);

  const graph = await openDevicesAndConnections(page);
  await expect(graph.getByText(DEVICES_SUMMARY)).toHaveText("1 host · 1 runtime · 1 endpoint");

  // + Add opens the choices of what the runtime can gain, grouped by what the user wants to do.
  await graph.locator("[data-canvas-add]").click();
  const choices = graph.getByRole("complementary", { name: "Add device, connection, or simulated plant" });
  await expect(choices.getByRole("group", { name: "Devices and I/O" })).toBeVisible({ timeout: 30_000 });
  await page.screenshot({ path: picker, ...FINISHED_ANIMATIONS });

  // Modbus opens its form, which the runtime's schema describes.
  await choices.getByRole("button").filter({ hasText: "Modbus" }).first().click();
  const add = graph.getByRole("complementary", { name: "Add device" });
  await expect(add).toContainText("Add Modbus TCP", { timeout: 30_000 });
  await expect(add.getByText("Device address")).toBeVisible();
  await page.screenshot({ path: form, ...FINISHED_ANIMATIONS });
});
