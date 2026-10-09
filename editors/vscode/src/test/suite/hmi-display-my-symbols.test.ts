import * as assert from "assert";

import type { PageData } from "../../hmiBuilder/display/runtimeDisplay";
import { AGITATOR } from "./hmi-drawing-my-symbols.test";
import { click, Host, object, openedWith, settle } from "./hmiDisplayHost";

// Specification 39, §14.4 item 4: the runtime display draws the project's own symbols the page
// comes with, their parts driven by the live values.

const STIRRER = { ...AGITATOR, name: "stirrer" };
const MIXER = { ...AGITATOR, name: "mixer" };

function data(): PageData {
  return {
    screen: { size: [1280, 720], theme: "dark" },
    pages: ["mixing"],
    bundle: "v1",
    page: {
      name: "mixing",
      model: {
        title: "Mixing",
        object: [
          {
            id: "mixer",
            type: "my:stirrer",
            tag: "M-1",
            at: [40, 40],
            size: [128, 108],
            rotation: 0,
            bind: { running: { path: "Main.Running", direction: "read" } },
          },
        ],
        pipe: [],
      },
    },
    symbols: [STIRRER],
    errors: [],
  };
}

suite("HMI display: My Symbols", () => {
  test("the page's own symbol is drawn, and its rotor turns while the runtime says it runs", async () => {
    const { host, root } = await openedWith(new Host(data()));
    const mixer = object(root, "mixer");
    assert.ok(mixer.querySelector('[data-part="rotor"]'), `the symbol's drawing: ${mixer.outerHTML}`);
    host.sockets[0].send({ period_ms: 250 });
    host.sockets[0].send({ cycle: 1, values: { "Main.Running": { value: true } } });
    await settle(host);
    assert.strictEqual(object(root, "mixer").querySelector('[data-part="rotor"]')?.getAttribute("class"), "hmi-spin");
  });

  test("a popup's own symbols come with it and are drawn", async () => {
    const host = new Host({
      ...data(),
      page: {
        name: "mixing",
        model: {
          title: "Mixing",
          object: [
            {
              id: "open",
              type: "page-button",
              at: [40, 40],
              size: [160, 44],
              rotation: 0,
              text: "Mixer",
              bind: {},
              click: { popup: "mixer" },
            },
          ],
          pipe: [],
        },
      },
    });
    host.popups.mixer = {
      popup: {
        name: "mixer",
        model: {
          title: "Mixer",
          size: [400, 300],
          object: [{ id: "unit", type: "my:mixer", at: [24, 24], size: [128, 108], rotation: 0, bind: {} }],
          pipe: [],
        },
      },
      symbols: [MIXER],
      errors: [],
      bundle: "v1",
    };
    const { root, window } = await openedWith(host);
    click(window, object(root, "open"));
    await settle(host);
    const popup = root.querySelector('[data-part="popup"]');
    assert.ok(
      popup?.querySelector('[data-object="unit"] [data-part="rotor"]'),
      `the popup's symbol is drawn: ${popup?.outerHTML.slice(0, 400)}`,
    );
  });
});
