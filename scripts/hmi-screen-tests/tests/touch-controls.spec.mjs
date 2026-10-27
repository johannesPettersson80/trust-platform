// Spec 39 §3.5: the shared runtime controls keep 44px targets and 14px text.
// The editor capture first exposed the old 36px theme / 30px login buttons; this also locks
// the rebuilt runtime surface, using the same styles, in both themes.
import { expect, test } from "@playwright/test";

for (const theme of ["dark", "light"]) {
  test(`session controls have touch-sized targets in ${theme}`, async ({ page }, testInfo) => {
    await page.addInitScript((theme) => localStorage.setItem("trust-hmi-theme", theme), theme);
    await page.goto("/hmi/overview");
    await expect(page.locator('[data-object="setpoint-1"]')).toHaveAttribute("data-state", "confirmed");
    // One evaluation finds the controls and measures them, so a redraw between the two cannot leave
    // detached controls, which measure as empty and would drop out unchecked.
    const sizes = async (selector) => page.evaluate((selector) => [...document.querySelectorAll(selector)].filter((element) => element.getBoundingClientRect().width > 0).map((element) => {
      const box = element.getBoundingClientRect();
      return { width: box.width, height: box.height, font: parseFloat(getComputedStyle(element).fontSize) };
    }), selector);
    const check = (controls) => {
      expect(controls.length).toBeGreaterThan(0);
      for (const control of controls) {
        expect(control.width).toBeGreaterThanOrEqual(44);
        expect(control.height).toBeGreaterThanOrEqual(44);
        expect(control.font).toBeGreaterThanOrEqual(14);
      }
    };
    check(await sizes('.hmi-topbar button'));
    await page.click('[data-action="login"]');
    check(await sizes('[data-part="login"] button, [data-part="login"] input'));
    await page.screenshot({ path: testInfo.outputPath(`login-${theme}.png`) });
    await page.fill('[data-part="login"] input[name="user"]', "operator");
    await page.fill('[data-part="login"] input[name="password"]', "operator");
    await page.click('[data-part="login"] button[type="submit"]');
    await expect(page.locator('[data-part="session"]')).toContainText("operator", { timeout: 15_000 });
    check(await sizes('.hmi-topbar button'));
    await page.click('[data-object="setpoint-1"]');
    check(await sizes('[data-part="entry"] button, [data-part="entry"] input'));
  });
}
