// Specification 39 §5.9: real runtime values, parsed per-pen ranges and a changing visible axis.
import { expect, test } from "@playwright/test";

test("a live trend keeps separate pen ranges and follows the first visible pen", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto("/hmi/trend-scales");
  const chart = page.locator('[data-object="scales"]');
  await expect(chart.locator('polyline[data-pen="Level"]')).toHaveCount(1, { timeout: 30000 });
  await expect(chart.locator('[data-part="legend"][data-pen="Level"]')).toContainText(/Level\s*\d/);
  await expect(chart.locator('text[data-axis-pen="Level"]')).toHaveText(["0", "1.5", "3"]);
  await expect(chart.locator('text[data-axis-pen="Level"]').first()).toHaveAttribute("fill", "var(--hmi-pen-1)");
  await page.screenshot({ path: test.info().outputPath("scales-dark.png") });
  await chart.locator('[data-part="legend"][data-pen="Level"]').click();
  await expect(chart.locator('polyline[data-pen="Level"]')).toHaveCount(0);
  await expect(chart.locator('text[data-axis-pen="Setpoint"]')).toHaveText(["0", "50", "100"]);
  await expect(chart.locator('text[data-axis-pen="Setpoint"]').first()).toHaveAttribute("fill", "var(--hmi-pen-2)");
  await page.click('[data-part="top-bar"] [data-action="theme"]');
  await expect(page.locator("#hmi")).toHaveClass(/hmi-theme-light/);
  await page.screenshot({ path: test.info().outputPath("scales-light.png") });
  expect(errors).toEqual([]);
});
