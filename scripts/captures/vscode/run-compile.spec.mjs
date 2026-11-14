import fs from "node:fs";
import path from "node:path";
import { expect, test } from "@playwright/test";
import { ensureParent, publicImagePath, repoRoot } from "../lib/paths.mjs";
import { dismissCodeServerChrome } from "./helpers.mjs";

// "Run & Compile" shows Compile and Start of the truST view on the shipped project that the Devices &
// Connections pictures show. Each attempt opens a copy of its own, so that the build writes into the
// copy, and code-server, which restores the last window of a folder, has nothing to restore into it.
// Compile and Start need the toolchain that start-code-server.sh hands to the container.
const PROJECT = path.join(repoRoot, "examples/memory_marker_counter");
const COPIES = path.join(repoRoot, "scripts/captures/.cache/run-compile");
// The copies as the code-server container sees them: start-code-server.sh mounts the repository there.
const CONTAINER_COPIES = "/workspaces/trust-platform/scripts/captures/.cache/run-compile";
// The screenshots take CSS animations at their end, so that no view is caught while it changes.
const FINISHED_ANIMATIONS = { animations: "disabled" };

async function runCommand(page, title) {
  await page.keyboard.press("Control+Shift+P");
  const widget = page.locator(".quick-input-widget");
  await expect(widget).toBeVisible({ timeout: 30_000 });
  await widget.locator("input").fill(`>${title}`);
  await expect(widget.locator(".quick-input-list")).toContainText(title, { timeout: 30_000 });
  await page.keyboard.press("Enter");
  await expect(widget).toBeHidden({ timeout: 10_000 });
}

/** The frame of the webview that shows `text`: a webview's, not the workbench's own. */
async function webview(page, text, timeout = 120_000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    for (const frame of page.frames().filter((frame) => frame !== page.mainFrame())) {
      if ((await frame.getByText(text, { exact: true }).count().catch(() => 0)) > 0) {
        return frame;
      }
    }
    await page.waitForTimeout(1_000);
  }
  throw new Error(`no webview showed "${text}"`);
}

test("capture Compile and Start in the truST view", async ({ page }, testInfo) => {
  const compiled = publicImagePath("vscode/vscode-run-check-passed.png");
  const running = publicImagePath("vscode/vscode-run-simulator-running.png");
  ensureParent(compiled);

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
  await runCommand(page, "truST: Focus on truST View");
  const statusBar = page.locator(".part.statusbar");

  // Compile builds the program; when it passes, the status bar says so for 8 seconds and the button
  // shows a check.
  const view = await webview(page, "Memory Marker Counter");
  await view.getByRole("button", { name: "Compile" }).click();
  await expect(statusBar).toContainText("Compile passed", { timeout: 120_000 });
  await expect(view.locator("#compileIcon.codicon-check")).toBeVisible({ timeout: 7_000 });
  await page.screenshot({ path: compiled, ...FINISHED_ANIMATIONS });

  // Start runs it on the simulator in a debug session, which opens Live Values on the program's values.
  await view.getByRole("button", { name: "Start" }).click();
  await expect(statusBar).toContainText("truST: Simulator running", { timeout: 120_000 });
  const liveValues = await webview(page, "P1.Counter");
  await expect(liveValues.getByText("Running", { exact: true })).toBeVisible({ timeout: 60_000 });
  await expect(liveValues.getByText(/^scan #\d+$/)).toBeVisible({ timeout: 60_000 });
  await page.screenshot({ path: running, ...FINISHED_ANIMATIONS });

  // The debug toolbar's stop ends the simulation.
  await page.locator(".debug-toolbar").getByRole("button", { name: /^Stop/ }).click();
  await expect(statusBar).toContainText("truST: Simulator stopped", { timeout: 60_000 });
});
