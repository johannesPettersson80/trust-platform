// Live equipment presentation (specification 39, §5.9), against the equipment example's copy with
// the equipment-ranges fixture (serve-example.mjs): a `Plant.FB_AnalogInput` reads 100 on 0-200
// while the PLC switches its unit and high limit; the gauge and bar follow at once, and the alarm
// its high limit raised keeps the unit it was raised with.
import { expect, test } from "@playwright/test";

const EQUIPMENT = `http://127.0.0.1:${Number(process.env.HMI_SCREEN_RANGES_PORT || 18482)}`;
for (const theme of ["dark", "light"]) {
  test(`equipment metadata changes reach the live drawing in ${theme}`, async ({ page }, info) => {
    const errors = [], samples = [];
    page.on("pageerror", (error) => errors.push(String(error)));
    page.on("websocket", (socket) => socket.on("framereceived", ({ payload }) => {
      try { const value = JSON.parse(String(payload)).values?.["Main.Metadata.Value"]; if (value) samples.push(value); } catch { /* non-value traffic */ }
    }));
    await page.addInitScript((theme) => localStorage.setItem("trust-hmi-theme", theme), theme);
    await page.goto(`${EQUIPMENT}/hmi/equipment-ranges`);
    const gauge = page.locator('[data-object="gauge"]');
    await expect(gauge).toHaveAttribute("data-state", "confirmed");
    await page.click('[data-action="login"]');
    await page.fill('[data-part="login"] input[name="user"]', "operator");
    await page.fill('[data-part="login"] input[name="password"]', "operator");
    await page.click('[data-part="login"] button[type="submit"]');
    await expect(page.locator('[data-part="session"]')).toContainText("operator", { timeout: 15000 });
    const toggle = page.locator('[data-object="alternate"]');
    await expect(toggle).toContainText("Alternate display metadata");
    if ((await gauge.textContent()).includes("deg C")) {
      await toggle.click();
      await expect(gauge).toContainText("100.00°C");
    }
    await expect(gauge).toContainText("100.00°C");
    await expect(gauge.locator('[data-limit="high"]')).toHaveAttribute("data-fraction", "0.45");
    const recorded = page.locator('[data-part="alarm-row"][data-alarm="Main.Metadata.HighAlarm"] [data-column="value"]');
    await expect(recorded).toHaveText("100.00 > 90.00 °C");
    await toggle.click();
    await expect(toggle).toHaveAttribute("data-outcome", "confirmed", { timeout: 15000 });
    for (const id of ["gauge", "bar"]) {
      await expect(page.locator(`[data-object="${id}"]`)).toContainText("100.00deg C");
      await expect(page.locator(`[data-object="${id}"] [data-limit="high"]`)).toHaveAttribute("data-fraction", "0.7");
      await expect(page.locator(`[data-object="${id}"] [data-limit]`)).toHaveCount(4);
    }
    expect(samples.some((sample) => sample.value === 100 && sample.presentation?.unit === "°C")).toBe(true);
    expect(samples.some((sample) => sample.value === 100 && sample.presentation?.unit === "deg C" && sample.presentation?.limits?.high === 140)).toBe(true);
    await expect(recorded).toHaveText("100.00 > 90.00 °C");
    await page.screenshot({ path: info.outputPath(`equipment-ranges-${theme}.png`) });
    expect(errors).toEqual([]);
  });
}
