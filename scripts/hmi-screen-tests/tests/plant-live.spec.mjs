// The Plant journey (specification 39, §8.4, §21.2, §21.4 and §21.7; Plant library §3, §4.2 and
// §12) against the disposable project of `plant-live-fixture.mjs`, at the HMI screen's size of
// 1280 × 800: an Engineer opens pump P101's panel, titled "P101 — Pump"; in Auto, Start is greyed
// with the pump's reason and the runtime refuses it; in Manual, Start and the stop-variant Stop are
// confirmed by the running feedback; the Settings dialog applies a setting and shows it again when
// it opens anew; the runtime lists the process alarm, which is acknowledged. An Operator sees no
// Settings button, the Engineer's buttons locked, and the settings route refuses the Operator.
import { expect, test } from "@playwright/test";

const BASE = "http://127.0.0.1:18484";
const POPUP = '[data-part="popup"]';
/** The pump's `CommandText` in Auto (Plant §4.2 item 3). */
const AUTO_REASON = "Auto: switch to Manual";
/** How a refusal reads beside the control and in the notice (§21.2 item 3, §21.8 item 4). */
const AUTO_REFUSAL = `Not accepted: ${AUTO_REASON}`;

test.use({ viewport: { width: 1280, height: 800 } });

async function login(page, user) {
  await page.click('[data-action="login"]');
  await page.fill('[data-part="login"] input[name="user"]', user);
  await page.fill('[data-part="login"] input[name="password"]', user);
  await page.click('[data-part="login"] button[type="submit"]');
  await expect(page.locator('[data-part="session"]')).toContainText(user, { timeout: 15_000 });
}

/** Opens P101's panel and waits until it shows the pump's mode. */
async function openPump(page) {
  await page.click('[data-object="pump"]');
  const popup = page.locator(POPUP);
  await expect(popup.locator('[data-part="popup-title"]')).toHaveText("P101 — Pump");
  await expect(popup.locator('[data-object="mode"] [data-option][data-active="true"]')).toHaveCount(1, { timeout: 15_000 });
  return popup;
}

async function shot(page, name) {
  await page.screenshot({ path: test.info().outputPath(name) });
}

test("an Engineer operates pump P101 in Manual, applies a setting and acknowledges the process alarm", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto(`${BASE}/hmi`);
  await expect(page.locator('[data-object="pump"]')).toBeVisible({ timeout: 30_000 });
  await login(page, "engineer");
  const popup = await openPump(page);
  const mode = popup.locator('[data-object="mode"]');
  const start = popup.locator('[data-object="start"]');
  const stop = popup.locator('[data-object="stop"]');
  const running = popup.locator('[data-object="state"]');
  const notice = page.locator('[data-part="notice"]');

  // Auto: the program commands the pump. Start and Stop are greyed with the pump's reason (§8.4
  // item 4), and a press is refused by the runtime with that reason (§12.6, §21.2 item 3). The
  // greyed look and the exact words are soft, so the journey goes on and reports every step.
  await expect(mode.locator('[data-option="Auto"]')).toHaveAttribute("data-active", "true");
  await expect(running).toHaveAttribute("data-on", "false");
  await expect(popup.locator('[data-object="reason"]')).toHaveText(AUTO_REASON);
  await expect.soft(start.locator('[data-part="face"]'), "Start is greyed in Auto").toHaveAttribute("opacity", "0.45");
  await expect.soft(stop.locator('[data-part="face"]'), "Stop is greyed in Auto").toHaveAttribute("opacity", "0.45");
  await start.click();
  await expect(start).toHaveAttribute("data-outcome", "refused");
  await expect(notice).toContainText(AUTO_REASON);
  await expect.soft(notice, "the notice names the pump's reason").toHaveText(AUTO_REFUSAL);
  await expect.soft(start.locator("title"), "the reason beside Start").toHaveText(AUTO_REFUSAL);
  await expect(running).toHaveAttribute("data-on", "false");
  await shot(page, "plant-auto-refused.png");

  // Manual: the operator commands the pump through the mode selector (Plant §3.2, §3.3).
  await mode.locator('[data-option="Manual"]').click();
  await expect(mode.locator('[data-option="Manual"]')).toHaveAttribute("data-active", "true", { timeout: 15_000 });
  await expect(start.locator('[data-part="face"]')).not.toHaveAttribute("opacity", "0.45");
  await expect(stop.locator('[data-part="face"]')).not.toHaveAttribute("opacity", "0.45");
  await shot(page, "plant-manual.png");

  // Start, confirmed when the running feedback arrives (§12.5).
  await start.click();
  await expect(start).toHaveAttribute("data-outcome", "confirmed", { timeout: 15_000 });
  await expect(running).toHaveAttribute("data-on", "true");
  await shot(page, "plant-running.png");

  // Stop is the stop variant, never looking like Start (§8.4 item 4), confirmed when it stops.
  await expect(stop).toHaveAttribute("class", /hmi-button-stop/);
  await stop.click();
  await expect(stop).toHaveAttribute("data-outcome", "confirmed", { timeout: 15_000 });
  await expect(running).toHaveAttribute("data-on", "false");
  await shot(page, "plant-stopped.png");

  // Settings, for Engineers only: the definition's settings in order with their units; Apply and
  // Cancel at the bottom, the result beside Apply (§21.4).
  await popup.locator('[data-action="settings"]').click();
  const form = page.locator('[data-part="equipment-settings"]');
  const footer = form.locator('[data-part="settings-footer"]');
  const startTimeout = form.locator('input[name="StartTimeout"]');
  await expect(form.locator("h2")).toHaveText("P101 — Pump · Settings");
  // The feedback flag is a setting too (7 October 2026: the definitions offer it).
  await expect(form.locator("[data-member] > span")).toHaveText(
    ["Has running feedback", "Start timeout (s)", "Stop timeout (s)", "Service interval (h)"],
    { timeout: 15_000 },
  );
  await expect(startTimeout).toHaveValue("5");
  await startTimeout.fill("10");
  await expect(footer.locator('[data-action="apply"]')).toBeInViewport();
  await expect(footer.locator('[data-action="cancel"]')).toBeInViewport();
  await footer.locator('[data-action="apply"]').click();
  await expect(footer.locator('[data-part="settings-result"]')).toHaveText("Applied", { timeout: 30_000 });
  await expect(startTimeout).toHaveValue("10");
  await shot(page, "plant-settings-applied.png");

  // Opened again, the dialog reads the value back from the runtime.
  await footer.locator('[data-action="cancel"]').click();
  await expect(form).toHaveCount(0);
  await popup.locator('[data-action="settings"]').click();
  await expect(startTimeout).toHaveValue("10", { timeout: 15_000 });
  await shot(page, "plant-settings-reopened.png");

  // The runtime lists every alarm instance itself; the list binds nothing (specification 40, §5).
  await page.goto(`${BASE}/hmi/alarms`);
  const row = page.locator('[data-part="alarm-row"]', { hasText: "Test process alarm" });
  await expect(row).toBeVisible({ timeout: 15_000 });
  await shot(page, "plant-alarms.png");
  await row.locator('[data-alarm-action="acknowledge"]').click();
  await expect(row.locator('[data-alarm-action="acknowledge"]')).toHaveCount(0, { timeout: 15_000 });
  expect(errors).toEqual([]);
});

test("an Operator sees no Settings button on pump P101's panel, and the settings route refuses", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto(`${BASE}/hmi`);
  await expect(page.locator('[data-object="pump"]')).toBeVisible({ timeout: 30_000 });
  await login(page, "operator");
  const popup = await openPump(page);
  await expect(popup.locator('[data-action="settings"]')).toHaveCount(0);
  // Out of service, Return to service and Service done are the Engineer's (Plant §3.6, §12.4).
  for (const id of ["out-of-service", "return-to-service", "service-done"]) {
    await expect(popup.locator(`[data-object="${id}"]`)).toHaveAttribute("data-locked", "engineer");
  }
  expect(await popup.locator('[data-object="start"]').getAttribute("data-locked")).toBeNull();
  await shot(page, "plant-operator-panel.png");
  // §21.4 item 1: the route refuses other roles, also when asked directly with the session.
  const answer = await page.request.post(`${BASE}/api/hmi/settings`, {
    data: { faceplate: "faceplate:plant-live/pump", action: "read" },
  });
  expect(answer.status()).toBe(200);
  const refusal = await answer.json();
  expect(refusal.ok).toBe(false);
  expect(refusal.error).toContain("Engineer");
  expect(errors).toEqual([]);
});
