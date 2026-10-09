// Faceplates placed on a page (specification 39, §8.9), against the equipment example on a real
// runtime: its Panels page (`cards.toml`) places the Plant library's run, PID and tank panels, each
// bound to its instance and showing that instance's values from the page's stream, in both themes.
import { expect, test } from "@playwright/test";

const BASE = "http://127.0.0.1:18481";

test("the Cards page shows each placed card bound to its instance, with its values", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.setViewportSize({ width: 1688, height: 1024 });
  await page.goto(`${BASE}/hmi/cards`);
  for (const id of ["p101", "t101", "lic101"]) {
    await expect(page.locator(`[data-placed="${id}"]`)).toBeVisible({ timeout: 30_000 });
  }
  // Each card's header names its instance by the placement's tag.
  await expect(page.locator('[data-placed="p101"] [data-part="placed-header"]')).toContainText("P-101");
  await expect(page.locator('[data-placed="lic101"] [data-part="placed-header"]')).toContainText("LIC-101");
  // The tank panel's level shows the draining tank's level in metres, with two decimals.
  await expect(page.locator('[data-placed="t101"] [data-object="level"]')).toContainText(/\d\.\d\d/, { timeout: 15_000 });
  // The pump panel's Running lamp follows the pump, which runs in Auto.
  await expect(page.locator('[data-placed="p101"] [data-object="state"]')).toHaveAttribute("data-on", "true", { timeout: 15_000 });
  await page.waitForTimeout(1500);
  await page.screenshot({ path: test.info().outputPath("cards-dark.png") });
  await page.locator('[data-action="theme"]').click();
  await page.waitForTimeout(500);
  await page.screenshot({ path: test.info().outputPath("cards-light.png") });
  expect(errors).toEqual([]);
});
