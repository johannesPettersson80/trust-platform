import * as assert from "assert";
import { drawPage } from "../../hmiBuilder/drawing";
import type { HmiObjectModel } from "../../hmiBuilder/drawing/model";
import { findAll, SvgNode } from "../../hmiBuilder/drawing/svg";

function draw(
  type: string,
  limits?: Record<string, number>,
  variant?: string,
  size: [number, number] = [240, 240],
): SvgNode {
  const object = {
    id: "signal",
    type,
    variant,
    at: [0, 0],
    size,
    rotation: 0,
    min: 0,
    max: 100,
    limits,
    bind: {
      value: { path: "Signal", direction: "read" },
      first: { path: "Signal", direction: "read" },
      second: { path: "Signal", direction: "read" },
    },
  } as HmiObjectModel;
  return drawPage(
    { title: "Limits", object: [object], pipe: [] },
    { size: [480, 320], values: { Signal: { value: 50, state: "confirmed" } } },
  );
}
const parts = (svg: SvgNode, name: string): SvgNode[] => findAll(svg, (node) => node.attrs["data-part"] === name);

suite("HMI drawing: alarm ranges", () => {
  test("a compact gauge never emits an invalid negative-radius range arc", () => {
    const band = parts(draw("gauge", { low: 10, high: 90 }, undefined, [48, 48]), "normal-range")[0];
    const radii = String(band.attrs.d).match(/ A ([\d.-]+) ([\d.-]+) /);
    assert.ok(radii && Number(radii[1]) > 0 && Number(radii[2]) > 0, String(band.attrs.d));
  });
  for (const [type, variant] of [
    ["bar", undefined],
    ["bar", "horizontal"],
    ["gauge", undefined],
    ["split-bar", undefined],
  ]) {
    test(`${type} ${variant ?? "default"} shows a quiet normal band and limit ticks`, () => {
      const svg = draw(type!, { "low-low": 5, low: 10, high: 90, "high-high": 95 }, variant);
      const bands = parts(svg, "normal-range");
      assert.strictEqual(bands.length, type === "split-bar" ? 2 : 1, "each scale has a visible normal band");
      for (const band of bands) {
        assert.strictEqual(Number(band.attrs["data-from"]), 0.1);
        assert.strictEqual(Number(band.attrs["data-to"]), 0.9);
      }
      const ticks = findAll(svg, (node) => node.attrs["data-limit"] !== undefined);
      assert.strictEqual(ticks.length, type === "split-bar" ? 8 : 4);
      for (const tick of ticks) {
        const name = tick.attrs["data-limit"];
        assert.strictEqual(
          tick.attrs.stroke,
          `var(--hmi-alarm-${name === "low-low" || name === "high-high" ? "high" : "medium"})`,
        );
        assert.strictEqual(
          Number(tick.attrs["data-fraction"]),
          ({ "low-low": 0.05, low: 0.1, high: 0.9, "high-high": 0.95 } as Record<string, number>)[name],
        );
      }
      if (type === "bar") {
        const track = parts(svg, "track")[0].attrs;
        for (const tick of ticks) {
          const fraction = Number(tick.attrs["data-fraction"]);
          if (variant === "horizontal")
            assert.ok(Math.abs(Number(tick.attrs.x1) - (Number(track.x) + Number(track.width) * fraction)) < 0.001);
          else
            assert.ok(
              Math.abs(Number(tick.attrs.y1) - (Number(track.y) + Number(track.height) * (1 - fraction))) < 0.001,
            );
        }
      }
    });
  }

  test("missing limits add no marks; outer-only limits bound the normal band and off-scale ticks stay hidden", () => {
    assert.strictEqual(parts(draw("gauge"), "normal-range").length, 0);
    const outer = draw("gauge", { "high-high": 95 });
    const band = parts(outer, "normal-range")[0];
    assert.ok(band, "the one known outer threshold bounds the normal band");
    assert.strictEqual(Number(band.attrs["data-from"]), 0);
    assert.strictEqual(Number(band.attrs["data-to"]), 0.95);
    const clipped = draw("bar", { "low-low": -10, low: 10, high: 110, "high-high": 120 });
    assert.deepStrictEqual(
      findAll(clipped, (node) => node.attrs["data-limit"] !== undefined).map((node) => node.attrs["data-limit"]),
      ["low"],
    );
  });
});
