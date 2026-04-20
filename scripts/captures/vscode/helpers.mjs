import { expect } from "@playwright/test";

async function neutralizeWelcomeOverlay(page) {
  await page
    .addStyleTag({
      content: `
        .onboarding-a-overlay,
        [role="dialog"][aria-label="Welcome to Visual Studio Code"],
        .monaco-dialog-modal-block.dimmed {
          display: none !important;
          opacity: 0 !important;
          pointer-events: none !important;
        }
      `
    })
    .catch(() => {});

  await page.evaluate(() => {
    for (const selector of [
      ".onboarding-a-overlay",
      '[role="dialog"][aria-label="Welcome to Visual Studio Code"]',
      ".monaco-dialog-modal-block.dimmed"
    ]) {
      for (const element of document.querySelectorAll(selector)) {
        element.remove();
      }
    }
  });
}

export async function dismissCodeServerChrome(page) {
  await page.waitForTimeout(1_000);
  await neutralizeWelcomeOverlay(page);

  const welcomeOverlay = page.locator(
    'div[role="dialog"][aria-label="Welcome to Visual Studio Code"]'
  );
  for (let attempt = 0; attempt < 3; attempt += 1) {
    if (!(await welcomeOverlay.isVisible().catch(() => false))) {
      break;
    }

    for (const label of [
      "Continue without Signing In",
      "Skip",
      "Not now",
      "Close"
    ]) {
      const button = page.getByRole("button", { name: label, exact: true });
      if (await button.isVisible().catch(() => false)) {
        await button.click();
        await page.waitForTimeout(750);
      }
    }

    if (await welcomeOverlay.isVisible().catch(() => false)) {
      await page.keyboard.press("Escape").catch(() => {});
      await page.waitForTimeout(750);
    }
  }

  await neutralizeWelcomeOverlay(page);
  await page.waitForTimeout(250);

  const neverOpenRepo = page.getByText("Never", { exact: true });
  if (await neverOpenRepo.isVisible().catch(() => false)) {
    await neverOpenRepo.click();
    await page.waitForTimeout(500);
  }

  const chatPane = page.locator("#workbench\\.parts\\.auxiliarybar");
  if (await chatPane.isVisible().catch(() => false)) {
    await page.keyboard.press("Control+Alt+I");
    await expect(chatPane).toBeHidden({ timeout: 10_000 });
  }
}

export async function openSmokeMainFile(page) {
  await dismissCodeServerChrome(page);
  await expect(page.locator(".explorer-folders-view")).toContainText("src");

  const srcFolder = page.locator(".explorer-folders-view .label-name", {
    hasText: "src"
  });
  await srcFolder.first().click();
  await dismissCodeServerChrome(page);

  const mainFile = page.locator(".explorer-folders-view .label-name", {
    hasText: "Main.st"
  });
  await mainFile.first().click();
}

export function smokeMainEditorLines(page) {
  return page.locator("#workbench\\.parts\\.editor .editor-instance .view-lines").first();
}
