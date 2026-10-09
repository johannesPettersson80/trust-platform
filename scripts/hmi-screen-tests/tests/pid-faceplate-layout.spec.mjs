// Specification 39 §18 phase 5: PID faceplate readings, labels and controls do not overlap, on the
// Plant PID panel (`plant-pid`) of LIC-101 in the equipment example, once its setpoint reads.
import { expect, test } from "@playwright/test";
const BASE = `http://127.0.0.1:${Number(process.env.HMI_SCREEN_EQUIPMENT_PORT || 18481)}`;
for (const theme of ["dark", "light"]) {
  test(`PID faceplate has readable separated labels in ${theme}`, async ({ page }, info) => {
    await page.addInitScript((theme) => localStorage.setItem("trust-hmi-theme", theme), theme);
    await page.goto(`${BASE}/hmi/overview`);
    await page.click('[data-object="lic-101"]');
    const popup = page.locator('[data-part="popup"]');
    await expect(popup).toBeVisible();
    await expect(popup.locator('[data-object="setpoint"]')).toContainText(/\d\.\d/, { timeout: 15000 });
    await page.screenshot({ path: info.outputPath(`pid-${theme}.png`) });
    const overlaps = await popup.locator("svg text").evaluateAll((nodes) => {
      const texts = nodes.filter(n => n.textContent.trim()).map(n => ({
        text: n.textContent, object: n.closest("[data-object]")?.getAttribute("data-object"), rect: n.getBoundingClientRect(),
      }));
      const problems = [];
      for (let i = 0; i < texts.length; i++) for (let j = i + 1; j < texts.length; j++) {
        const a = texts[i], b = texts[j];
        if (Math.min(a.rect.right, b.rect.right) - Math.max(a.rect.left, b.rect.left) > 1 &&
            Math.min(a.rect.bottom, b.rect.bottom) - Math.max(a.rect.top, b.rect.top) > 1)
          problems.push(`${a.object}: ${a.text} overlaps ${b.object}: ${b.text}`);
      }
      return problems;
    });
    expect(overlaps).toEqual([]);
  });
}
