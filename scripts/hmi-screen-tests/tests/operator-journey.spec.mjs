// The operator journey of the tank-level example (specification 39, §18 phase 1D and phase 3),
// against the simulator: the live level shows, an Operator logs in and changes the setpoint and sees
// it confirmed by readback, the high-level alarm is raised and acknowledged.
import { expect, test } from "@playwright/test";

const SETPOINT = '[data-object="setpoint-1"]';
const LEVEL = '[data-object="value-1"]';

async function setpointTo(page, value) {
  await page.click(SETPOINT);
  await page.fill('[data-part="entry"] input[name="value"]', String(value));
  await page.click('[data-part="entry"] button[type="submit"]');
  await expect(page.locator(SETPOINT)).toHaveAttribute("data-outcome", "confirmed", { timeout: 15_000 });
}

test("an operator changes the setpoint, sees it confirmed, and acknowledges the high-level alarm", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto("/hmi/overview");
  await expect(page.locator(LEVEL)).toHaveAttribute("data-state", "confirmed", { timeout: 30_000 });
  // A control asks for the login first (§12.4).
  await page.click(SETPOINT);
  await page.fill('[data-part="login"] input[name="user"]', "operator");
  await page.fill('[data-part="login"] input[name="password"]', "operator");
  await page.click('[data-part="login"] button[type="submit"]');
  await expect(page.locator('[data-part="session"]')).toContainText("operator", { timeout: 15_000 });
  await setpointTo(page, 2.0);
  await expect(page.locator(SETPOINT)).toContainText("2");
  // Above the high limit, the level raises the high alarm.
  await setpointTo(page, 2.6);
  const row = page.locator('[data-part="alarm-row"][data-alarm$=".High"]');
  await expect(row).toBeVisible({ timeout: 120_000 });
  await row.locator('[data-alarm-action="acknowledge"]').click();
  await expect(row.locator('[data-alarm-action="acknowledge"]')).toHaveCount(0, { timeout: 15_000 });
  expect(errors).toEqual([]);
});
