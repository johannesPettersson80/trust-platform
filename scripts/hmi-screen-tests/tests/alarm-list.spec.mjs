// The alarm list (specification 39, §5.9 item 2), against the equipment example in simulation,
// whose tank drains from 1.5 m with its valve closed below the low (Medium) and low-low (High)
// limits of its level transmitter LT-101, Plant limit alarms (Plant §7.1 item 5): columns under
// their headings, the low-level alarms' value against their limits, a heading sorts, a quick
// filter filters and says how many rows it shows, and a High alarm's sound offers Silence in the
// top bar until it is pressed.
import { expect, test } from "@playwright/test";

const EQUIPMENT = `http://127.0.0.1:${Number(process.env.HMI_SCREEN_EQUIPMENT_PORT || 18481)}`;
const LIST = '[data-object="alarm-list"]';
const SILENCE = '[data-part="top-bar"] [data-action="silence"]';

test("the alarm list has columns, sorts by a heading, filters, and offers Silence while a High alarm sounds", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto(`${EQUIPMENT}/hmi/alarms`);
  const rows = page.locator(`${LIST} [data-part="alarm-row"]`);
  await expect(rows.first()).toBeVisible({ timeout: 30_000 });
  await expect(page.locator(`${LIST} [data-part="alarm-heading"]`)).toHaveText(["Priority", "Time", "Tag", "Message", "Value", "State"]);
  await expect(page.locator(`${LIST} [data-part="alarm-cell"][data-column="value"]`).first()).toHaveText(/^\d+(\.\d+)? < \d+(\.\d+)? m$/);
  await expect(page.locator(SILENCE)).toBeVisible({ timeout: 15_000 });
  await page.screenshot({ path: test.info().outputPath("alarm-list.png") });
  await page.click(`${LIST} [data-alarm-sort="time"]`);
  await expect(page.locator(`${LIST} [data-alarm-sort="time"]`)).toHaveAttribute("data-sorted", "natural");
  const total = await rows.count();
  await page.click(`${LIST} [data-alarm-filter="priority"][data-value="high"]`);
  await expect(page.locator(`${LIST} [data-alarm-filter="priority"][data-value="high"]`)).toHaveAttribute("data-selected", "true");
  await expect(page.locator(LIST)).toContainText(`of ${total}`);
  await page.screenshot({ path: test.info().outputPath("alarm-list-filtered.png") });
  await page.click(SILENCE);
  await expect(page.locator(SILENCE)).toBeHidden();
  expect(errors).toEqual([]);
});
