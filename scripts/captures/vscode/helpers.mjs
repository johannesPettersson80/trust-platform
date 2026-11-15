import { expect } from "@playwright/test";

async function dismissWelcomeOverlay(page) {
  const welcomeOverlay = page.locator(
    'div[role="dialog"][aria-label="Welcome to Visual Studio Code"]'
  );
  const deadline = Date.now() + 15_000;
  let sawOverlay = false;

  while (Date.now() < deadline) {
    const visible = await welcomeOverlay.isVisible().catch(() => false);
    if (!visible) {
      if (sawOverlay) {
        await expect(welcomeOverlay).toBeHidden({ timeout: 1_000 });
        return;
      }
      await page.waitForTimeout(500);
      continue;
    }

    sawOverlay = true;
    for (const label of [
      "Continue without Signing In",
      "Skip",
      "Not now",
      "Close"
    ]) {
      const button = welcomeOverlay.getByRole("button", {
        name: label,
        exact: true
      });
      if (await button.isVisible().catch(() => false)) {
        await button.click();
        await page.waitForTimeout(500);
      }
    }

    if (await welcomeOverlay.isVisible().catch(() => false)) {
      await page.keyboard.press("Escape").catch(() => {});
      await page.waitForTimeout(500);
    }
  }

  await expect(welcomeOverlay).toBeHidden({ timeout: 5_000 });
}

async function clickExplorerEntry(page, locator) {
  for (let attempt = 0; attempt < 4; attempt += 1) {
    await dismissCodeServerChrome(page);
    try {
      await locator.click({ timeout: 10_000 });
      return;
    } catch (error) {
      const message = String(error?.message || error);
      if (!message.includes("intercepts pointer events")) {
        throw error;
      }
      await page.waitForTimeout(500);
    }
  }

  await locator.click({ timeout: 10_000 });
}

export async function dismissCodeServerChrome(page) {
  await page.waitForTimeout(1_000);
  await dismissWelcomeOverlay(page);

  await page.waitForTimeout(250);

  const neverOpenRepo = page.getByText("Never", { exact: true });
  if (await neverOpenRepo.isVisible().catch(() => false)) {
    await neverOpenRepo.click();
    await page.waitForTimeout(500);
  }

  const chatPane = page.locator("#workbench\\.parts\\.auxiliarybar");
  if (await chatPane.isVisible().catch(() => false)) {
    await page.keyboard.press("Control+Alt+B").catch(() => {});
    await page.waitForTimeout(750);

    if (await chatPane.isVisible().catch(() => false)) {
      await page.keyboard.press("Control+Shift+P");
      const quickInput = page.locator(".quick-input-widget");
      await expect(quickInput).toBeVisible({ timeout: 10_000 });
      const input = quickInput.locator("input");
      await input.fill(">View: Toggle Secondary Side Bar Visibility");
      await expect(quickInput).toContainText(
        "View: Toggle Secondary Side Bar Visibility",
        { timeout: 10_000 }
      );
      await page.keyboard.press("Enter");
      await page.waitForTimeout(750);
    }

    await expect(chatPane).toBeHidden({ timeout: 10_000 });
  }
}

// The footer of the Devices & Connections graph, which counts what it draws once the language server
// and the runtime answered.
export const DEVICES_SUMMARY = /\d+ hosts? · \d+ runtimes? · \d+ endpoints?/;

/**
 * Runs a command from the command palette. Enter runs the palette's first row, which can be another
 * command whose title holds the words of this one: for ">Debug: Stop" it is "Debug: Start Debugging
 * and Stop on Entry", which pauses a running simulator. So the first row must be the command.
 */
export async function runCommand(page, title) {
  await page.keyboard.press("Control+Shift+P");
  const widget = page.locator(".quick-input-widget");
  await expect(widget).toBeVisible({ timeout: 30_000 });
  await widget.locator("input").fill(`>${title}`);
  await expect(widget.locator('.quick-input-list .monaco-list-row[data-index="0"]')).toContainText(title, {
    timeout: 30_000
  });
  await page.keyboard.press("Enter");
  await expect(widget).toBeHidden({ timeout: 10_000 });
}

/** Runs "truST: Open Devices & Connections" and returns its webview's frame once the graph is drawn. */
export async function openDevicesAndConnections(page, timeout = 120_000) {
  await runCommand(page, "truST: Open Devices & Connections");

  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    for (const frame of page.frames()) {
      if ((await frame.getByText(DEVICES_SUMMARY).count().catch(() => 0)) > 0) {
        return frame;
      }
    }
    await page.waitForTimeout(1_000);
  }
  throw new Error("Devices & Connections drew no graph");
}

export async function openSmokeMainFile(page) {
  await dismissCodeServerChrome(page);
  await expect(page.locator(".explorer-folders-view")).toContainText("src");

  const srcFolder = page.locator(".explorer-folders-view .label-name", {
    hasText: "src"
  });
  await clickExplorerEntry(page, srcFolder.first());
  await dismissCodeServerChrome(page);

  const mainFile = page.locator(".explorer-folders-view .label-name", {
    hasText: "Main.st"
  });
  await clickExplorerEntry(page, mainFile.first());
}

export async function waitForStructuredTextMode(page) {
  const structuredTextButton = page.getByRole("button", {
    name: "Structured Text",
    exact: true
  });

  const deadline = Date.now() + 90_000;
  while (Date.now() < deadline) {
    await dismissCodeServerChrome(page);
    if (await structuredTextButton.isVisible().catch(() => false)) {
      return;
    }
    await page.locator(".monaco-workbench").click().catch(() => {});
    await page.waitForTimeout(2_000);
  }

  await expect(structuredTextButton).toBeVisible({ timeout: 5_000 });
}

export function smokeMainEditorLines(page) {
  return page.locator("#workbench\\.parts\\.editor .editor-instance .view-lines").first();
}
