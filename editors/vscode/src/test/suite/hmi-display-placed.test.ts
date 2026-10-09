import * as assert from "assert";

import type { PageData, PlacedFaceplateData } from "../../hmiBuilder/display/runtimeDisplay";
import { ANNA, click, commands, Host as DisplayTestHost, openedWith, settle } from "./hmiDisplayHost";

// Specification 39, §8.9: a faceplate placed on a page shows its instance's values from the
// page's stream, and its controls send their commands named by the placement, as its popup's do,
// locked by the roles of its own instance.

function placed(id: string, instance: string, roles: Record<string, string | null> = {}): PlacedFaceplateData {
  return {
    name: `faceplate:pumps/${id}`,
    roles,
    model: {
      title: instance.split(".").pop() ?? instance,
      description: "Transfer pump",
      size: [400, 300],
      pipe: [],
      object: [
        {
          id: "speed",
          type: "value",
          at: [24, 24],
          size: [200, 48],
          rotation: 0,
          label: "Speed",
          bind: { value: { path: `${instance}.ActualSpeed`, direction: "read" } },
        },
        {
          id: "start",
          type: "button",
          at: [24, 100],
          size: [120, 44],
          rotation: 0,
          text: "Start",
          action: "set",
          bind: { target: { path: `${instance}.StartCmd`, direction: "write" } },
        },
      ],
    },
  };
}

const PUMPS: PageData = {
  screen: { size: [1280, 720], theme: "dark" },
  pages: ["pumps"],
  bundle: "v1",
  page: {
    name: "pumps",
    model: {
      title: "Pumps",
      object: [
        {
          id: "card-1",
          type: "faceplate",
          faceplate: "pump",
          instance: "Main.P101",
          at: [40, 40],
          size: [400, 344],
          rotation: 0,
          bind: {},
        },
        {
          id: "card-2",
          type: "faceplate",
          faceplate: "pump",
          instance: "Main.P102",
          at: [460, 40],
          size: [400, 344],
          rotation: 0,
          bind: {},
        },
      ],
      pipe: [],
    },
  },
  faceplates: {
    "card-1": placed("card-1", "Main.P101", { start: "engineer" }),
    "card-2": placed("card-2", "Main.P102", { start: "operator" }),
  },
  errors: [],
};

function inside(root: HTMLElement, placement: string, id: string): Element {
  const found = root.querySelector(`[data-placed="${placement}"] [data-object="${id}"]`);
  assert.ok(found, `${placement} draws ${id}`);
  return found;
}

suite("HMI display: placed faceplates", () => {
  test("a placed faceplate's button sends its command named by the placement", async () => {
    const host = new DisplayTestHost(PUMPS);
    const { root, window } = await openedWith(host);
    host.reply("/api/hmi/command", { status: 200, body: { id: "x", outcome: "applied" } });
    click(window, inside(root, "card-2", "start"));
    await settle(host);
    const sent = commands(host);
    assert.strictEqual(sent.length, 1, JSON.stringify(sent));
    assert.deepStrictEqual(
      { ...sent[0], id: "?" },
      { id: "?", bundle: "v1", page: "faceplate:pumps/card-2", object: "start", action: "set" },
    );
    assert.strictEqual(inside(root, "card-2", "start").getAttribute("data-outcome"), "applied");
    assert.strictEqual(
      inside(root, "card-1", "start").getAttribute("data-outcome"),
      null,
      "the other placement's button has no outcome",
    );
  });

  test("a placed faceplate shows its instance's values from the page's stream", async () => {
    const host = new DisplayTestHost(PUMPS);
    const { root } = await openedWith(host);
    host.sockets[0].send({
      cycle: 1,
      values: { "Main.P101.ActualSpeed": { value: 0 }, "Main.P102.ActualSpeed": { value: 1460 } },
    });
    await settle(host);
    assert.ok(
      inside(root, "card-2", "speed").textContent?.includes("1460"),
      inside(root, "card-2", "speed").textContent ?? "",
    );
    assert.ok(
      !inside(root, "card-1", "speed").textContent?.includes("1460"),
      inside(root, "card-1", "speed").textContent ?? "",
    );
  });

  test("a placed faceplate's control is locked by its own instance's roles", async () => {
    const host = new DisplayTestHost(PUMPS);
    host.session = { status: 200, body: ANNA };
    const { root } = await openedWith(host);
    assert.strictEqual(inside(root, "card-1", "start").getAttribute("data-locked"), "engineer");
    assert.strictEqual(inside(root, "card-2", "start").getAttribute("data-locked"), null);
  });
});
