import * as assert from "assert";

import type { PageData } from "../../hmiBuilder/display/runtimeDisplay";
import { click, Host, openedWith } from "./hmiDisplayHost";

// Specification 39, §18 phase 5 item 6: a day and night switch in the display's top bar that
// operators may use, remembered by each panel, the project's theme being the default.

function page(theme: "dark" | "light"): PageData {
  return {
    screen: {
      size: [1280, 720],
      theme,
      top_bar: { title: "Plant", clock: true, alarms: false, user: false, page_title: true },
    },
    pages: ["overview"],
    navigation: [{ name: "overview", title: "Overview" }],
    bundle: "v1",
    page: { name: "overview", model: { title: "Overview", object: [], pipe: [] } },
    errors: [],
  };
}

suite("HMI display: day and night", () => {
  test("the top bar switches between day and night, the panel remembers it, and the project's theme is the default", async () => {
    const host = new Host(page("dark"));
    const { root, window } = await openedWith(host);
    assert.ok(root.classList.contains("hmi-theme-dark"), root.className);
    const toggle = root.querySelector('[data-part="top-bar"] [data-action="theme"]') as Element;
    assert.ok(toggle, "a switch in the top bar");
    assert.strictEqual(toggle.getAttribute("aria-label"), "Day", "it offers the other one");
    click(window, toggle);
    assert.ok(root.classList.contains("hmi-theme-light") && !root.classList.contains("hmi-theme-dark"), root.className);
    assert.strictEqual(toggle.getAttribute("aria-label"), "Night");
    const again = new Host(page("dark"));
    again.stored = { ...host.stored };
    const reopened = await openedWith(again);
    assert.ok(reopened.root.classList.contains("hmi-theme-light"), "remembered by the panel");
    const elsewhere = await openedWith(new Host(page("light")));
    assert.ok(elsewhere.root.classList.contains("hmi-theme-light"), "the project's theme without a choice");
  });
});
