import fs from "node:fs";
import path from "node:path";
import { expect, test } from "@playwright/test";
import { ensureParent, publicImagePath, repoRoot } from "../lib/paths.mjs";
import { dismissCodeServerChrome } from "./helpers.mjs";

// "Create A New Project" shows the truST view of a window with no folder, then the project that
// "+ Create project" makes. Each attempt creates its project in an empty folder of its own, so that
// code-server, which restores the last window of a folder, has nothing to restore into it.
const PARENTS = path.join(repoRoot, "scripts/captures/.cache/create-project");
// The parent folders as the code-server container sees them: start-code-server.sh mounts the repository there.
const CONTAINER_PARENTS = "/workspaces/trust-platform/scripts/captures/.cache/create-project";
const NAME = "my-first-plc";
// The truST view names a project after its folder, in words.
const DISPLAY_NAME = "My First Plc";
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

/** The frame of the truST view once it shows `text`: a webview's, not the workbench's own. */
async function truSTView(page, text, timeout = 120_000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    for (const frame of page.frames().filter((frame) => frame !== page.mainFrame())) {
      if ((await frame.getByText(text, { exact: true }).count().catch(() => 0)) > 0) {
        return frame;
      }
    }
    await page.waitForTimeout(1_000);
  }
  throw new Error(`the truST view never showed "${text}"`);
}

test("capture creating a project from the truST view", async ({ page }, testInfo) => {
  const noProject = publicImagePath("vscode/vscode-create-no-project.png");
  const created = publicImagePath("vscode/vscode-create-project-shell.png");
  ensureParent(noProject);

  const attempt = String(testInfo.retry);
  fs.rmSync(path.join(PARENTS, attempt), { recursive: true, force: true });
  fs.mkdirSync(path.join(PARENTS, attempt), { recursive: true });

  // A window without a folder: `ew` opens an empty one, not the last folder.
  const response = await page.goto("http://127.0.0.1:8080/?ew=true", { waitUntil: "domcontentloaded" });
  expect(response?.ok()).toBeTruthy();
  await page.locator(".monaco-workbench").waitFor({ timeout: 120_000 });
  await dismissCodeServerChrome(page);
  await runCommand(page, "truST: Focus on truST View");
  const welcome = await truSTView(page, "No project");
  await expect(welcome.getByRole("button", { name: "+ Create project" })).toBeVisible();
  await page.screenshot({ path: noProject, ...FINISHED_ANIMATIONS });

  // + Create project asks for the parent folder, here in code-server's own folder dialog, and the name.
  await welcome.getByRole("button", { name: "+ Create project" }).click();
  const dialog = page.locator(".quick-input-widget");
  await expect(dialog).toBeVisible({ timeout: 30_000 });
  await dialog.locator("input").fill(`${CONTAINER_PARENTS}/${attempt}/`);
  await expect(dialog.getByRole("button", { name: "Select Parent Folder" })).toBeVisible({ timeout: 30_000 });
  await dialog.getByRole("button", { name: "Select Parent Folder" }).click();
  await expect(dialog.locator("input")).toHaveValue("my-st-project", { timeout: 30_000 });
  await dialog.locator("input").fill(NAME);
  await page.keyboard.press("Enter");

  // `trust-runtime new` writes the project, and code-server opens it in this window at src/Main.st
  // (specification 25 §3.2). The truST view, which the Explorer replaced, shows the project.
  await page.waitForURL(new RegExp(`folder=.*${NAME}`), { timeout: 120_000 });
  await page.locator(".monaco-workbench").waitFor({ timeout: 120_000 });
  await dismissCodeServerChrome(page);
  await expect(page.locator(".tab.active")).toContainText("Main.st", { timeout: 120_000 });
  await runCommand(page, "truST: Focus on truST View");
  const project = await truSTView(page, DISPLAY_NAME);
  await expect(project.getByRole("button", { name: "Compile" })).toBeVisible({ timeout: 60_000 });
  await page.screenshot({ path: created, ...FINISHED_ANIMATIONS });
});
