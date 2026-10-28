// Moving parts in a layer of their own (specification 39, §19.6; §18 phase 5 item 4), against the
// tank-level example in simulation: the running pump's impeller turns in an element of its own
// above the page, where the page draws it, and keeps turning across updates, so an update does not
// restyle or repaint it. A panel placed at twice its size on the equipment example's copy keeps
// its turning impeller aligned in the same way (§8.9).
import { expect, test } from "@playwright/test";

const LEVEL = '[data-object="value-1"]';
const MOVING = '[data-motion-layer] [data-motion="p-101"]';
const ORIGINAL = '[data-object="p-101"] .hmi-spin';

function centre(rect) {
  return [rect.x + rect.width / 2, rect.y + rect.height / 2];
}

test("a running pump's impeller turns in its own layer, where the page draws it, across updates", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto("/hmi/overview");
  await expect(page.locator(LEVEL)).toHaveAttribute("data-state", "confirmed", { timeout: 30_000 });
  const moving = page.locator(MOVING);
  await expect(moving).toHaveCount(1, { timeout: 15_000 });
  expect(await moving.evaluate((node) => node.closest("svg.hmi-page") === null), "outside the page's drawing").toBe(true);
  // The page's own impeller is hidden and does not turn.
  const original = await page.locator(ORIGINAL).evaluate((node) => ({
    visibility: getComputedStyle(node).visibility,
    animations: node.getAnimations().length,
    rect: node.getBoundingClientRect().toJSON(),
  }));
  expect(original.visibility).toBe("hidden");
  expect(original.animations).toBe(0);
  // The layer's impeller stands where the page draws it.
  const placed = await moving.evaluate((node) => node.getBoundingClientRect().toJSON());
  const [x, y] = centre(placed);
  const [expectedX, expectedY] = centre(original.rect);
  expect(Math.abs(x - expectedX)).toBeLessThan(1.5);
  expect(Math.abs(y - expectedY)).toBeLessThan(1.5);
  // Its size as laid out: its bounding box turns with it.
  const width = await moving.evaluate((node) => parseFloat(getComputedStyle(node).width));
  expect(Math.abs(width - original.rect.width)).toBeLessThan(2);
  // It turns: a running animation of its own element.
  const first = await moving.evaluate((node) => {
    const [animation] = node.getAnimations();
    return { name: animation?.animationName, time: Number(animation?.currentTime ?? 0), state: animation?.playState };
  });
  expect(first.name).toBe("hmi-spin");
  expect(first.state).toBe("running");
  // Updates leave it alone: the same element keeps turning.
  const element = await moving.elementHandle();
  await page.waitForTimeout(1500);
  const later = await page.locator(MOVING).elementHandle();
  expect(await element.evaluate((node, other) => node === other, later), "the same element").toBe(true);
  const time = await later.evaluate((node) => Number(node.getAnimations()[0]?.currentTime ?? 0));
  expect(time).toBeGreaterThan(first.time + 1000);
  expect(errors).toEqual([]);
});

// The Plant panels carry no symbol, so the equipment example's copy has a panel of the fixture's
// own (fixtures/turning-panel.toml) placed at twice its size for pump P-101, which runs in Auto.
test("a scaled faceplate keeps its impeller aligned with the pump across updates and resizing", async ({ page }) => {
  await page.goto(`http://127.0.0.1:${Number(process.env.HMI_SCREEN_EQUIPMENT_PORT || 18481)}/hmi/placed-turning`);
  const moving = page.locator('[data-motion-layer] [data-motion="symbol"]');
  const original = page.locator('[data-placed="pump-panel"] .hmi-spin');
  await expect(moving).toHaveCount(1, { timeout: 15000 });
  const aligned = async () => {
    const a = await moving.evaluate(node => node.getBoundingClientRect().toJSON());
    const b = await original.evaluate(node => node.getBoundingClientRect().toJSON());
    expect(Math.abs(centre(a)[0] - centre(b)[0]), "horizontal impeller centre").toBeLessThan(1.5);
    expect(Math.abs(centre(a)[1] - centre(b)[1]), "vertical impeller centre").toBeLessThan(1.5);
    const width = await moving.evaluate(node => parseFloat(getComputedStyle(node).width));
    expect(Math.abs(width - b.width), "faceplate scaling applies to the motion layer").toBeLessThan(2);
  };
  await aligned();
  const element = await moving.elementHandle();
  await page.waitForTimeout(1000);
  await aligned();
  expect(await element.evaluate(node => node.isConnected)).toBe(true);
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.waitForTimeout(500);
  await aligned();
});
