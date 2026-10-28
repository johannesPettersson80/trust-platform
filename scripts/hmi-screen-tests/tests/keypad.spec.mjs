// The on-screen keypad (specification 39, §18 phase 5 item 6), against the tank-level example in
// simulation on a touch screen: a tap on the setpoint opens a large keypad with the allowed range,
// which types a value that is sent and confirmed.
import { expect, test } from "@playwright/test";

test.use({ hasTouch: true });

const SETPOINT = '[data-object="setpoint-1"]';
const LEVEL = '[data-object="value-1"]';

test("a tap on the setpoint opens the keypad, which types a value that is sent and confirmed", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto("/hmi/overview");
  await expect(page.locator(LEVEL)).toHaveAttribute("data-state", "confirmed", { timeout: 30_000 });
  await page.tap(SETPOINT);
  await page.fill('[data-part="login"] input[name="user"]', "operator");
  await page.fill('[data-part="login"] input[name="password"]', "operator");
  await page.tap('[data-part="login"] button[type="submit"]');
  await expect(page.locator('[data-part="session"]')).toContainText("operator", { timeout: 15_000 });
  await page.tap(SETPOINT);
  const keypad = page.locator('[data-part="entry"] [data-part="keypad"]');
  await expect(keypad).toBeVisible();
  for (let index = 0; index < 8; index += 1) await keypad.locator('[data-key="back"]').tap();
  for (const key of ["2", ".", "1"]) await keypad.locator(`[data-key="${key}"]`).tap();
  await expect(page.locator('[data-part="entry"] input[name="value"]')).toHaveValue("2.1");
  await page.screenshot({ path: test.info().outputPath("keypad.png") });
  await page.tap('[data-part="entry"] button[type="submit"]');
  await expect(page.locator(SETPOINT)).toHaveAttribute("data-outcome", "confirmed", { timeout: 15_000 });
  expect(errors).toEqual([]);
});
