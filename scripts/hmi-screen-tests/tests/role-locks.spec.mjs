// A control the signed-in role may not use (specification 39, §5.9 item 5), against the equipment
// example in simulation: an Operator opens the pump's Plant panel; Out of service, which the panel
// gives to Engineers (Plant §3.6), is dimmed under a lock naming the Engineer and a press sends
// nothing; Start stays usable.
import { expect, test } from "@playwright/test";

const EQUIPMENT = `http://127.0.0.1:${Number(process.env.HMI_SCREEN_EQUIPMENT_PORT || 18481)}`;
const POPUP = '[data-part="popup"]';

test("an Operator sees the Engineer's control locked under the Engineer's name, and its press sends nothing", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto(`${EQUIPMENT}/hmi/overview`);
  await page.click('[data-action="login"]');
  await page.fill('[data-part="login"] input[name="user"]', "operator");
  await page.fill('[data-part="login"] input[name="password"]', "operator");
  await page.click('[data-part="login"] button[type="submit"]');
  await expect(page.locator('[data-part="session"]')).toContainText("operator", { timeout: 15_000 });
  await page.click('[data-object="p-101"]');
  const outOfService = page.locator(`${POPUP} [data-object="out-of-service"]`);
  await expect(outOfService).toHaveAttribute("data-locked", "engineer", { timeout: 15_000 });
  await expect(outOfService.locator('[data-part="lock"]')).toContainText("Engineer");
  expect(await page.locator(`${POPUP} [data-object="start"]`).getAttribute("data-locked")).toBeNull();
  await page.screenshot({ path: test.info().outputPath("role-lock.png") });
  await outOfService.click();
  await expect(page.locator('[data-part="notice"]')).toContainText("Engineer");
  await expect(outOfService).not.toHaveAttribute("data-outcome", /./);
  expect(errors).toEqual([]);
});
