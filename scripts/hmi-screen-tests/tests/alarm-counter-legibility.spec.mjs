import { expect, test } from "@playwright/test";

const EQUIPMENT = `http://127.0.0.1:${Number(process.env.HMI_SCREEN_RANGES_PORT || 18482)}`;
for (const theme of ["dark", "light"]) {
  test(`the alarm count stays readable through its attention blink in ${theme}`, async ({ page }, info) => {
    await page.addInitScript((theme) => localStorage.setItem("trust-hmi-theme", theme), theme);
    await page.goto(`${EQUIPMENT}/hmi/equipment-ranges`);
    const count = page.locator('[data-part="alarm-count"]');
    await expect(count).toHaveAttribute("data-blink", "true", { timeout: 30000 });
    const state = await count.evaluate((element) => {
      for (const animation of element.getAnimations({ subtree: true })) { animation.pause(); animation.currentTime = 750; }
      return { opacity: getComputedStyle(element).opacity, markAnimations: element.querySelector(".hmi-alarm-mark").getAnimations().length };
    });
    expect(state.opacity, "the number and badge remain fully opaque").toBe("1");
    expect(state.markAnimations, "the priority mark still calls for acknowledgement").toBeGreaterThan(0);
    await page.screenshot({ path: info.outputPath(`alarm-counter-${theme}.png`) });
  });
}
