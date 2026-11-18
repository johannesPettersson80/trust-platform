import fs from "node:fs";
import path from "node:path";
import { expect, test } from "@playwright/test";
import { ensureParent, publicImagePath, repoRoot } from "../lib/paths.mjs";
import { dismissCodeServerChrome, runCommand } from "./helpers.mjs";

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

  // + Create project asks first what the project starts as (specification 25 §3.2): the page shows the
  // Structured Text start, whose row is the one that names src/Main.st. Then it asks for the parent
  // folder, here in code-server's own folder dialog, and the name.
  await welcome.getByRole("button", { name: "+ Create project" }).click();
  const dialog = page.locator(".quick-input-widget");
  const structuredText = dialog
    .locator(".monaco-list-row")
    .filter({ hasText: "A program in src/Main.st and the configuration that runs it." });
  await expect(structuredText).toBeVisible({ timeout: 30_000 });
  await structuredText.click();
  const input = dialog.locator("input");
  const accept = dialog.getByRole("button", { name: "Select Parent Folder" });
  const namePrompt = dialog.getByText("Enter a name for the new Structured Text project");
  await expect(accept).toBeVisible({ timeout: 30_000 });
  // The dialog opens by listing the home folder, then writes that folder's path over the box,
  // so the path is typed once the list shows. It may drop an accept that arrives while it lists
  // a folder, so the path is typed and accepted again, while the dialog still shows, until the
  // name's prompt shows (W46). The suggested name is the VS Code suite's to check
  // (new-project.test.ts).
  await expect(dialog.locator(".monaco-list-row").first()).toBeVisible({ timeout: 30_000 });
  const parent = `${CONTAINER_PARENTS}/${attempt}/`;
  await expect(async () => {
    if (await accept.isVisible()) {
      await input.fill(parent);
      await expect(input).toHaveValue(parent);
      await accept.click({ timeout: 5_000 });
    }
    await expect(namePrompt).toBeVisible({ timeout: 10_000 });
  }).toPass({ timeout: 90_000 });
  await input.fill(NAME);
  await page.keyboard.press("Enter");

  // `trust-runtime new` writes the project, and code-server opens it in this window at src/Main.st
  // (specification 25 §3.2). The truST view, which the Explorer replaced, shows the project. The
  // project lies in this attempt's folder: an accept the dialog took late never picked another.
  await page.waitForURL(new RegExp(`folder=.*create-project(?:/|%2F)${attempt}(?:/|%2F)${NAME}`), {
    timeout: 120_000,
  });
  await page.locator(".monaco-workbench").waitFor({ timeout: 120_000 });
  await dismissCodeServerChrome(page);
  await expect(page.locator(".tab.active")).toContainText("Main.st", { timeout: 120_000 });
  await runCommand(page, "truST: Focus on truST View");
  const project = await truSTView(page, DISPLAY_NAME);
  await expect(project.getByRole("button", { name: "Compile" })).toBeVisible({ timeout: 60_000 });
  await page.screenshot({ path: created, ...FINISHED_ANIMATIONS });
});
