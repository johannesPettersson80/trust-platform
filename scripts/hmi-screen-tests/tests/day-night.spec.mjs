// The day and night switch (specification 39, §18 phase 5 item 6), against the tank-level example
// in simulation: the top bar switches the dark project to day, and the panel keeps it on reload.
import { expect, test } from "@playwright/test";

const LEVEL = '[data-object="value-1"]';

test("the top bar switches to day and the panel keeps it", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto("/hmi/overview");
  await expect(page.locator(LEVEL)).toHaveAttribute("data-state", "confirmed", { timeout: 30_000 });
  const display = page.locator("#hmi");
  await expect(display).toHaveClass(/hmi-theme-dark/);
  await page.click('[data-part="top-bar"] [data-action="theme"]');
  await expect(display).toHaveClass(/hmi-theme-light/);
  await page.screenshot({ path: test.info().outputPath("day.png") });
  await page.reload();
  await expect(page.locator(LEVEL)).toHaveAttribute("data-state", "confirmed", { timeout: 30_000 });
  await expect(page.locator("#hmi")).toHaveClass(/hmi-theme-light/);
  await page.click('[data-part="top-bar"] [data-action="theme"]');
  await expect(page.locator("#hmi")).toHaveClass(/hmi-theme-dark/);
  expect(errors).toEqual([]);
});
