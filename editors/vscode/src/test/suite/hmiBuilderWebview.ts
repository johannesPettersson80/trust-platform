// The HMI Builder webview as it is shipped (media/hmiBuilderWebview.js), in a DOM, for the webview
// tests: mouse and keyboard events in, the messages it posts out.

import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import type { HmiObjectModel, HmiPageModel } from "../../hmiBuilder/drawing/model";
import type { Catalogue } from "../../hmiBuilder/editor/catalogue";
import type { HostToWebview, ScreenModel } from "../../hmiBuilder/editor/messages";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string, options: object) => { window: any } };

const BUNDLE = path.resolve(__dirname, "../../../media/hmiBuilderWebview.js");

/** The catalogue as the language server gives it: every component also takes a visibility
 * binding (§18 phase 5 item 2), added below. */
export const CATALOGUE: Catalogue = {
  format: 1,
  components: [
    {
      type: "value",
      category: "display",
      phase: 1,
      size: [160, 48],
      properties: { value: { accepts: ["value"], directions: ["read"] } },
    },
    {
      type: "gauge",
      category: "display",
      phase: 1,
      size: [160, 160],
      properties: { value: { accepts: ["number"], directions: ["read"] } },
    },
    {
      type: "bar",
      category: "display",
      phase: 1,
      size: [48, 160],
      variants: ["vertical", "horizontal"],
      properties: { value: { accepts: ["number"], directions: ["read"] } },
    },
    { type: "panel", category: "layout", phase: 1, size: [320, 240] },
    { type: "faceplate", category: "layout", phase: 5, size: [400, 344] },
    {
      type: "text",
      category: "display",
      phase: 1,
      size: [160, 32],
      variants: ["heading", "body", "caption"],
      properties: { text: { accepts: ["value"], directions: ["read"] } },
    },
    {
      type: "status",
      category: "display",
      phase: 1,
      size: [160, 48],
      properties: { state: { accepts: ["boolean", "enumeration", "number"], directions: ["read"] } },
    },
    {
      type: "switch",
      category: "control",
      phase: 1,
      size: [96, 48],
      properties: { value: { accepts: ["boolean"], directions: ["read-write", "write"] } },
    },
    {
      type: "selector",
      category: "control",
      phase: 1,
      size: [160, 40],
      properties: { value: { accepts: ["enumeration", "number"], directions: ["read-write", "write"] } },
    },
    {
      type: "button",
      category: "control",
      phase: 1,
      size: [120, 40],
      properties: { target: { accepts: ["boolean"], directions: ["write"] } },
    },
    {
      type: "pump",
      category: "equipment",
      group: "liquid",
      phase: 2,
      size: [120, 120],
      properties: {
        running: { accepts: ["boolean"], directions: ["read"] },
        fault: { accepts: ["boolean"], directions: ["read"] },
        manual: { accepts: ["boolean"], directions: ["read"] },
        "out-of-service": { accepts: ["boolean"], directions: ["read"] },
      },
      ports: ["inlet", "outlet"],
    },
    {
      type: "valve",
      category: "equipment",
      group: "liquid",
      phase: 2,
      size: [128, 108],
      properties: { open: { accepts: ["boolean"], directions: ["read"] } },
      ports: ["inlet", "outlet"],
    },
    {
      type: "junction",
      category: "equipment",
      group: "liquid",
      phase: 2,
      size: [16, 16],
      ports: ["left", "right", "top", "bottom"],
    },
    {
      type: "temperature-sensor",
      category: "equipment",
      group: "instruments",
      phase: 2,
      size: [176, 56],
      attaches: true,
      properties: {
        value: { accepts: ["number"], directions: ["read"] },
        fault: { accepts: ["boolean"], directions: ["read"] },
      },
    },
    {
      type: "trend",
      category: "display",
      phase: 1,
      size: [480, 240],
      properties: {
        "high-limit": { accepts: ["number"], directions: ["read"] },
        "low-limit": { accepts: ["number"], directions: ["read"] },
      },
      additional: { accepts: ["number", "boolean"], directions: ["read"] },
    },
  ],
};
for (const component of CATALOGUE.components) {
  component.properties = { ...component.properties, visible: { accepts: ["boolean"], directions: ["read"] } };
}

export const SCREEN: ScreenModel = { size: [1280, 720], theme: "dark" };

export function object(
  id: string,
  type: string,
  at: [number, number],
  size: [number, number],
  extra: Partial<HmiObjectModel> = {},
): HmiObjectModel {
  return { id, type, at, size, rotation: 0, bind: {}, ...extra };
}

export const PAGE: HmiPageModel = {
  title: "Overview",
  object: [
    object("speed", "value", [40, 40], [240, 96], {
      label: "Pump speed",
      bind: { value: { path: "Main.P101.Speed", direction: "read" } },
    }),
    object("level", "bar", [320, 40], [80, 240]),
  ],
  pipe: [],
};

export class Webview {
  readonly posted: Array<{ type: string; model?: HmiPageModel; [key: string]: unknown }> = [];
  readonly window: any;
  readonly document: any;

  constructor(view: "page" | "symbol" | "screen" = "page") {
    assert.ok(fs.existsSync(BUNDLE), "the webview bundle is built (npm run build:hmi-builder)");
    const dom = new JSDOM(
      '<!doctype html><html><body><div id="root" data-font-url="font.woff2" data-asset-base="https://assets.test"></div></body></html>',
      {
        runScripts: "outside-only",
        pretendToBeVisual: true,
        url: "https://webview.test/",
      },
    );
    this.window = dom.window;
    this.document = dom.window.document;
    this.window.acquireVsCodeApi = () => ({
      postMessage: (message: unknown) => this.posted.push(JSON.parse(JSON.stringify(message))),
      getState: () => undefined,
      setState: () => undefined,
    });
    this.document.getElementById("root").dataset.view = view;
    this.window.eval(fs.readFileSync(BUNDLE, "utf8"));
  }

  send(message: HostToWebview): void {
    this.window.dispatchEvent(new this.window.MessageEvent("message", { data: message }));
  }

  /** Waits for the webview's "ready", as the editor does before it sends anything. */
  async ready(): Promise<void> {
    await this.until("ready", () => this.posted.some((message) => message.type === "ready"));
  }

  async loaded(page: HmiPageModel = PAGE): Promise<void> {
    await this.ready();
    this.send({ type: "catalogue", catalogue: CATALOGUE });
    this.send({ type: "screen", screen: SCREEN });
    this.send({ type: "page", model: page });
    const first = page.object[0]?.id ?? "speed";
    await this.until("the page on the canvas", () => this.document.querySelector(`svg [data-object="${first}"]`));
  }

  query(selector: string): any {
    return this.document.querySelector(selector);
  }

  async until<T>(what: string, read: () => T | null | undefined | false, timeoutMs = 3000): Promise<T> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const value = read();
      if (value) return value;
      if (Date.now() > deadline)
        assert.fail(`timed out waiting for ${what}: ${this.document.body.innerHTML.slice(0, 2000)}`);
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
  }

  mouse(type: string, target: any, x: number, y: number, extra: object = {}): void {
    target.dispatchEvent(
      new this.window.MouseEvent(type, {
        bubbles: true,
        cancelable: true,
        clientX: x,
        clientY: y,
        button: 0,
        ...extra,
      }),
    );
  }

  /** A press, a move and a release, as a drag from one design point to another at zoom 1. */
  drag(target: any, from: [number, number], to: [number, number], extra: object = {}): void {
    this.mouse("mousedown", target, from[0], from[1], extra);
    this.mouse("mousemove", this.window, to[0], to[1], extra);
    this.mouse("mouseup", this.window, to[0], to[1], extra);
  }

  key(key: string, extra: object = {}): void {
    this.document.body.dispatchEvent(
      new this.window.KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...extra }),
    );
  }

  /** Picks an option of a select, as a user would. */
  choose(select: any, value: string): void {
    const setter = Object.getOwnPropertyDescriptor(this.window.HTMLSelectElement.prototype, "value")!.set!;
    setter.call(select, value);
    select.dispatchEvent(new this.window.Event("change", { bubbles: true }));
  }

  /** Ticks or unticks a checkbox, as a user would. */
  check(input: any, on: boolean): void {
    if (input.checked !== on) input.click();
  }

  /** Types into an input and commits with Enter, as a user would. */
  type(input: any, text: string): void {
    const setter = Object.getOwnPropertyDescriptor(this.window.HTMLInputElement.prototype, "value")!.set!;
    setter.call(input, text);
    input.dispatchEvent(new this.window.Event("input", { bubbles: true }));
    input.dispatchEvent(new this.window.KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
  }

  /** Types into an input and leaves it, as a user would: the browser follows the blur with a
   * focusout, which is what React's onBlur listens to. */
  leave(input: any, text: string): void {
    const setter = Object.getOwnPropertyDescriptor(this.window.HTMLInputElement.prototype, "value")!.set!;
    setter.call(input, text);
    input.dispatchEvent(new this.window.Event("input", { bubbles: true }));
    input.dispatchEvent(new this.window.FocusEvent("blur"));
    input.dispatchEvent(new this.window.FocusEvent("focusout", { bubbles: true }));
  }

  edits(): HmiPageModel[] {
    return this.posted.filter((message) => message.type === "edit").map((message) => message.model as HmiPageModel);
  }

  async nextEdit(after: number): Promise<HmiPageModel> {
    return this.until("an edit", () => this.edits()[after]);
  }

  layer(): any {
    return this.query('[data-part="editing-layer"]');
  }
}
