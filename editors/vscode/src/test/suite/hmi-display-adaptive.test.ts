import * as assert from "assert";

import type { PageData } from "../../hmiBuilder/display/runtimeDisplay";
import { Host, object, openedWith, settle } from "./hmiDisplayHost";

// Specification 39, §4 item 5: on a page area smaller than the design size the runtime display
// shows the page at scale 1, laid out again at the area's width, and lays it out anew when the
// window changes size.

function data(): PageData {
  return {
    screen: { size: [1280, 720], theme: "dark" },
    pages: ["overview"],
    bundle: "v1",
    page: {
      name: "overview",
      model: {
        title: "Overview",
        object: [
          {
            id: "speed",
            type: "value",
            at: [40, 40],
            size: [200, 96],
            rotation: 0,
            label: "Speed",
            bind: { value: { path: "Main.P101.Speed", direction: "read" } },
          },
          { id: "box", type: "panel", at: [760, 40], size: [480, 120], rotation: 0, label: "Pump", bind: {} },
          {
            id: "start",
            type: "button",
            at: [776, 84],
            size: [120, 44],
            rotation: 0,
            text: "Start",
            bind: { target: { path: "Main.Start", direction: "write" } },
          },
        ],
        pipe: [],
      },
    },
    errors: [],
  };
}

function sized(element: Element, width: number, height: number): void {
  Object.defineProperty(element, "clientWidth", { configurable: true, value: width });
  Object.defineProperty(element, "clientHeight", { configurable: true, value: height });
}

suite("HMI display: adaptive layout", () => {
  test("a narrower page area lays the page out at its width, at scale 1, anew when the window changes size", async () => {
    const { host, root, window } = await openedWith(new Host(data()));
    const svg = (): Element => root.querySelector(".hmi-surface > svg") as Element;
    assert.strictEqual(
      svg().getAttribute("viewBox"),
      "0 0 1280 720",
      "a page area not yet measured shows the page as designed",
    );
    const surface = root.querySelector(".hmi-surface") as HTMLElement;
    sized(surface, 600, 700);
    window.dispatchEvent(new window.Event("resize"));
    await settle(host);
    assert.strictEqual(svg().getAttribute("viewBox"), "0 0 600 700");
    assert.ok(surface.classList.contains("hmi-scroll"), "shown at scale 1, to scroll");
    // Moved left to fit, the panel would overlap the value: it starts at the left margin below it.
    assert.strictEqual(object(root, "box").getAttribute("transform"), "translate(40 152)");
    assert.strictEqual(
      object(root, "start").getAttribute("transform"),
      "translate(56 196)",
      "its button moves with it",
    );
    sized(surface, 1280, 720);
    window.dispatchEvent(new window.Event("resize"));
    await settle(host);
    assert.strictEqual(svg().getAttribute("viewBox"), "0 0 1280 720", "the design size again");
    assert.ok(!surface.classList.contains("hmi-scroll"), "scaled to fit");
    assert.strictEqual(object(root, "box").getAttribute("transform"), "translate(760 40)");
  });
});
