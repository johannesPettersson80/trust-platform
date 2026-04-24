import { expect, test } from "@playwright/test";
import { ensureParent, publicImagePath } from "../lib/paths.mjs";

const BASE = "http://127.0.0.1:18082/hmi";

async function waitForOverviewLive(page) {
  await expect(page.locator("#connectionState")).toHaveText(/connected/i, {
    timeout: 30_000,
  });
  await expect(page.locator("#freshnessState")).toHaveText(
    /freshness:\s*\d+\s*ms/i,
    { timeout: 30_000 },
  );
  await expect(page.locator("#pageContent")).not.toContainText("--", {
    timeout: 30_000,
  });
}

async function ensureDark(page) {
  if ((await page.locator("body").getAttribute("data-theme")) !== "dark") {
    await page.locator("#themeLabel").click();
  }
  await expect(page.locator("body")).toHaveAttribute("data-theme", "dark");
}

async function navSidebar(page, label) {
  await page
    .locator("#pageSidebar")
    .getByText(label, { exact: false })
    .first()
    .click();
  // Let the page content swap before the screenshot.
  await page.waitForTimeout(1500);
}

test("capture browser hmi operator pages", async ({ page }) => {
  const overviewGuide = publicImagePath(
    "browser/hmi-operator-guide-overview.png",
  );
  const overviewDaily = publicImagePath(
    "browser/hmi-operator-daily-checks.png",
  );
  const alarms = publicImagePath("browser/hmi-operator-alarm-page.png");
  const trends = publicImagePath("browser/hmi-operator-shift-handover.png");
  [overviewGuide, overviewDaily, alarms, trends].forEach(ensureParent);

  const resp = await page.goto(BASE, { waitUntil: "domcontentloaded" });
  expect(resp?.ok()).toBeTruthy();

  // Sidebar populated, overview live, dark mode.
  await expect(page.locator("#pageSidebar")).toContainText("Overview", {
    timeout: 30_000,
  });
  await waitForOverviewLive(page);
  await ensureDark(page);

  // Overview -> same screenshot used for both operator-guide and daily-checks.
  await page.screenshot({ path: overviewGuide });
  await page.screenshot({ path: overviewDaily });

  // Alarms page.
  await navSidebar(page, "Alarms");
  await expect(page.locator("#pageContent")).toContainText(/Alarms/i);
  await page.screenshot({ path: alarms });

  // Trends page (used during shift handover review).
  await navSidebar(page, "Trends");
  await expect(page.locator("#pageContent")).toContainText(/Trends/i);
  await page.screenshot({ path: trends });
});
