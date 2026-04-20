import { expect, test } from "@playwright/test";
import { ensureParent, publicImagePath } from "../lib/paths.mjs";

test("capture browser hmi home", async ({ page }) => {
  const output = publicImagePath("browser/hmi-home.png");
  ensureParent(output);

  let response = await page.goto("http://127.0.0.1:18082/hmi", {
    waitUntil: "commit"
  });

  expect(response?.ok()).toBeTruthy();
  await expect.poll(
    async () => {
      const exportResponse = await page.request.get("http://127.0.0.1:18082/hmi/export.json");
      if (!exportResponse.ok()) {
        return "";
      }
      const payload = await exportResponse.json();
      const pages =
        payload?.config?.schema?.pages ??
        payload?.config?.descriptor?.pages ??
        payload?.pages ??
        [];
      if (!Array.isArray(pages)) {
        return "";
      }
      return pages.map((page) => page?.title || page?.label || page?.id || "").join(" ");
    },
    { timeout: 60_000, intervals: [1_000, 2_000, 5_000] }
  ).toContain("Overview");

  response = await page.goto("http://127.0.0.1:18082/hmi", {
    waitUntil: "domcontentloaded"
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

  const themeLabel = page.locator("#themeLabel");
  if ((await page.locator("body").getAttribute("data-theme")) !== "dark") {
    await themeLabel.click();
  }
  await expect(page.locator("body")).toHaveAttribute("data-theme", "dark");
  await expect(themeLabel).toHaveText(/dark mode/i);
  await expect(page.locator("#connectionState")).toHaveText(/connected/i, {
    timeout: 30_000
  });
  await expect(page.locator("#freshnessState")).toHaveText(/freshness:\s*\d+\s*ms/i, {
    timeout: 30_000
  });
  await expect(page.locator("#pageContent")).not.toContainText("--");

  await page.screenshot({ path: output });
});
