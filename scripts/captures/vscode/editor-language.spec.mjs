import fs from "node:fs";
import path from "node:path";
import { expect, test } from "@playwright/test";
import { ensureParent, publicImagePath, repoRoot } from "../lib/paths.mjs";
import { dismissCodeServerChrome, runCommand, waitForStructuredTextMode } from "./helpers.mjs";

// The editor pictures of "Program In VS Code" show the shipped capstone tutorial: its program calls the
// function block LevelController, which another file declares, so hover, rename and completion answer
// from across the project. Each attempt of each capture opens a copy of its own: the captures type and
// rename only in copies, so the shipped project stays as it is, and code-server, which restores the
// last window of a folder, has nothing to restore into a new one.
const PROJECT = path.join(repoRoot, "examples/filling_line");
const COPIES = path.join(repoRoot, "scripts/captures/.cache/editor-language");
// The copies as the code-server container sees them: start-code-server.sh mounts the repository there.
const CONTAINER_COPIES = "/workspaces/trust-platform/scripts/captures/.cache/editor-language";
// `LevelController` in `Ctrl : LevelController;` of src/Main.st.
const FUNCTION_BLOCK_TYPE = { line: 11, column: 12 };
// The empty line after the call `Ctrl(...)` of src/Main.st, where a capture types a statement.
const EMPTY_LINE = { line: 17, column: 1 };
// Where the mouse rests, beside the code, so that no hover or tooltip covers a picture by chance.
const MOUSE_REST = { x: 1300, y: 300 };
// The screenshots take CSS animations at their end: a hover fades in after it shows, and a picture taken
// at once shows it half drawn.
const FINISHED_ANIMATIONS = { animations: "disabled" };

test.beforeAll(() => {
  fs.rmSync(COPIES, { recursive: true, force: true });
});

async function quickInput(page, text) {
  await page.keyboard.press("Control+Shift+P");
  const widget = page.locator(".quick-input-widget");
  await expect(widget).toBeVisible({ timeout: 30_000 });
  await widget.locator("input").fill(text);
  return widget;
}

/** Moves the cursor and waits until the status bar shows it there, so that keys typed next reach that place. */
async function goTo(page, { line, column }) {
  const widget = await quickInput(page, `:${line}:${column}`);
  // The palette turns into Go to Line once it has read the colon; an Enter before that goes nowhere,
  // and the palette stays open (two captures of #520's first Docs Captures run).
  await expect(widget.locator(".quick-input-list")).toContainText(
    `go to line ${line} at column ${column}`,
    { timeout: 10_000 }
  );
  await page.keyboard.press("Enter");
  await expect(widget).toBeHidden({ timeout: 10_000 });
  await expect(page.getByRole("button", { name: /^Ln \d+, Col \d+$/ })).toHaveText(`Ln ${line}, Col ${column}`, {
    timeout: 10_000
  });
}

async function openFile(page, name) {
  const widget = await quickInput(page, name);
  await expect(widget.locator(".quick-input-list")).toContainText(name, { timeout: 30_000 });
  await page.keyboard.press("Enter");
  await expect(page.locator(".tab.active")).toContainText(name, { timeout: 30_000 });
}

/**
 * Opens a new copy of the project at src/LevelController.st once the language server has analysed the
 * project: it counts the reference to the function block from src/Main.st above its declaration.
 */
async function openProject(page, testInfo, name) {
  const copy = `${name}-${testInfo.retry}`;
  fs.cpSync(PROJECT, path.join(COPIES, copy, "filling_line"), { recursive: true });
  const response = await page.goto(
    `http://127.0.0.1:8080/?folder=${CONTAINER_COPIES}/${copy}/filling_line`,
    { waitUntil: "domcontentloaded" }
  );
  expect(response?.ok()).toBeTruthy();
  await page.locator(".monaco-workbench").waitFor({ timeout: 120_000 });
  await dismissCodeServerChrome(page);
  await openFile(page, "LevelController.st");
  await waitForStructuredTextMode(page);
  await expect(page.locator(".codelens-decoration").first()).toContainText("References: 1", {
    timeout: 90_000
  });
}

async function typeStatement(page, text) {
  await goTo(page, EMPTY_LINE);
  await page.keyboard.type(text, { delay: 50 });
}

test("capture Structured Text in the editor", async ({ page }, testInfo) => {
  const output = publicImagePath("vscode/vscode-st-syntax.png");
  ensureParent(output);

  await openProject(page, testInfo, "syntax");
  await goTo(page, { line: 1, column: 1 });
  await page.mouse.move(MOUSE_REST.x, MOUSE_REST.y);
  await page.screenshot({ path: output, ...FINISHED_ANIMATIONS });
});

test("capture the hover of a function block from another file", async ({ page }, testInfo) => {
  const output = publicImagePath("vscode/vscode-st-hover.png");
  ensureParent(output);

  await openProject(page, testInfo, "hover");
  await openFile(page, "Main.st");
  await goTo(page, FUNCTION_BLOCK_TYPE);
  await runCommand(page, "Show or Focus Hover");
  await expect(page.locator(".monaco-hover").filter({ hasText: "FUNCTION_BLOCK LevelController" })).toBeVisible({
    timeout: 30_000
  });
  await page.screenshot({ path: output, ...FINISHED_ANIMATIONS });
});

test("capture a rename across files in its preview", async ({ page }, testInfo) => {
  const output = publicImagePath("vscode/lsp-rename-across-files.png");
  ensureParent(output);

  await openProject(page, testInfo, "rename");
  await openFile(page, "Main.st");
  await goTo(page, FUNCTION_BLOCK_TYPE);
  await page.keyboard.press("F2");
  const input = page.locator(".rename-box input").first();
  await expect(input).toBeVisible({ timeout: 30_000 });
  await input.fill("TankLevelController");
  // Ctrl+Enter, as the rename box says, renames into the Refactor Preview, which lists every file that
  // the rename changes.
  await page.keyboard.press("Control+Enter");
  const panel = page.locator("#workbench\\.parts\\.panel");
  await expect(panel).toContainText("LevelController.st", { timeout: 60_000 });
  await expect(panel).toContainText("Main.st");
  await page.mouse.move(MOUSE_REST.x, MOUSE_REST.y);
  await page.screenshot({ path: output, ...FINISHED_ANIMATIONS });
});

test("capture completion while typing", async ({ page }, testInfo) => {
  const output = publicImagePath("vscode/vscode-st-completion.png");
  ensureParent(output);

  await openProject(page, testInfo, "completion");
  await openFile(page, "Main.st");
  await typeStatement(page, "PumpSpeedOut := REAL_TO_I");
  // Typing opens the suggestions; Ctrl+Space asks for them when it did not.
  const suggestions = page.locator(".suggest-widget");
  if (!(await suggestions.isVisible())) {
    await page.keyboard.press("Control+Space");
  }
  await expect(suggestions).toContainText("REAL_TO_INT", { timeout: 30_000 });
  // In an open list Ctrl+Space shows or hides the pane with the selected suggestion's description, and
  // code-server remembers the choice between windows. The picture shows the list alone: the pane opens
  // at its smallest size, which cuts the description off.
  const description = page.locator(".suggest-details");
  if (await description.isVisible()) {
    await page.keyboard.press("Control+Space");
  }
  await expect(description).toBeHidden({ timeout: 10_000 });
  await page.screenshot({ path: output, ...FINISHED_ANIMATIONS });
});

test("capture an IEC rule in the Problems panel", async ({ page }, testInfo) => {
  const output = publicImagePath("vscode/iec-diagnostics.png");
  ensureParent(output);

  await openProject(page, testInfo, "problems");
  await openFile(page, "Main.st");
  await typeStatement(page, "PumpSpeedOut := LevelPct;");
  await page.keyboard.press("Escape");
  await runCommand(page, "View: Focus Problems (Errors, Warnings, Infos)");
  await expect(page.locator(".markers-panel")).toContainText("cannot assign 'REAL' to 'INT'", {
    timeout: 60_000
  });
  await page.mouse.move(MOUSE_REST.x, MOUSE_REST.y);
  await page.screenshot({ path: output, ...FINISHED_ANIMATIONS });
});
