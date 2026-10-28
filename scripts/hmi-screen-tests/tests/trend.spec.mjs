// Trends (specification 39, §5.9 item 1), against the tank-level example in simulation: range
// chips; the wheel zooms the time axis and pauses the trend, with Back to live; the cursor line
// reads the pens; Export saves the visible window as CSV.
import { expect, test } from "@playwright/test";

const TREND = '[data-object="trend-1"]';
const LEVEL = '[data-object="value-1"]';

test("a trend zooms with the wheel, reads with the cursor, exports, and goes back to live", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto("/hmi/overview");
  await expect(page.locator(LEVEL)).toHaveAttribute("data-state", "confirmed", { timeout: 30_000 });
  const trend = page.locator(TREND);
  await expect(trend.locator('[data-part="trend-range"]')).toHaveCount(4);
  const plot = await trend.locator('[data-part="plot"]').boundingBox();
  await page.mouse.move(plot.x + plot.width * 0.75, plot.y + plot.height / 2);
  await page.mouse.wheel(0, -300);
  await expect(trend).toHaveAttribute("data-paused", "true");
  await page.mouse.move(plot.x + plot.width * 0.5, plot.y + plot.height / 2);
  await expect(trend.locator('[data-part="trend-cursor"]')).toHaveCount(1);
  await page.screenshot({ path: test.info().outputPath("trend-paused-cursor.png") });
  const download = page.waitForEvent("download");
  await trend.locator('[data-trend-action="export"]').click();
  const file = await download;
  expect(file.suggestedFilename()).toMatch(/^trend-1-\d{8}-\d{6}\.csv$/);
  await trend.locator('[data-trend-action="live"]').click();
  await expect(trend).not.toHaveAttribute("data-paused", "true");
  await trend.locator('[data-trend-action="expand"]').click();
  await expect(page.locator('[data-part="trend-expanded"] [data-object="trend-1"]')).toHaveCount(1);
  await page.screenshot({ path: test.info().outputPath("trend-expanded.png") });
  await page.click('[data-part="trend-expanded"] [data-action="close"]');
  await expect(page.locator('[data-part="trend-expanded"]')).toHaveCount(0);
  expect(errors).toEqual([]);
});
