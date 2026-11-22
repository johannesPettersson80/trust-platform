// examples/room_heating's "Try it", steps 1 and 4, as its README writes them, on a runtime of its own in
// simulation at 1x (specification 39, §13.6 and §12.4; specification 40, §3). Catches a step the README teaches
// that the example no longer does: the heating sequence started from its panel, the low-water trip failing it with
// "T-101 water low" and stopping the pump, and the recovery by acknowledge, Reset and Start. The Simulation page's
// fault switches are locked to the Engineer role, so the test logs in as engineer.
import { expect, test } from "@playwright/test";

const ROOM = `http://127.0.0.1:${Number(process.env.HMI_SCREEN_ROOM_HEATING_PORT || 18486)}`;
const PANEL = '[data-object="heating-panel"]';
const PUMP = '[data-object="pump-1"]';
const LOW_WATER = '[data-object="switch-1"]';

/** Presses the heating panel's command `name`. */
async function press(page, name) {
  await page.locator(PANEL).getByText(name, { exact: true }).click();
}

/** Turns the Simulation page's low-water switch to `on` and comes back to the overview. */
async function lowWater(page, on) {
  await page.goto(`${ROOM}/hmi/simulation`);
  await expect(page.locator(LOW_WATER)).toHaveAttribute("data-state", "confirmed", { timeout: 30_000 });
  await page.click(LOW_WATER);
  await expect(page.locator(LOW_WATER)).toHaveAttribute("data-on", String(on), { timeout: 15_000 });
  await page.goto(`${ROOM}/hmi/overview`);
  await expect(page.locator(PANEL)).toBeVisible({ timeout: 30_000 });
}

test("room heating, Try it steps 1 and 4: the sequence heats the room, low water trips it, reset and start recover", async ({
  page,
}) => {
  // The runtime starts with the other web servers and stands idle while earlier specs run. Its valve stays closed
  // until the room heats (checklist G252), so the tank loses only a little heat, and Start heats it to 40 °C within a
  // few minutes at 1x.
  test.setTimeout(600_000);
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto(`${ROOM}/hmi/overview`);
  await expect(page.locator('[data-object="value-1"]')).toHaveAttribute("data-state", "confirmed", { timeout: 60_000 });

  // 1. Start on the Heating panel: P-101 runs, and once the tank is warm the sequence heats the room.
  await press(page, "Start");
  await page.fill('[data-part="login"] input[name="user"]', "engineer");
  await page.fill('[data-part="login"] input[name="password"]', "engineer");
  await page.click('[data-part="login"] button[type="submit"]');
  await expect(page.locator('[data-part="session"]')).toContainText("engineer", { timeout: 15_000 });
  await page.keyboard.press("Escape");
  await press(page, "Start");
  await expect(page.locator(PUMP)).toHaveAttribute("data-equipment", "running", { timeout: 30_000 });
  await expect(page.locator(PANEL)).toContainText("Heat the room", { timeout: 420_000 });

  // 4. Low water: the protections trip, the pump stops, the sequence fails with "T-101 water low" and the alarm
  // list shows it.
  await lowWater(page, true);
  await expect(page.locator('[data-object="value-1"]')).toContainText("Failed", { timeout: 30_000 });
  await expect(page.locator(PANEL)).toContainText("T-101 water low");
  await expect(page.locator(PUMP)).toHaveAttribute("data-equipment", "stopped", { timeout: 15_000 });
  await expect(page.locator('[data-part="alarm-row"][data-alarm="Main.LSL101.SwitchAlarm"]')).toBeVisible({
    timeout: 15_000,
  });

  // Switch it off, acknowledge, Reset, then Start: the sequence runs again and no alarm is left.
  await lowWater(page, false);
  // The overview draws its alarm list after its first update: wait for the low-water row's Acknowledge, or the loop
  // below finds no button and the alarm, back to normal but unacknowledged, stays listed (checklist G278).
  const acknowledge = page.locator('[data-part="alarm-row"] [data-alarm-action="acknowledge"]');
  await expect(
    page.locator('[data-part="alarm-row"][data-alarm="Main.LSL101.SwitchAlarm"] [data-alarm-action="acknowledge"]'),
  ).toBeVisible({ timeout: 15_000 });
  while ((await acknowledge.count()) > 0) {
    await acknowledge.first().click();
    await page.waitForTimeout(500);
  }
  await press(page, "Reset");
  await expect(page.locator('[data-object="value-1"]')).toContainText("Idle", { timeout: 15_000 });
  await press(page, "Start");
  await expect(page.locator('[data-object="value-1"]')).toContainText("Running", { timeout: 15_000 });
  await expect(page.locator(PUMP)).toHaveAttribute("data-equipment", "running", { timeout: 30_000 });
  await expect(page.locator('[data-part="alarm-row"]')).toHaveCount(0, { timeout: 15_000 });
  expect(errors).toEqual([]);
});
