import * as assert from "assert";

import { MotionLayer, Placement } from "../../hmiBuilder/display/motionLayer";

// Specification 39, §19.6 and §18 phase 5 item 4: the turning parts are drawn in a layer of their
// own above the page, placed where the page draws them, kept while they turn, refreshed when their
// drawing changes and given back when they stop. The browser's measure is stood in for here; the
// screen test `motion-layer.spec.mjs` measures in Chromium.

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

const PAGE =
  '<svg xmlns="http://www.w3.org/2000/svg" class="hmi-page" viewBox="0 0 400 300" width="400" height="300">' +
  '<g data-object="p1" transform="translate(100 50)"><g data-part="symbol" transform="scale(2)">' +
  '<g data-part="rotor" class="hmi-spin" style="animation-delay: -120ms"><circle cx="28" cy="32" r="22" fill="red"></circle></g>' +
  "</g></g></svg>";

/** The rotor: its box in the symbol, and the page's transform of it (scale 2, moved by 100, 50). */
const PLACEMENT: Placement = {
  box: { x: 6, y: 10, width: 44, height: 44 },
  matrix: { a: 2, b: 0, c: 0, d: 2, e: 100, f: 50 },
};

function surfaceWith(page: string): HTMLElement {
  const document = new JSDOM("<!doctype html><body></body>").window.document as Document;
  const surface = document.createElement("div");
  surface.innerHTML = page;
  document.body.append(surface);
  return surface;
}

const moving = (surface: HTMLElement) => surface.querySelector<SVGSVGElement>('[data-motion-layer] [data-motion="p1"]');
const rotor = (surface: HTMLElement) => surface.querySelector<SVGGElement>('[data-object="p1"] [data-part="rotor"]');

suite("HMI display: moving parts in a layer of their own", () => {
  test("a turning part gets an element of its own above the page, where the page draws it, with its phase", () => {
    const surface = surfaceWith(PAGE);
    new MotionLayer(surface, () => PLACEMENT).sync();
    const item = moving(surface);
    assert.ok(item, "an element of its own");
    assert.strictEqual(item.closest("svg.hmi-page"), null, "outside the page's drawing");
    assert.strictEqual(item.getAttribute("class"), "hmi-spin");
    assert.strictEqual(item.getAttribute("viewBox"), "6 10 44 44");
    // Centre (28, 32) at scale 2 moved by (100, 50) is (156, 114); its box is 88 wide.
    assert.deepStrictEqual(
      [item.style.left, item.style.top, item.style.width, item.style.height, item.style.animationDelay],
      ["112px", "70px", "88px", "88px", "-120ms"],
    );
    assert.ok(item.querySelector('circle[fill="red"]'), "with the part's drawing");
    assert.ok(rotor(surface)?.hasAttribute("data-moved"), "the page's own part is set aside");
  });

  test("the element stays while the part turns, takes a change of its drawing, and goes when it stops", () => {
    const surface = surfaceWith(PAGE);
    const layer = new MotionLayer(surface, () => PLACEMENT);
    layer.sync();
    const item = moving(surface);
    layer.sync();
    assert.strictEqual(moving(surface), item, "kept across updates, so it turns on");
    rotor(surface)?.querySelector("circle")?.setAttribute("fill", "blue");
    layer.sync();
    assert.strictEqual(moving(surface), item);
    assert.ok(item?.querySelector('circle[fill="blue"]'), "the changed drawing");
    rotor(surface)?.removeAttribute("class");
    layer.sync();
    assert.strictEqual(moving(surface), null, "stopped: no element");
    assert.strictEqual(rotor(surface)?.hasAttribute("data-moved"), false, "the page's own part is back");
  });

  test("a page drawn anew gets its turning parts lifted again", () => {
    const surface = surfaceWith(PAGE);
    const layer = new MotionLayer(surface, () => PLACEMENT);
    layer.sync();
    surface.innerHTML = PAGE;
    layer.sync();
    assert.ok(moving(surface), "lifted again");
    assert.strictEqual(surface.querySelectorAll("[data-motion-layer]").length, 1, "one layer");
  });

  test("resizing a nested SVG remeasures its lifted part without replacing the animation", () => {
    const nested = PAGE.replace(
      '<g data-object="p1"',
      '<svg data-placed="card" width="200" height="150" viewBox="0 0 400 300"><g data-object="p1"',
    ).replace("</g></g></svg>", "</g></g></svg></svg>");
    const surface = surfaceWith(nested);
    let measures = 0;
    const layer = new MotionLayer(surface, () => {
      measures++;
      return PLACEMENT;
    });
    layer.sync();
    const item = moving(surface);
    surface.querySelector('[data-placed="card"]')?.setAttribute("width", "400");
    layer.sync();
    assert.strictEqual(measures, 2, "nested viewport dimensions change the part's placement");
    assert.strictEqual(moving(surface), item, "the animation element survives resizing");
  });

  test("where the page cannot be measured, the parts turn in the page", () => {
    const surface = surfaceWith(PAGE);
    new MotionLayer(surface, () => undefined).sync();
    assert.strictEqual(moving(surface), null);
    assert.strictEqual(rotor(surface)?.hasAttribute("data-moved"), false);
  });
});
