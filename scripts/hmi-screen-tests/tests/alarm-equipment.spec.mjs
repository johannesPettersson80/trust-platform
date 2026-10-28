// Alarm details name their equipment (specification 39, §5.9, §13.6 item 9), against the equipment
// example: the tank owns no alarms, its level transmitter LT-101 does (Plant §7.5), and its low
// alarms rise while the tank drains. Show equipment opens the overview focused on LT-101 and
// acknowledges nothing.
import { expect, test } from "@playwright/test";

const EQUIPMENT = `http://127.0.0.1:${Number(process.env.HMI_SCREEN_EQUIPMENT_PORT || 18481)}`;

for (const theme of ["dark", "light"]) test(`alarm details show equipment without acknowledging in ${theme}`, async ({ page }) => {
  const writes = [];
  page.on("request", (request) => {
    if (request.method() === "POST" && /\/api\/hmi\/(alarm|command)$/.test(new URL(request.url()).pathname)) writes.push(request.url());
  });
  await page.goto(`${EQUIPMENT}/hmi/alarms`);
  const row = page.locator('[data-part="alarm-row"][data-alarm^="Main.LT101."]').first();
  await expect(row).toBeVisible({ timeout: 30000 });
  if (theme === "light") await page.click('[data-part="top-bar"] [data-action="theme"]');
  await row.click();
  const dialog = page.locator('[data-part="alarm"]');
  await expect(dialog).toBeVisible();
  const show = dialog.locator('[data-action="show-equipment"]');
  await expect(show).toBeEnabled();
  const box = await show.boundingBox();
  expect(box.height, "Show equipment is at least 44 px high").toBeGreaterThanOrEqual(44);
  await page.screenshot({ path: test.info().outputPath("alarm-details.png") });
  await show.click();
  await expect(page).toHaveURL(/\/hmi\/overview\?focus=lt-101$/);
  await expect(page.locator('[data-object="lt-101"]')).toHaveAttribute("data-focused", "true");
  await expect(page.locator('[data-object="lt-101"]')).toHaveAttribute("data-state", "confirmed", { timeout: 4000 });
  await page.screenshot({ path: test.info().outputPath("equipment-focused.png") });
  expect(writes).toEqual([]);
  await expect(page.locator('[data-object="lt-101"]')).not.toHaveAttribute("data-focused", "true", { timeout: 10000 });
});
