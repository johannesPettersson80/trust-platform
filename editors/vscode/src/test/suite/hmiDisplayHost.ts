// Test helpers for the runtime display: a host with a recording fetch, post and stream, and the
// display opened on a page in jsdom.

import * as assert from "assert";

import { adaptPage } from "../../hmiBuilder/drawing/adaptive";
import { priorityMarkImage } from "../../hmiBuilder/drawing/alarms";
import { drawPage } from "../../hmiBuilder/drawing";
import { renderToString } from "../../hmiBuilder/drawing/svg";
import { registerSymbols } from "../../hmiBuilder/drawing/symbols";
import { tokensCss } from "../../hmiBuilder/drawing/tokens";
import { DisplayHost, PageData, SocketLike, startDisplay } from "../../hmiBuilder/display/runtimeDisplay";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

/** An Operator's session as the runtime answers it, with what it may do (§12.4 item 4). */
export const ANNA = {
  ok: true,
  user: "anna",
  role: "operator",
  acts_as: ["viewer", "operator"],
  may_change_settings: false,
  session: "s1",
  idle_seconds: 900,
  expires_seconds: 43200,
};

/** An Engineer's session as the runtime answers it. */
export const ERIK = {
  ...ANNA,
  user: "erik",
  role: "engineer",
  acts_as: ["viewer", "operator", "engineer"],
  may_change_settings: true,
  session: "s2",
};

export type Answer = { status: number; body: unknown } | "fail";

export class FakeSocket implements SocketLike {
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;
  closed = false;
  close(): void {
    this.closed = true;
  }
  send(message: unknown): void {
    this.onmessage?.({ data: JSON.stringify(message) });
  }
}

export class Host implements DisplayHost {
  constructor(private readonly data: PageData) {}
  location = { pathname: "/hmi/operate", host: "plc.local:8080", protocol: "http:" };
  posts: Array<{ url: string; body: any }> = [];
  sockets: FakeSocket[] = [];
  session: { status: number; body: unknown } = { status: 200, body: ANNA };
  replies: Record<string, Answer[]> = {};
  reloads = 0;
  private frames: Array<() => void> = [];
  /** Popup data by name, for `/api/hmi/popup`. */
  popups: Record<string, unknown> = {};
  /** The addresses the page went to. */
  navigations: string[] = [];
  /** The address of each stream opened. */
  socketUrls: string[] = [];
  /** What `/api/hmi/history` answers: the historian's samples for trends. */
  historyBody: unknown = { enabled: false, samples: {} };
  /** The runtime's answer to a trend's export (`/api/hmi/history/export`). */
  exportBody: unknown = { name: "trend.csv", csv: "time\n" };
  /** What `/api/hmi/alarms` answers: the alarm list and the records the runtime keeps. */
  alarmsBody: unknown = { alarms: null, events: [] };
  /** The addresses fetched, in order. */
  fetched: string[] = [];
  /** What the panel's browser keeps, such as its day or night choice (§18 phase 5 item 6). */
  stored: Record<string, string> = {};
  remember(key: string, value: string): void {
    this.stored[key] = value;
  }
  recall(key: string): string | undefined {
    return this.stored[key];
  }
  /** The files the display gave to save, by name (§5.9 item 1). */
  downloads: Array<{ name: string; text: string }> = [];
  download(name: string, text: string): void {
    this.downloads.push({ name, text });
  }
  /** The alarm tones started, and whether a repeating one sounds (§5.9 item 2). */
  played: string[] = [];
  sounding = false;
  sounds = {
    play: (pattern: "high" | "medium"): void => {
      this.played.push(pattern);
      if (pattern === "high") this.sounding = true;
    },
    stop: (): void => {
      this.sounding = false;
    },
  };
  async fetchJson(url: string): Promise<{ status: number; body: unknown }> {
    this.fetched.push(url);
    if (url.startsWith("/api/hmi/session")) return this.session;
    if (url.startsWith("/api/hmi/alarms")) return { status: 200, body: this.alarmsBody };
    if (url.startsWith("/api/hmi/history/export")) return { status: 200, body: this.exportBody };
    if (url.startsWith("/api/hmi/history")) return { status: 200, body: this.historyBody };
    if (url.startsWith("/api/hmi/popup")) {
      // A popup by its name, a faceplate by the object that opens it (§8.7).
      const name = url.includes("faceplate=")
        ? `faceplate:${decodeURIComponent(url.split("faceplate=")[1] ?? "")}`
        : decodeURIComponent(url.split("name=")[1] ?? "");
      return name in this.popups
        ? { status: 200, body: this.popups[name] }
        : { status: 404, body: { error: "no popup" } };
    }
    return { status: 200, body: this.data };
  }
  navigate(url: string): void {
    this.navigations.push(url);
  }
  async postJson(url: string, body: unknown): Promise<{ status: number; body: unknown }> {
    this.posts.push({ url, body });
    const reply = this.replies[url]?.shift();
    if (reply === "fail") throw new Error("the connection was lost");
    return reply ?? { status: 500, body: {} };
  }
  openSocket(url: string): SocketLike {
    this.socketUrls.push(url);
    const socket = new FakeSocket();
    this.sockets.push(socket);
    return socket;
  }
  /** The fake clock, in ms; `advance` moves it and runs the delayed callbacks that come due. */
  clock = 0;
  private timers: Array<{ at: number; callback: () => void }> = [];
  now(): number {
    return this.clock;
  }
  schedule(callback: () => void): void {
    this.frames.push(callback);
  }
  later(milliseconds: number, callback: () => void): void {
    this.timers.push({ at: this.clock + milliseconds, callback });
  }
  advance(milliseconds: number): void {
    const until = this.clock + milliseconds;
    for (;;) {
      const due = this.timers.filter((timer) => timer.at <= until).sort((a, b) => a.at - b.at)[0];
      if (!due) break;
      this.timers.splice(this.timers.indexOf(due), 1);
      this.clock = due.at;
      due.callback();
    }
    this.clock = until;
  }
  reload(): void {
    this.reloads += 1;
  }
  paint(): void {
    const frames = this.frames;
    this.frames = [];
    frames.forEach((frame) => frame());
  }
  reply(url: string, ...answers: Answer[]): void {
    this.replies[url] = [...(this.replies[url] ?? []), ...answers];
  }
}

export const drawing = { adaptPage, drawPage, registerSymbols, renderToString, tokensCss, priorityMarkImage };

/** Lets promises settle, then paints. */
export async function settle(host: Host): Promise<void> {
  for (let round = 0; round < 5; round += 1) {
    await new Promise((resolve) => setTimeout(resolve, 0));
    host.paint();
  }
}

export async function openedWith(
  host: Host,
  signal?: AbortSignal,
): Promise<{ host: Host; root: HTMLElement; window: any }> {
  const dom = new JSDOM('<!doctype html><div id="hmi"></div>');
  const root = dom.window.document.getElementById("hmi");
  await startDisplay(root, host, drawing, signal);
  await settle(host);
  return { host, root, window: dom.window };
}

export function object(root: HTMLElement, id: string): Element {
  const found = root.querySelector(`[data-object="${id}"]`);
  assert.ok(found, `the object ${id} is drawn`);
  return found;
}

export function click(window: any, target: Element): void {
  target.dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
}

export function fill(window: any, input: HTMLInputElement, value: string): void {
  input.value = value;
  input.dispatchEvent(new window.Event("input", { bubbles: true }));
}

export function submit(window: any, form: Element): void {
  form.dispatchEvent(new window.Event("submit", { bubbles: true, cancelable: true }));
}

export function commands(host: Host): any[] {
  return host.posts.filter((post) => post.url === "/api/hmi/command").map((post) => post.body);
}
