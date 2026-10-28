import * as assert from "assert";

import { drawPage } from "../../hmiBuilder/drawing";
import type {
  DrawFrame,
  HmiObjectModel,
  HmiPageModel,
  LiveValue,
  PlacedFaceplate,
} from "../../hmiBuilder/drawing/model";
import { findAll, renderToString, SvgNode, textsOf } from "../../hmiBuilder/drawing/svg";

// Specification 39, §8.9 item 3: a placed faceplate draws the faceplate's objects bound to its
// instance, as its popup does, under a slim header with the instance's tag and no close button;
// placed three times for three instances, each shows its own instance's values. Item 4: it keeps
// the faceplate's proportions, so it scales as one.

function card(id: string, instance: string, at: [number, number], size: [number, number]): HmiObjectModel {
  return { id, type: "faceplate", faceplate: "pump", instance, at, size, rotation: 0, bind: {} };
}

function faceplate(instance: string, title: string): PlacedFaceplate {
  return {
    model: {
      title,
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

const PAGE: HmiPageModel = {
  title: "Pumps",
  pipe: [],
  object: [
    card("card-1", "Main.P101", [40, 40], [400, 344]),
    card("card-2", "Main.P102", [460, 40], [400, 344]),
    card("card-3", "Main.P103", [880, 40], [200, 172]),
  ],
};

function confirmed(entries: Record<string, number>): Record<string, LiveValue> {
  return Object.fromEntries(Object.entries(entries).map(([path, value]) => [path, { value, state: "confirmed" }]));
}

const FRAME: DrawFrame = {
  size: [1280, 720],
  values: confirmed({ "Main.P101.ActualSpeed": 0, "Main.P102.ActualSpeed": 1460, "Main.P103.ActualSpeed": 735 }),
  placed: {
    "card-1": faceplate("Main.P101", "P101"),
    "card-2": faceplate("Main.P102", "P-102"),
    "card-3": faceplate("Main.P103", "P103"),
  },
};

function placement(svg: SvgNode, id: string): SvgNode {
  const found = findAll(svg, (node) => node.attrs["data-placed"] === id);
  assert.strictEqual(found.length, 1, renderToString(svg));
  return found[0];
}

suite("HMI drawing: placed faceplate", () => {
  test("a placed panel shows its labels, buttons and description in the display's language (§14.5 item 8)", () => {
    const svg = drawPage(PAGE, {
      ...FRAME,
      texts: { Speed: "Hastighet", Start: "Starta", "Transfer pump": "Överföringspump", P101: "nej" },
      language: "sv",
    });
    const shown = textsOf(placement(svg, "card-1"));
    for (const word of ["Hastighet", "Starta", "Överföringspump"])
      assert.ok(shown.includes(word), `${word} in ${shown}`);
    assert.ok(shown.includes("P101"), "the tag is a name and stays as written");
    assert.ok(!shown.includes("Speed"), shown.join(" | "));
  });

  test("placed three times for three instances, each shows its own instance's values under a slim header with its tag", () => {
    const svg = drawPage(PAGE, FRAME);
    for (const [id, tag, speed] of [
      ["card-1", "P101", "0"],
      ["card-2", "P-102", "1460"],
      ["card-3", "P103", "735"],
    ]) {
      const drawn = placement(svg, id);
      const header = findAll(drawn, (node) => node.attrs["data-part"] === "placed-header");
      assert.strictEqual(header.length, 1, id);
      assert.ok(textsOf(header[0]).includes(tag), `${id}: ${textsOf(header[0]).join(" | ")}`);
      assert.strictEqual(findAll(drawn, (node) => node.attrs["data-action"] === "close").length, 0, "no close button");
      const value = findAll(drawn, (node) => node.attrs["data-object"] === "speed");
      assert.strictEqual(value.length, 1, id);
      assert.ok(
        textsOf(value[0]).some((shown) => shown.startsWith(speed)),
        `${id}: ${textsOf(value[0]).join(" | ")}`,
      );
    }
  });

  test("a placement keeps the faceplate's proportions: the card scales as one", () => {
    const drawn = placement(drawPage(PAGE, FRAME), "card-3");
    assert.ok(String(drawn.attrs.viewBox).startsWith("0 0 400 "), String(drawn.attrs.viewBox));
    assert.strictEqual(drawn.attrs.preserveAspectRatio, "xMidYMid meet");
    assert.strictEqual(String(drawn.attrs.width), "200");
    assert.strictEqual(String(drawn.attrs.height), "172");
  });

  test("a placement whose faceplate the project lacks says so; one not known yet shows its card alone", () => {
    const lacking = findAll(
      drawPage(PAGE, { ...FRAME, placed: { "card-1": null } }),
      (node) => node.attrs["data-object"] === "card-1",
    )[0];
    assert.ok(textsOf(lacking).includes("No faceplate pump"), textsOf(lacking).join(" | "));
    const unknown = findAll(
      drawPage(PAGE, { ...FRAME, placed: {} }),
      (node) => node.attrs["data-object"] === "card-1",
    )[0];
    assert.ok(!textsOf(unknown).includes("No faceplate pump"), textsOf(unknown).join(" | "));
    assert.ok(textsOf(unknown).includes("P101"), `the header names the instance: ${textsOf(unknown).join(" | ")}`);
  });
});
