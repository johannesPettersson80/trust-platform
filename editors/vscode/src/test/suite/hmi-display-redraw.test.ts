import * as assert from "assert";
const { JSDOM } = require("jsdom") as {
  JSDOM: new (html: string) => { window: { document: Document; close(): void } };
};
import { ValuePaintQueue } from "../../hmiBuilder/display/valuePaintQueue";
import type { HmiObjectModel } from "../../hmiBuilder/drawing/model";
import type { SvgNode } from "../../hmiBuilder/drawing/svg";

import {
  startDisplay,
  type DisplayHost,
  type DrawingApi,
  type PageData,
} from "../../hmiBuilder/display/runtimeDisplay";
import { drawing, Host, object, openedWith, settle } from "./hmiDisplayHost";

// Specification 39, §19.6: an update changes the drawn page in place, only where its drawing
// changed: a new value changes its text, and what does not change, a turning impeller included,
// keeps its nodes and its animation.

const PAGE: PageData = {
  screen: { size: [1280, 720], theme: "dark" },
  pages: ["overview"],
  bundle: "v1",
  page: {
    name: "overview",
    model: {
      title: "Overview",
      object: [
        {
          id: "p1",
          type: "pump",
          at: [40, 40],
          size: [128, 108],
          rotation: 0,
          bind: { running: { path: "Line.Running", direction: "read" } },
        },
        {
          id: "a",
          type: "value",
          at: [240, 40],
          size: [160, 48],
          rotation: 0,
          decimals: 1,
          bind: { value: { path: "Line.A", direction: "read" } },
        },
        {
          id: "b",
          type: "value",
          at: [240, 120],
          size: [160, 48],
          rotation: 0,
          decimals: 1,
          bind: { value: { path: "Line.B", direction: "read" } },
        },
      ],
      pipe: [],
    },
  },
  errors: [],
};

suite("HMI display: redraw", () => {
  test("fresh updates avoid redundant timer drawing while stalled values still age", async () => {
    const dom = new JSDOM('<!doctype html><div id="hmi"></div>');
    const root = dom.window.document.getElementById("hmi")!;
    const host = new Host(PAGE);
    let draws = 0;
    await startDisplay(root, host, {
      ...drawing,
      drawPage: (...args) => {
        draws++;
        return drawing.drawPage(...args);
      },
    });
    await settle(host);
    host.advance(750);
    host.sockets[0].send({ cycle: 1, period_ms: 250, values: { "Line.A": { value: 1 } } });
    await settle(host);
    const fresh = draws;
    host.advance(250);
    await settle(host);
    assert.strictEqual(draws, fresh, "a fresh page already drawn by the stream needs no timer redraw");
    host.advance(1000);
    await settle(host);
    assert.ok(draws > fresh, "the stalled stream still redraws on the timer");
    assert.strictEqual(object(root, "a").getAttribute("data-stale"), "true");
    dom.window.close();
  });
  test("the served project format reaches the rendered value without rewriting its model", async () => {
    const page = JSON.parse(JSON.stringify(PAGE)) as PageData;
    delete page.page!.model.object[1].decimals;
    Object.assign(page, { formats: { "line.a": { unit: "m", decimals: 2 } } });
    const { host, root } = await openedWith(new Host(page));
    host.sockets[0].send({ cycle: 1, values: { "Line.A": { value: 1.234 } } });
    await settle(host);
    const text = object(root, "a").textContent ?? "";
    assert.ok(text.includes("1.23") && !text.includes("1.234") && text.includes("m"), text);
    assert.strictEqual(page.page!.model.object[1].unit, undefined);
    assert.strictEqual(page.page!.model.object[1].decimals, undefined);
  });
  test("an update changes only what changed, in place", async () => {
    const { host, root } = await openedWith(new Host(PAGE));
    const stream = host.sockets[0];
    stream.send({ period_ms: 250 });
    stream.send({
      cycle: 1,
      values: { "Line.Running": { value: true }, "Line.A": { value: 1 }, "Line.B": { value: 2 } },
    });
    await settle(host);
    const [pump, a, b] = ["p1", "a", "b"].map((id) => object(root, id));
    const numberText = a.querySelector("tspan")!.firstChild;
    host.advance(250);
    stream.send({
      cycle: 2,
      values: { "Line.Running": { value: true }, "Line.A": { value: 5 }, "Line.B": { value: 2 } },
    });
    await settle(host);
    const text = a.querySelector("text");
    assert.strictEqual(object(root, "a"), a, "the changed value keeps its node");
    assert.strictEqual(a.querySelector("text"), text, "and its text node");
    assert.strictEqual(
      a.querySelector("tspan")!.firstChild,
      numberText,
      "the numeric text node is retained across updates",
    );
    assert.ok(a.textContent?.includes("5.0"), a.textContent ?? "");
    assert.strictEqual(object(root, "b"), b, "an unchanged value keeps its node");
    assert.strictEqual(object(root, "p1"), pump, "a turning pump keeps its node, so its animation runs on");
  });
});

// §11: changing the editor's source ends the old display even if a request is in flight.
suite("HMI display: lifetime", () => {
  const start = startDisplay as (
    root: HTMLElement,
    host: DisplayHost,
    api: DrawingApi,
    signal?: AbortSignal,
  ) => Promise<void>;
  test("ending a display closes its stream and cannot reconnect or repaint", async () => {
    const { root, window } = await openedWith(new Host(PAGE));
    root.replaceChildren();
    const host = new Host(PAGE);
    const lifetime = new AbortController();
    await start(root, host, drawing, lifetime.signal);
    await settle(host);
    const socket = host.sockets[0];
    lifetime.abort();
    const html = root.innerHTML;
    assert.ok(socket.closed, "ending Live mode closes the old stream");
    socket.send({ cycle: 9, values: { "Line.A": { value: 99 } } });
    socket.onclose?.();
    window.dispatchEvent(new window.Event("resize"));
    host.advance(5000);
    await settle(host);
    assert.strictEqual(host.sockets.length, 1, "an ended display never reconnects");
    assert.strictEqual(root.innerHTML, html, "late frames and values cannot repaint it");
  });

  test("ending while the page loads ignores its late response", async () => {
    const { root } = await openedWith(new Host(PAGE));
    root.replaceChildren();
    const host = new Host(PAGE);
    let reply!: (answer: { status: number; body: unknown }) => void;
    host.fetchJson = () =>
      new Promise((resolve) => {
        reply = resolve;
      });
    const lifetime = new AbortController();
    const loading = start(root, host, drawing, lifetime.signal);
    lifetime.abort();
    reply({ status: 200, body: PAGE });
    // A revived display would otherwise await its session request forever.
    host.fetchJson = async () => ({ status: 200, body: null });
    await loading;
    await settle(host);
    assert.strictEqual(root.children.length, 0, "a late page cannot revive an ended display");
    assert.strictEqual(host.sockets.length, 0);
  });
  test("ending a multilingual display removes its document input handlers", async () => {
    const { root } = await openedWith(new Host(PAGE));
    root.replaceChildren();
    const document = root.ownerDocument;
    const handlers = new Set<EventListenerOrEventListenerObject | null>();
    const add = document.addEventListener.bind(document);
    const remove = document.removeEventListener.bind(document);
    document.addEventListener = ((
      name: string,
      handler: EventListenerOrEventListenerObject | null,
      options?: boolean | AddEventListenerOptions,
    ) => {
      if (name === "click" || name === "keydown") handlers.add(handler);
      if (handler) add(name, handler, options);
    }) as typeof document.addEventListener;
    document.removeEventListener = ((
      name: string,
      handler: EventListenerOrEventListenerObject | null,
      options?: boolean | EventListenerOptions,
    ) => {
      if (name === "click" || name === "keydown") handlers.delete(handler);
      if (handler) remove(name, handler, options);
    }) as typeof document.removeEventListener;
    const lifetime = new AbortController();
    const host = new Host({
      ...PAGE,
      language: "en",
      languages: [
        { tag: "en", name: "English" },
        { tag: "sv", name: "Svenska" },
      ],
    });
    await start(root, host, drawing, lifetime.signal);
    assert.ok(handlers.size >= 2, "the popup and language controls listen while active");
    lifetime.abort();
    assert.strictEqual(handlers.size, 0, "page navigation leaves no document handlers behind");
  });
});

suite("HMI display: bounded value painting", () => {
  const model = (value: string): SvgNode => ({ tag: "text", attrs: {}, children: [], text: value });
  test("coalesces pending values from the actually painted state and limits each frame", () => {
    const dom = new JSDOM('<!doctype html><body><div id="a">old</div><div id="b">old</div></body>');
    const a = dom.window.document.getElementById("a")!,
      b = dom.window.document.getElementById("b")!;
    const frames: Array<() => void> = [],
      before: string[] = [];
    const queue = new ValuePaintQueue(
      (callback) => frames.push(callback),
      (element, previous, next) => {
        before.push(previous.text!);
        element.textContent = next.text!;
      },
      1,
    );
    queue.enqueue(a, model("old"), model("intermediate"));
    queue.enqueue(a, model("intermediate"), model("latest"));
    queue.enqueue(b, model("old"), model("next"));
    assert.strictEqual(frames.length, 1);
    frames.shift()!();
    assert.strictEqual(a.textContent, "latest");
    assert.strictEqual(b.textContent, "old");
    frames.shift()!();
    assert.strictEqual(b.textContent, "next");
    assert.deepStrictEqual(before, ["old", "old"]);
    dom.window.close();
  });
  test("urgent state changes supersede deferred values and disposed displays cannot paint", () => {
    const dom = new JSDOM('<!doctype html><body><div id="a">old</div></body>'),
      a = dom.window.document.getElementById("a")!;
    const frames: Array<() => void> = [];
    const queue = new ValuePaintQueue(
      (callback) => frames.push(callback),
      (element, _before, next) => {
        element.textContent = next.text!;
      },
    );
    queue.enqueue(a, model("old"), model("deferred"));
    queue.immediate(a, model("deferred"), model("unknown"));
    assert.strictEqual(a.textContent, "unknown");
    frames.shift()!();
    assert.strictEqual(a.textContent, "unknown");
    queue.enqueue(a, model("unknown"), model("late"));
    queue.dispose();
    frames.shift()!();
    assert.strictEqual(a.textContent, "unknown");
    dom.window.close();
  });
  test("a large page paints ordinary cards in bounded frames while preserving their nodes", async () => {
    const data = JSON.parse(JSON.stringify(PAGE)) as PageData;
    data.page!.model.object = Array.from({ length: 270 }, (_, index): HmiObjectModel => ({
      id: `v${index}`,
      type: "value",
      at: [32 + (index % 17) * 64, 40 + Math.floor(index / 17) * 40],
      size: [64, 32],
      rotation: 0,
      decimals: 1,
      bind: { value: { path: `Line.V${index}`, direction: "read" } },
    }));
    const { host, root } = await openedWith(new Host(data));
    const values = (value: number) =>
      Object.fromEntries(data.page!.model.object.map((_, index) => [`Line.V${index}`, { value }]));
    host.sockets[0].send({ cycle: 1, values: values(0) });
    await settle(host);
    const first = object(root, "v0");
    host.sockets[0].send({ cycle: 2, values: values(1) });
    host.paint();
    const changed = () =>
      [...root.querySelectorAll(".hmi-value")].filter((item) => item.textContent?.includes("1.0")).length;
    assert.strictEqual(changed(), 96, "the first bounded batch shares the rendering turn");
    host.paint();
    assert.strictEqual(changed(), 192);
    host.paint();
    assert.strictEqual(changed(), 270);
    assert.strictEqual(object(root, "v0"), first);
  });
});
