import * as assert from "assert";

import { drawPage } from "../../hmiBuilder/drawing";
import type { DrawFrame, HmiObjectModel, LiveValue } from "../../hmiBuilder/drawing/model";
import { findAll, renderToString, SvgNode, textsOf } from "../../hmiBuilder/drawing/svg";

// Specification 39, §5.1 and §5.2: text, image, panel, group, line and page button.

function draw(object: HmiObjectModel, values: Record<string, LiveValue> = {}, asset?: DrawFrame["asset"]): SvgNode {
  const svg = drawPage({ title: "Elements", object: [object], pipe: [] }, { size: [1280, 720], values, asset });
  const found = findAll(svg, (node) => node.attrs["data-object"] === object.id);
  assert.strictEqual(found.length, 1, renderToString(svg));
  return found[0];
}

function element(id: string, type: string, extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  return { id, type, at: [0, 0], size: [320, 48], rotation: 0, bind: {}, ...extra };
}

function sizeOf(node: SvgNode, content: string): number {
  const found = findAll(node, (child) => child.text === content)[0];
  assert.ok(found, renderToString(node));
  return Number(found.attrs["font-size"]);
}

suite("HMI drawing: page elements", () => {
  test("a text shows its words, or its bound value, in the size of its variant", () => {
    assert.ok(textsOf(draw(element("title", "text", { text: "Feed area" }))).includes("Feed area"));
    const bound = draw(element("name", "text", { bind: { text: { path: "Line.Recipe", direction: "read" } } }), {
      "Line.Recipe": { value: "Batch 42", state: "confirmed" },
    });
    assert.ok(textsOf(bound).includes("Batch 42"));
    const unknown = draw(element("name", "text", { bind: { text: { path: "Line.Recipe", direction: "read" } } }), {
      "Line.Recipe": { value: null, state: "unknown" },
    });
    assert.ok(textsOf(unknown).includes("?"));
    const heading = sizeOf(draw(element("h", "text", { text: "Overview", variant: "heading" })), "Overview");
    const body = sizeOf(draw(element("b", "text", { text: "Overview" })), "Overview");
    const caption = sizeOf(draw(element("c", "text", { text: "Overview", variant: "caption" })), "Overview");
    assert.ok(heading > body && body > caption, `${heading} > ${body} > ${caption}`);
    assert.ok(caption >= 14, "never below 14 px");
  });

  test("an image draws its project asset through the frame's resolver", () => {
    const node = draw(
      element("logo", "image", { image: "hmi/assets/logo.svg", size: [160, 80] }),
      {},
      (path) => `https://assets.example/${path}`,
    );
    const picture = findAll(node, (child) => child.tag === "image")[0];
    assert.ok(picture, renderToString(node));
    assert.strictEqual(picture.attrs.href, "https://assets.example/hmi/assets/logo.svg");
    assert.strictEqual(picture.attrs.preserveAspectRatio, "xMidYMid meet");
    const empty = draw(element("logo", "image", { size: [160, 80] }));
    assert.strictEqual(findAll(empty, (child) => child.tag === "image").length, 0);
    assert.ok(textsOf(empty).includes("No image"));
  });

  test("a panel is a rounded surface card with its title", () => {
    const node = draw(element("feed", "panel", { label: "Feed", size: [400, 300] }));
    const card = findAll(node, (child) => child.attrs["data-part"] === "card")[0];
    assert.ok(card && Number(card.attrs.rx) >= 12, renderToString(node));
    assert.strictEqual(card.attrs.fill, "var(--hmi-surface)");
    assert.ok(textsOf(node).includes("Feed"));
  });

  test("a group is a titled outline without a fill", () => {
    const node = draw(element("pumps", "group", { label: "Pumps", size: [400, 300] }));
    const outline = findAll(node, (child) => child.tag === "rect")[0];
    assert.ok(outline, renderToString(node));
    assert.strictEqual(outline.attrs.fill, "none");
    assert.ok(textsOf(node).includes("Pumps"));
  });

  test("a line runs straight across the middle of its box", () => {
    const node = draw(element("rule", "line", { size: [300, 2] }));
    const line = findAll(node, (child) => child.tag === "line")[0];
    assert.ok(line, renderToString(node));
    assert.deepStrictEqual([line.attrs.x1, line.attrs.y1, line.attrs.x2, line.attrs.y2], ["0", "1", "300", "1"]);
  });

  test("a page button shows its label and goes to its page", () => {
    const node = draw(
      element("to-trends", "page-button", { text: "Trends", size: [160, 44], click: { page: "trends" } }),
    );
    assert.ok(textsOf(node).includes("Trends"));
    assert.strictEqual(node.attrs["data-click"], "page:trends");
  });

  test("any object with a click names its target for the display to wire", () => {
    const node = draw(element("pump", "value", { click: { faceplate: "pump" } }));
    assert.strictEqual(node.attrs["data-click"], "faceplate:pump");
    const instanceFirst = draw(
      element("pump", "value", { click: { instance: "Main.P101", faceplate: "pump" } as HmiObjectModel["click"] }),
    );
    assert.strictEqual(
      instanceFirst.attrs["data-click"],
      "faceplate:pump",
      "the target, whatever the order of its keys",
    );
  });

  test("an object with a click takes it anywhere in its box, also where the symbol is not drawn", () => {
    const node = draw(element("fv-101", "control-valve", { size: [128, 108], click: { faceplate: "control-valve" } }));
    const hit = node.children.find((child) => child.attrs["data-part"] === "hit");
    assert.ok(hit, renderToString(node));
    assert.deepStrictEqual([hit.attrs.x, hit.attrs.y, hit.attrs.width, hit.attrs.height].map(Number), [0, 0, 128, 108]);
    assert.strictEqual(hit.attrs.fill, "transparent", "invisible, yet it takes the pointer");
    assert.strictEqual(node.children[0], hit, "under everything the object draws");
    assert.ok(
      !draw(element("plain", "value")).children.some((child) => child.attrs["data-part"] === "hit"),
      "only an object with a click",
    );
  });
  test("a layout panel lies behind the pipes that cross it, and the pipes behind the equipment (§6 item 8)", () => {
    const pump = { ...element("p", "pump"), at: [100, 100] as [number, number], size: [128, 108] as [number, number] };
    const tank = { ...element("t", "tank"), at: [400, 100] as [number, number], size: [128, 172] as [number, number] };
    const room = {
      ...element("room", "panel"),
      at: [50, 50] as [number, number],
      size: [600, 400] as [number, number],
    };
    const svg = drawPage(
      { title: "Layers", object: [pump, tank, room], pipe: [{ from: "p.outlet", to: "t.inlet" }] },
      { size: [1280, 720], values: {} },
    );
    const order: string[] = [];
    findAll(svg, (node) => {
      const tag = node.attrs["data-object"] ?? (node.attrs["data-pipe"] !== undefined ? "pipe" : undefined);
      if (tag && !order.includes(tag)) order.push(tag);
      return false;
    });
    assert.deepStrictEqual(order, ["room", "pipe", "p", "t"], "drawn back to front");
  });
});
