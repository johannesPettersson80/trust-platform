// Tutorial 24 (examples/tutorials/24_hmi_alarms), steps 5 to 9, as its README writes them, against the tank
// example in simulation at 1x on a runtime of its own (operator-journey.spec.mjs walks steps 3 and 4; specification
// 40, §4 and §5; specification 39, §13.6). Catches a step the tutorial teaches that the display or the alarm
// library no longer does: shelving and unshelving, the trip of the high-high level and its first out, the
// equipment marks, the Suppressed tab and the event log.
import { expect, test } from "@playwright/test";

const TANK = `http://127.0.0.1:${Number(process.env.HMI_SCREEN_TUTORIAL_PORT || 18485)}`;
const LEVEL = '[data-object="value-1"]';
const PUMP = '[data-object="p-101"]';
const MODE = '[data-object="mode-selector-1"]';

const row = (page, path) => page.locator(`[data-part="alarm-row"][data-alarm="${path}"]`);
const tab = (page, name) => page.locator(`[data-alarm-tab="${name}"]`);

/** The level the card shows, in metres. */
async function level(page) {
  const text = await page.locator(LEVEL).textContent();
  return Number.parseFloat(/-?\d+(\.\d+)?/.exec(text ?? "")?.[0] ?? "NaN");
}

/** Writes `value` into the control `object` and waits for its readback (§12.4). */
async function enter(page, object, value) {
  await page.click(`[data-object="${object}"]`);
  await page.fill('[data-part="entry"] input[name="value"]', String(value));
  await page.click('[data-part="entry"] button[type="submit"]');
  await expect(page.locator(`[data-object="${object}"]`)).toHaveAttribute("data-outcome", "confirmed", {
    timeout: 15_000,
  });
}

/** Chooses the controller mode `option` and waits until the selector shows it. */
async function mode(page, option) {
  await page.click(`${MODE} [data-option="${option}"]`);
  await expect(page.locator(`${MODE} [data-option="${option}"]`)).toHaveAttribute("data-active", "true", {
    timeout: 15_000,
  });
}

/** Acknowledges every alarm of the active list that still waits for it. */
async function acknowledgeAll(page) {
  const buttons = page.locator('[data-part="alarm-row"] [data-alarm-action="acknowledge"]');
  while ((await buttons.count()) > 0) {
    await buttons.first().click();
    await page.waitForTimeout(500);
  }
}

test("tutorial 24, steps 5 to 9: shelve, a trip and its first out, equipment alarms, suppression and the event log", async ({
  page,
}) => {
  test.setTimeout(900_000);
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto(`${TANK}/hmi/overview`);
  await expect(page.locator(LEVEL)).toHaveAttribute("data-state", "confirmed", { timeout: 60_000 });

  // 8. In Auto the level-rise alarm does not apply: it is listed on the Suppressed tab, so nothing is hidden.
  await tab(page, "suppressed").click();
  await expect(row(page, "Tank.Vessel.Rising")).toBeVisible({ timeout: 15_000 });
  await tab(page, "active").click();

  // 3. As operator, the setpoint at 2.6 m raises the high-level alarm once the level passes 2.5 m.
  await page.click('[data-object="setpoint-1"]');
  await page.fill('[data-part="login"] input[name="user"]', "operator");
  await page.fill('[data-part="login"] input[name="password"]', "operator");
  await page.click('[data-part="login"] button[type="submit"]');
  await expect(page.locator('[data-part="session"]')).toContainText("operator", { timeout: 15_000 });
  await page.keyboard.press("Escape");
  await enter(page, "setpoint-1", 2.6);
  await expect(row(page, "Tank.Vessel.High")).toBeVisible({ timeout: 180_000 });

  // 5. Shelve for 15 min takes it off the active list without losing it: it is on the Shelved tab.
  await row(page, "Tank.Vessel.High").click();
  await page.click('[data-action="shelve"][data-shelve="900"]');
  await expect(row(page, "Tank.Vessel.High")).toHaveCount(0, { timeout: 15_000 });
  await tab(page, "shelved").click();
  await expect(row(page, "Tank.Vessel.High")).toBeVisible({ timeout: 15_000 });
  // Unshelve: its condition still holds, so it comes back unacknowledged.
  await row(page, "Tank.Vessel.High").click();
  await page.click('[data-action="unshelve"]');
  await tab(page, "active").click();
  await expect(row(page, "Tank.Vessel.High").locator('[data-alarm-action="acknowledge"]')).toBeVisible({
    timeout: 15_000,
  });

  // 6. Manual, the valve at 100 %: the level rises past 2.8 m and the high-high level trips the pump.
  await mode(page, "Manual");
  await enter(page, "numeric-input-1", 100);
  await expect(row(page, "Tank.Vessel.HighHigh")).toBeVisible({ timeout: 300_000 });
  // P-101 trips: its impeller stands still.
  await expect(page.locator(PUMP)).toHaveAttribute("data-equipment", "fault", { timeout: 15_000 });
  // 7. The level card, bound inside the vessel, shows the vessel's alarms.
  await expect(page.locator(`${LEVEL} [data-part="equipment-alarm"]`)).toBeVisible();
  // The high-high alarm is marked First out, the cause of the trip.
  await row(page, "Tank.Vessel.HighHigh").click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("Tank.Vessel.HighHigh");
  await expect(dialog).toContainText(/First out\s*Yes/);
  await page.click('[data-action="cancel"]');

  // Back in Auto with the level below the limits, Reset trip resets the latched alarms and the pump runs again.
  await mode(page, "Auto");
  await enter(page, "setpoint-1", 1.5);
  await expect.poll(() => level(page), { timeout: 300_000, intervals: [2_000] }).toBeLessThan(2.4);
  await acknowledgeAll(page);
  await page.click('[data-object="pulse-button-1"]');
  await expect(page.locator(PUMP)).toHaveAttribute("data-equipment", "running", { timeout: 30_000 });
  await acknowledgeAll(page);
  await expect(tab(page, "active")).toContainText("Active 0", { timeout: 30_000 });

  // 9. The Alarms page's event log lists the alarm transitions and the operator's actions.
  await page.goto(`${TANK}/hmi/alarms`);
  const log = page.locator('[data-object="event-log-1"]');
  // Its newest page: the operator's acknowledgements with who did them and their outcome, and the
  // high-high alarm clearing after the reset.
  await expect(log).toContainText("Confirmed · Acknowledge", { timeout: 30_000 });
  await expect(log).toContainText("operator");
  await expect(log).toContainText("Cleared · T-101 level high-high");
  expect(errors).toEqual([]);
});
