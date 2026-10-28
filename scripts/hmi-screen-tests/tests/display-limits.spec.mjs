import { expect, test } from "@playwright/test";

for (const theme of ["dark", "light"]) {
  test(`display limits share the scale in ${theme}`, async ({ page }, testInfo) => {
    const errors = [];
    page.on("pageerror", (error) => errors.push(String(error)));
    await page.addInitScript((theme) => localStorage.setItem("trust-hmi-theme", theme), theme);
    await page.goto("/hmi/display-limits");
    await expect(page.locator('[data-object="bar"]')).toHaveAttribute("data-state", "confirmed");
    const answer = await (await page.request.get("/api/hmi/page?name=display-limits")).json();
    expect(answer.formats["tank.level"]).toEqual({ unit: "m", decimals: 2 });
    for (const id of ["gauge", "horizontal", "reading"]) {
      const authored = answer.page.model.object.find((object) => object.id === id);
      expect(authored.unit).toBeUndefined();
      expect(authored.decimals).toBeUndefined();
      await expect(page.locator(`[data-object="${id}"]`)).toContainText(/\d+\.\d{2}m/);
    }
    for (const id of ["bar", "gauge", "horizontal", "split"]) {
      const object = page.locator(`[data-object="${id}"]`);
      await expect(object.locator('[data-part="normal-range"]')).toHaveCount(id === "split" ? 2 : 1);
      await expect(object.locator('[data-limit]')).toHaveCount(id === "split" ? 8 : 4);
      // One evaluation finds the marks and measures them, so a redraw between the two cannot leave
      // detached marks, which measure as empty.
      const marks = await page.evaluate((selector) => [...document.querySelectorAll(selector)].map((element) => {
        const box = element.getBBox();
        return { width: box.width, height: box.height, title: element.textContent };
      }), `[data-object="${id}"] [data-limit]`);
      expect(marks).toHaveLength(id === "split" ? 8 : 4);
      expect(marks.every((mark) => mark.width > 0 || mark.height > 0)).toBe(true);
      expect(marks.every((mark) => /[LH]/.test(mark.title))).toBe(true);
    }
    await page.screenshot({ path: testInfo.outputPath(`display-limits-${theme}.png`) });
    expect(errors).toEqual([]);
  });
}
