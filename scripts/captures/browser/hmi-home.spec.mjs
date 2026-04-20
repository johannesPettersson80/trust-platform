import { expect, test } from "@playwright/test";
import { ensureParent, publicImagePath } from "../lib/paths.mjs";

test("capture browser hmi home", async ({ page }) => {
  const output = publicImagePath("browser/hmi-home.png");
  ensureParent(output);

  const response = await page.goto("http://127.0.0.1:18082/hmi", {
    waitUntil: "commit"
  });

  expect(response?.ok()).toBeTruthy();
  await expect(page.locator("#pageSidebar")).toContainText("Overview", {
    timeout: 30_000
  });
  await expect(page.locator("#pageContent")).toContainText("TANK-001", {
    timeout: 30_000
  });
  await expect(page.locator("#pageContent")).toContainText("PUMP-001", {
    timeout: 30_000
  });
  await expect(page.locator("#pageContent")).toContainText("RUNNING", {
    timeout: 30_000
  });
  await expect(page.locator("#connectionState")).toHaveText(/connected/i, {
    timeout: 30_000
  });
  await expect(page.locator("#freshnessState")).toHaveText(/freshness:\s*\d+\s*ms/i, {
    timeout: 30_000
  });
  await expect(page.locator("#pageContent")).not.toContainText("--");

  await page.screenshot({ path: output });
});
