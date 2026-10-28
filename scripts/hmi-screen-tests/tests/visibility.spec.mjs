// A visibility binding (specification 39, §18 phase 5 item 2), against the tank-level example in
// simulation: the page answer gets a visibility binding on the level value, `Tank.PumpRun`, which
// the page's switch writes; the value hides while the pump is off and shows while it runs.
import { expect, test } from "@playwright/test";

const LEVEL = '[data-object="value-1"]';
const SWITCH = '[data-object="switch-1"]';

test("an object hides while its BOOL is FALSE and shows while it is TRUE", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.route("**/api/hmi/page*", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    const level = body.page?.model?.object?.find((object) => object.id === "value-1");
    if (level) level.bind.visible = { path: "Tank.PumpRun", direction: "read" };
    await route.fulfill({ response, json: body });
  });
  await page.goto("/hmi/overview");
  await expect(page.locator(SWITCH)).toBeVisible({ timeout: 30_000 });
  await page.click(SWITCH);
  await page.fill('[data-part="login"] input[name="user"]', "operator");
  await page.fill('[data-part="login"] input[name="password"]', "operator");
  await page.click('[data-part="login"] button[type="submit"]');
  await expect(page.locator('[data-part="session"]')).toContainText("operator", { timeout: 15_000 });
  const running = (await page.locator(SWITCH).getAttribute("data-state")) !== "off";
  const flip = async () => {
    await page.click(SWITCH);
    await expect(page.locator(SWITCH)).toHaveAttribute("data-outcome", /confirmed|applied/, { timeout: 15_000 });
  };
  if (running) {
    await expect(page.locator(LEVEL)).not.toHaveAttribute("data-hidden", "true");
    await flip();
  }
  await expect(page.locator(LEVEL)).toHaveAttribute("data-hidden", "true", { timeout: 15_000 });
  await page.screenshot({ path: test.info().outputPath("hidden.png") });
  await flip();
  await expect(page.locator(LEVEL)).not.toHaveAttribute("data-hidden", "true", { timeout: 15_000 });
  expect(errors).toEqual([]);
});
