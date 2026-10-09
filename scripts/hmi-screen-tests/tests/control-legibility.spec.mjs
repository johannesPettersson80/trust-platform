// Unavailable controls stay readable (specification 39, §5.9 item 5; §3.5), on pump P-101's Plant
// panel in the equipment example: without a session Start is read-only; for an Operator the
// Engineer's Out of service is dimmed under its lock, its press sends nothing, and Start is free.
import { expect, test } from "@playwright/test";

const EQUIPMENT = `http://127.0.0.1:${Number(process.env.HMI_SCREEN_EQUIPMENT_PORT || 18481)}`;
async function textOpacities(object) {
  return object.locator("text").evaluateAll((texts) => texts.map((text) => {
    let opacity = 1;
    for (let element = text; element; element = element.parentElement) opacity *= Number(getComputedStyle(element).opacity);
    return opacity;
  }));
}
async function expectReadable(object) {
  const opacities = await textOpacities(object);
  expect(opacities.length).toBeGreaterThan(0);
  expect(Math.min(...opacities), "labels and role badges retain full opacity").toBe(1);
}
for (const theme of ["dark", "light"]) {
  test(`unavailable controls remain readable and refuse writes in ${theme}`, async ({ page }, info) => {
    const writes = [];
    page.on("request", (request) => { if (request.method() === "POST" && request.url().endsWith("/api/hmi/command")) writes.push(request.postData()); });
    await page.addInitScript((theme) => localStorage.setItem("trust-hmi-theme", theme), theme);
    await page.goto(`${EQUIPMENT}/hmi/overview`);
    await page.click('[data-object="p-101"]');
    const start = page.locator('[data-part="popup"] [data-object="start"]');
    await expect(start).toHaveAttribute("data-readonly", "true");
    await expectReadable(start);
    await page.screenshot({ path: info.outputPath(`readonly-${theme}.png`) });
    await start.click();
    await page.fill('[data-part="login"] input[name="user"]', "operator");
    await page.fill('[data-part="login"] input[name="password"]', "operator");
    await page.click('[data-part="login"] button[type="submit"]');
    await expect(page.locator('[data-part="session"]')).toContainText("operator", { timeout: 15000 });
    const outOfService = page.locator('[data-part="popup"] [data-object="out-of-service"]');
    await expect(outOfService).toHaveAttribute("data-locked", "engineer");
    await expectReadable(outOfService);
    await expect(outOfService.locator('[data-part="face"]')).toHaveAttribute("opacity", "0.45");
    await expect(start).not.toHaveAttribute("data-locked", /./);
    await page.screenshot({ path: info.outputPath(`role-locked-${theme}.png`) });
    await outOfService.click();
    await expect(page.locator('[data-part="notice"]')).toContainText("Engineer");
    expect(writes).toEqual([]);
  });
}
