// Spec 39 §18 Phase 5: two complete operator pages, reviewed in both themes. The overview has the
// level trend, whose equipment badge shows LT-101's low alarms as the tank drains; the Panels page
// (`cards.toml`) places the Plant run, PID and tank panels, its trend the PID panel's own.
import { expect, test } from "@playwright/test";
const BASE = `http://127.0.0.1:${Number(process.env.HMI_SCREEN_EQUIPMENT_PORT || 18481)}`;
for (const theme of ["dark", "light"]) for (const name of ["overview", "cards"]) {
  test(`${name} gives equipment, current readings, trends and alarms in ${theme}`, async ({ page }, info) => {
    await page.addInitScript((theme) => localStorage.setItem("trust-hmi-theme", theme), theme);
    await page.goto(`${BASE}/hmi/${name}`);
    const alarm = page.locator('[data-object="operating-alarms"]');
    await expect(alarm, "alarms stay on the operating page").toBeVisible();
    if (name === "overview") {
      await expect(page.locator('[data-object="level-gauge"]')).toContainText(/\d\.\d\d/, { timeout: 15000 });
      await expect(page.locator('[data-object="output-reading"]')).toContainText("%");
      await expect(page.locator('[data-object="pump-state"]')).toContainText("Pump running");
      const inlet = await page.locator('[data-pipe="1"] [data-part="line"]').getAttribute("d");
      const points = [...inlet.matchAll(/(-?[\d.]+) (-?[\d.]+)/g)].map(m => [Number(m[1]), Number(m[2])]);
      expect(points.at(-2)[1], "the feed pipe approaches the tank inlet from above").toBeLessThan(points.at(-1)[1]);
      // The drained tank's level is a flat line, which has no height to be "visible": count it.
      await expect(page.locator('[data-object="trend"] polyline[data-pen="Level"]')).toHaveCount(1, { timeout: 15000 });
      const badge = page.locator('[data-object="trend"] [data-part="equipment-alarm"]');
      await expect(badge).toBeVisible({timeout:60000});
      const badgeBounds = await badge.boundingBox();
      for (const action of await page.locator('[data-object="trend"] [data-trend-action], [data-object="trend"] [data-trend-range]').all()) {
        const bounds = await action.boundingBox();
        expect(bounds.x + bounds.width + 4, 'equipment alarm badge must leave trend actions clear').toBeLessThanOrEqual(badgeBounds.x);
      }
    } else {
      for (const id of ["p101", "t101", "lic101"]) {
        await expect(page.locator(`[data-placed="${id}"]`)).toBeVisible();
      }
      // A placed panel is drawn as its popup draws it (§8.9), so its trend draws its pens. Soft, so
      // the page's other readings are still checked when it does not.
      const history = page.locator('[data-placed="lic101"] [data-object="history"]');
      await expect.soft(history.locator('polyline[data-pen="setpoint"]'), "the placed PID panel's trend draws its setpoint").toHaveCount(1, { timeout: 15000 });
      await expect(page.locator('[data-placed="t101"] [data-object="level"]')).toContainText(/\d\.\d\d/, { timeout: 15000 });
      await expect(page.locator('[data-placed="p101"] [data-object="state"]')).toHaveAttribute("data-on", "true", { timeout: 15000 });
    }
    const gaps = await page.evaluate(() => {
      const bar = document.querySelector('.hmi-topbar').getBoundingClientRect();
      const main = document.querySelector('.hmi-main').getBoundingClientRect();
      const session = document.querySelector('[data-part="session"]').getBoundingClientRect();
      return { band: main.top - bar.bottom, sessionTop: session.top - bar.top, sessionBottom: bar.bottom - session.bottom };
    });
    expect(gaps.band, "the page begins immediately below the top bar").toBe(0);
    expect(gaps.sessionTop).toBeGreaterThanOrEqual(0);
    expect(gaps.sessionBottom).toBeGreaterThanOrEqual(0);
    await page.screenshot({ path: info.outputPath(`${name}-${theme}.png`) });
  });
}
