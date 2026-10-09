// The simulation badge of the display's top bar (specification 39, §8 item 8; specification 33,
// §6.1), against the tank-level example in simulation: the badge names the simulation, and shows
// the speed once the running simulation is set to another speed, live and without a reload. The
// speed is set back to 1 afterwards, so the other screen tests of this server see real time.
import { expect, test } from "@playwright/test";

const BADGE = 'header[data-part="top-bar"] [data-part="simulation"]';

/** Sets the running simulation's speed through the runtime's control API, as the IDE does. */
async function setSpeed(page, scale) {
  const answer = await page.request.post("/api/control", {
    data: { id: 1, type: "config.set", params: { "simulation.time_scale": scale } },
  });
  const body = await answer.json();
  expect(body.ok, JSON.stringify(body)).toBe(true);
}

test("the top bar names the running simulation and shows its speed when the speed changes", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto("/hmi/overview");
  const badge = page.locator(BADGE);
  await expect(badge).toBeVisible({ timeout: 30_000 });
  await expect(badge).toContainText("SIMULATION");
  try {
    await setSpeed(page, 10);
    await expect(badge).toContainText("×10", { timeout: 15_000 });
    await page.screenshot({ path: test.info().outputPath("simulation-badge.png") });
  } finally {
    await setSpeed(page, 1);
  }
  expect(errors).toEqual([]);
});
