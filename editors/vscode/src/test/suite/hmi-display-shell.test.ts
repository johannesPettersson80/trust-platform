import { adaptPage } from "../../hmiBuilder/drawing/adaptive";
import { priorityMarkImage } from "../../hmiBuilder/drawing/alarms";
import * as assert from "assert";

import { drawPage } from "../../hmiBuilder/drawing";
import { renderToString } from "../../hmiBuilder/drawing/svg";
import { registerSymbols } from "../../hmiBuilder/drawing/symbols";
import { tokensCss } from "../../hmiBuilder/drawing/tokens";
import { DisplayHost, PageData, SocketLike, startDisplay } from "../../hmiBuilder/display/runtimeDisplay";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

// Specification 39, §11.1: the runtime display in the browser.

const OVERVIEW: PageData = {
  screen: { size: [1280, 720], theme: "dark", colors: { dark: { accent: "#ff8800" } } },
  pages: ["overview", "trends"],
  page: {
    name: "overview",
    model: {
      title: "Overview",
      object: [
        {
          id: "speed",
          type: "value",
          at: [40, 40],
          size: [240, 96],
          rotation: 0,
          label: "Pump speed",
          bind: { value: { path: "Main.Speed", direction: "read" } },
        },
      ],
      pipe: [],
    },
  },
  errors: [],
};

class FakeSocket implements SocketLike {
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;
  closed = false;
  constructor(readonly url: string) {}
  close(): void {
    this.closed = true;
  }
  send(message: unknown): void {
    this.onmessage?.({ data: JSON.stringify(message) });
  }
}

class FakeHost implements DisplayHost {
  location: { pathname: string; host: string; protocol: string; search: string };
  reloads = 0;
  stored = new Map<string, string>();
  fetched: string[] = [];
  sockets: FakeSocket[] = [];
  clock = 0;
  private frames: Array<() => void> = [];
  private timers: Array<{ at: number; callback: () => void }> = [];

  constructor(
    pathname: string,
    private readonly answer: { status: number; body: unknown },
    protocol = "http:",
    search = "",
  ) {
    this.location = { pathname, host: "plc.local:8080", protocol, search };
  }
  remember(key: string, value: string): void {
    this.stored.set(key, value);
  }
  recall(key: string): string | undefined {
    return this.stored.get(key);
  }
  async fetchJson(url: string): Promise<{ status: number; body: unknown }> {
    this.fetched.push(url);
    return this.answer;
  }
  async postJson(): Promise<{ status: number; body: unknown }> {
    return { status: 500, body: {} };
  }
  reload(): void {
    this.reloads += 1;
  }
  navigate(): void {}
  openSocket(url: string): SocketLike {
    const socket = new FakeSocket(url);
    this.sockets.push(socket);
    return socket;
  }
  now(): number {
    return this.clock;
  }
  schedule(callback: () => void): void {
    this.frames.push(callback);
  }
  later(milliseconds: number, callback: () => void): void {
    this.timers.push({ at: this.clock + milliseconds, callback });
  }
  /** Moves the clock on, running the timers that come due and then the pending paint. */
  advance(milliseconds: number): void {
    this.clock += milliseconds;
    const due = this.timers.filter((timer) => timer.at <= this.clock);
    this.timers = this.timers.filter((timer) => timer.at > this.clock);
    due.forEach((timer) => timer.callback());
    this.paint();
  }
  paint(): void {
    const frames = this.frames;
    this.frames = [];
    frames.forEach((frame) => frame());
  }
}

const drawing = { adaptPage, drawPage, registerSymbols, renderToString, tokensCss, priorityMarkImage };

function socketOf(host: FakeHost, index = 0): FakeSocket {
  const socket = host.sockets[index];
  assert.ok(socket, `stream ${index + 1} is open`);
  return socket;
}

function container(): HTMLElement {
  return new JSDOM('<!doctype html><div id="hmi"></div>').window.document.getElementById("hmi");
}

/** A screen whose top bar shows the title only: the simulation badge is the only other part of it. */
const WITH_BAR: PageData = {
  ...OVERVIEW,
  screen: {
    size: [1280, 720],
    theme: "dark",
    top_bar: { title: "Line 2", clock: false, alarms: false, user: false, page_title: false },
  },
};

/** The simulation badge, in the top bar or in the corner of a screen without one. */
function badgeOf(element: HTMLElement): HTMLElement {
  const badge = element.querySelector('[data-part="simulation"]') as HTMLElement | null;
  assert.ok(badge, element.innerHTML);
  return badge;
}

/** The part of the badge that holds the speed after its label. */
function speedOf(badge: HTMLElement): HTMLElement {
  const speed = badge.querySelector('[data-part="simulation-speed"]') as HTMLElement | null;
  assert.ok(speed, badge.outerHTML);
  return speed;
}

suite("HMI display: browser shell", () => {
  test("the page named in the address is drawn with its screen's theme, colours and the bundled font", async () => {
    const host = new FakeHost("/hmi/overview", { status: 200, body: OVERVIEW });
    const element = container();
    await startDisplay(element, host, drawing);
    host.paint();
    assert.deepStrictEqual(
      host.fetched,
      ["/api/hmi/page?name=overview", "/api/hmi/session"],
      "the page, then the session (§12.4)",
    );
    assert.ok(element.classList.contains("hmi-theme-dark"), element.className);
    const style = element.querySelector("style")?.textContent ?? "";
    assert.match(style, /--hmi-accent: #ff8800;/);
    assert.match(style, /url\("\/hmi\/assets\/fonts\/InterVariable\.woff2"\)/);
    assert.ok(element.querySelector('svg [data-object="speed"]'), element.innerHTML);
    assert.deepStrictEqual(
      host.sockets.map((socket) => socket.url),
      ["ws://plc.local:8080/ws/hmi?page=overview"],
    );
  });

  test("a screen in several languages has a language button that lists them by their own names, and one click switches (§14.5)", async () => {
    const languages: PageData = {
      ...OVERVIEW,
      language: "en",
      languages: [
        { tag: "en", name: "English" },
        { tag: "sv", name: "Svenska" },
      ],
      texts: {},
    };
    const host = new FakeHost("/hmi/overview", { status: 200, body: languages });
    const element = container();
    await startDisplay(element, host, drawing);
    const button = element.querySelector('[data-action="language"]') as HTMLElement | null;
    assert.ok(button, element.innerHTML);
    assert.ok(button.textContent?.includes("English"), button.textContent ?? "");
    button.click();
    const names = [...element.querySelectorAll("[data-language]")].map((item) => item.textContent);
    assert.deepStrictEqual(names, ["English", "Svenska"]);
    // The open menu lies on the top layer, outside the frame, so nothing floating covers it.
    assert.strictEqual(
      element.querySelector(".hmi-frame [data-part='language-menu']"),
      null,
      "the menu is not inside the frame",
    );
    assert.ok(element.querySelector("[data-part='language-menu']"), element.innerHTML);
    (element.querySelector('[data-language="sv"]') as HTMLElement).click();
    assert.strictEqual(host.stored.get("trust-hmi-language"), "sv", "this browser remembers the choice");
    assert.strictEqual(host.reloads, 1, "the page comes again in the language chosen");
  });

  test("the page comes in the language the address names, else in the one this browser chose (§14.5)", async () => {
    const pinned = new FakeHost("/hmi/overview", { status: 200, body: OVERVIEW }, "http:", "?lang=sv");
    await startDisplay(container(), pinned, drawing);
    assert.strictEqual(pinned.fetched[0], "/api/hmi/page?name=overview&lang=sv");
    const chosen = new FakeHost("/hmi/overview", { status: 200, body: OVERVIEW });
    chosen.stored.set("trust-hmi-language", "de");
    await startDisplay(container(), chosen, drawing);
    assert.strictEqual(chosen.fetched[0], "/api/hmi/page?name=overview&lang=de");
  });

  test("the display's own words and the page's come from the screen's text table (§14.5)", async () => {
    const swedish: PageData = {
      ...OVERVIEW,
      screen: {
        size: [1280, 720],
        theme: "dark",
        top_bar: { title: "Linje 2", clock: false, alarms: true, user: true, page_title: false },
      },
      language: "sv",
      languages: [
        { tag: "en", name: "English" },
        { tag: "sv", name: "Svenska" },
      ],
      texts: { "No alarms": "Inga larm", "Log in": "Logga in", "Pump speed": "Pumpvarvtal" },
    };
    const host = new FakeHost("/hmi/overview", { status: 200, body: swedish });
    const element = container();
    await startDisplay(element, host, drawing);
    host.paint();
    assert.strictEqual(element.querySelector('[data-part="alarm-count"]')?.textContent, "Inga larm");
    assert.ok(
      element.querySelector("svg.hmi-page")?.textContent?.includes("Pumpvarvtal"),
      element.querySelector("svg.hmi-page")?.outerHTML,
    );
    assert.ok(
      [...element.querySelectorAll("button")].some((item) => item.textContent === "Logga in"),
      element.innerHTML,
    );
    assert.strictEqual(
      element.ownerDocument.documentElement.getAttribute("lang"),
      "sv",
      "the page speaks its language, for the forms of its font",
    );
  });

  test("with a top bar that does not show the user, the session still sits at the top bar's right end (§4.8 item 8)", async () => {
    // Catches the session floating over the page's top edge under a top bar without `user`, as it did on the
    // memory marker counter's /hmi (#134, 29 October 2026).
    const barred: PageData = {
      ...OVERVIEW,
      screen: {
        size: [1280, 720],
        theme: "dark",
        top_bar: { title: "Line 2", clock: true, alarms: false, user: false, page_title: false },
      },
    };
    const element = container();
    await startDisplay(element, new FakeHost("/hmi/overview", { status: 200, body: barred }), drawing);
    const bar = element.querySelector('header[data-part="top-bar"]');
    assert.ok(bar, element.innerHTML);
    const session = element.querySelector('[data-part="session"]');
    assert.ok(session?.classList.contains("hmi-session-inline"), session?.className);
    assert.strictEqual(session?.parentElement, bar.lastElementChild, "the session's slot ends the top bar");
  });

  test("a screen in one language shows no language button", async () => {
    const element = container();
    await startDisplay(element, new FakeHost("/hmi/overview", { status: 200, body: OVERVIEW }), drawing);
    assert.strictEqual(element.querySelector('[data-action="language"]'), null);
  });

  test("the simulation badge stays hidden without a simulation, then names it and its speed as the stream reports them (§8 item 8)", async () => {
    const host = new FakeHost("/hmi/overview", { status: 200, body: WITH_BAR });
    const element = container();
    await startDisplay(element, host, drawing);
    const badge = badgeOf(element);
    assert.ok(badge.closest('header[data-part="top-bar"]'), "the badge stands in the top bar");
    assert.strictEqual(badge.hidden, true, "no simulation, no badge");
    const report = (status: unknown): void => socketOf(host).send({ simulation: status });
    report({ enabled: false, speed_configured: 10, speed_achieved: null });
    assert.strictEqual(badge.hidden, true, "a stopped simulation shows no badge");
    report({ enabled: true, speed_configured: null, speed_achieved: null });
    assert.strictEqual(badge.hidden, false, "a simulation at speed 1 shows the badge");
    assert.strictEqual(badge.firstElementChild?.textContent, "SIMULATION");
    assert.strictEqual(speedOf(badge).hidden, true, "at speed 1 the runtime gives no speed, so none is shown");
    report({ enabled: true, speed_configured: 10, speed_achieved: null });
    assert.strictEqual(speedOf(badge).hidden, false);
    assert.strictEqual(
      speedOf(badge).textContent,
      "×10",
      "the configured speed alone when the runtime gives no achieved speed",
    );
    assert.strictEqual(badge.textContent, "SIMULATION ×10", "the label, a space, then the speed");
    report({ enabled: true, speed_configured: 10, speed_achieved: "6.8" });
    assert.strictEqual(
      speedOf(badge).textContent,
      "×6.8 of ×10",
      "behind, the achieved speed is shown as the runtime words it, with the phrase translated",
    );
    assert.strictEqual(badge.textContent, "SIMULATION ×6.8 of ×10");
    report({ enabled: true, speed_configured: 5, speed_achieved: "4.6" });
    assert.strictEqual(speedOf(badge).textContent, "×4.6 of ×5", "the words are shown as the runtime gives them");
    report({ enabled: true, speed_configured: 10, speed_achieved: "9.6" });
    assert.strictEqual(
      speedOf(badge).textContent,
      "×9.6 of ×10",
      "the page keeps no speed rule of its own: it shows the achieved speed it is given",
    );
    report({ enabled: false, speed_configured: 10, speed_achieved: "6.8" });
    assert.strictEqual(badge.hidden, true, "back to no simulation, no badge");
  });

  test("the simulation badge's label and the 'of' pattern come from the screen's text table (§14.5)", async () => {
    const swedish: PageData = {
      ...WITH_BAR,
      language: "sv",
      languages: [
        { tag: "en", name: "English" },
        { tag: "sv", name: "Svenska" },
      ],
      texts: { SIMULATION: "SIMULERING", "{achieved} of {configured}": "{achieved} av {configured}" },
    };
    const host = new FakeHost("/hmi/overview", { status: 200, body: swedish });
    const element = container();
    await startDisplay(element, host, drawing);
    const badge = badgeOf(element);
    assert.strictEqual(badge.firstElementChild?.textContent, "SIMULERING");
    socketOf(host).send({ simulation: { enabled: true, speed_configured: 10, speed_achieved: "6.8" } });
    assert.strictEqual(speedOf(badge).textContent, "×6.8 av ×10");
  });

  test("a page without a top bar shows the simulation badge in the corner, beside the language button (§8 item 8)", async () => {
    const bare: PageData = {
      ...OVERVIEW,
      language: "en",
      languages: [
        { tag: "en", name: "English" },
        { tag: "sv", name: "Svenska" },
      ],
      texts: {},
    };
    const host = new FakeHost("/hmi/overview", { status: 200, body: bare });
    const element = container();
    await startDisplay(element, host, drawing);
    const badge = badgeOf(element);
    assert.strictEqual(badge.closest("header"), null, "there is no top bar");
    const corner = badge.parentElement as HTMLElement;
    assert.ok(corner.classList.contains("hmi-corner"), corner.outerHTML);
    assert.ok(corner.querySelector('[data-action="language"]'), "the language button stands in the same corner");
    socketOf(host).send({ simulation: { enabled: true, speed_configured: 10, speed_achieved: null } });
    assert.strictEqual(badge.hidden, false, "the badge shows without a top bar too");
    assert.strictEqual(speedOf(badge).textContent, "×10");
    const single = container();
    await startDisplay(single, new FakeHost("/hmi/overview", { status: 200, body: OVERVIEW }), drawing);
    assert.ok(badgeOf(single).parentElement?.classList.contains("hmi-corner"), single.innerHTML);
  });

  test("the simulation badge takes its own place in the top bar, covers nothing there, and uses the accent-3 token (§8 item 8)", async () => {
    const element = container();
    await startDisplay(element, new FakeHost("/hmi/overview", { status: 200, body: WITH_BAR }), drawing);
    const style = element.querySelector("style")?.textContent ?? "";
    assert.match(
      style,
      /\.hmi-simulation \{[^}]*flex: none;[^}]*var\(--hmi-accent-3\)/,
      "in flow, in the accent-3 colour",
    );
    assert.doesNotMatch(style, /\.hmi-simulation \{[^}]*position: (absolute|fixed)/, "nothing is laid over the bar");
  });

  test("without a page name the first page is shown, and https pages use a secure socket", async () => {
    const host = new FakeHost("/hmi", { status: 200, body: OVERVIEW }, "https:");
    await startDisplay(container(), host, drawing);
    assert.deepStrictEqual(host.fetched, ["/api/hmi/page", "/api/hmi/session"]);
    assert.deepStrictEqual(
      host.sockets.map((socket) => socket.url),
      ["wss://plc.local:8080/ws/hmi?page=overview"],
    );
  });

  test("values from the stream redraw the page", async () => {
    const host = new FakeHost("/hmi/overview", { status: 200, body: OVERVIEW });
    const element = container();
    await startDisplay(element, host, drawing);
    socketOf(host).send({ period_ms: 250, stale_ms: 750 });
    socketOf(host).send({ cycle: 1, values: { "Main.Speed": { value: 1180 } } });
    host.paint();
    assert.ok(element.querySelector("svg")?.textContent?.includes("1180"), element.innerHTML);
  });

  test("a lost connection turns the values stale with their age, and the display connects again", async () => {
    const host = new FakeHost("/hmi/overview", { status: 200, body: OVERVIEW });
    const element = container();
    await startDisplay(element, host, drawing);
    socketOf(host).send({ cycle: 1, values: { "Main.Speed": { value: 1180 } } });
    host.paint();
    host.clock = 4000;
    socketOf(host).onclose?.();
    host.paint();
    const card = element.querySelector('[data-object="speed"]');
    assert.strictEqual(card?.getAttribute("data-stale"), "true", element.innerHTML);
    assert.ok(card?.textContent?.includes("4 s"), card?.textContent ?? "");
    host.advance(2000);
    assert.strictEqual(host.sockets.length, 2, "a new connection is opened");
  });

  test("the drawing is renewed every second, so stale ages and trends move on without new values", async () => {
    const host = new FakeHost("/hmi/overview", { status: 200, body: OVERVIEW });
    const element = container();
    await startDisplay(element, host, drawing);
    socketOf(host).send({ period_ms: 250, stale_ms: 750 });
    socketOf(host).send({ cycle: 1, values: { "Main.Speed": { value: 1180 } } });
    host.paint();
    host.advance(1000);
    host.advance(1000);
    const card = element.querySelector('[data-object="speed"]');
    assert.strictEqual(card?.getAttribute("data-stale"), "true", "no message for three periods");
    assert.ok(card?.textContent?.includes("2 s"), card?.textContent ?? "");
  });

  test("a page file that does not load shows its errors with file and line, and opens no stream", async () => {
    const broken: PageData = {
      ...OVERVIEW,
      page: null,
      errors: [{ file: "hmi/pages/broken.toml", line: 3, message: "unknown field `colour`" }],
    };
    const host = new FakeHost("/hmi/broken", { status: 200, body: broken });
    const element = container();
    await startDisplay(element, host, drawing);
    host.paint();
    assert.ok(element.textContent?.includes("hmi/pages/broken.toml:3"), element.textContent ?? "");
    assert.ok(element.textContent?.includes("unknown field `colour`"), element.textContent ?? "");
    assert.strictEqual(host.sockets.length, 0);
  });

  test("a page the project does not have says so", async () => {
    const host = new FakeHost("/hmi/missing", { status: 404, body: { error: "the project has no page `missing`" } });
    const element = container();
    await startDisplay(element, host, drawing);
    host.paint();
    assert.ok(element.textContent?.includes("the project has no page `missing`"), element.textContent ?? "");
    assert.strictEqual(host.sockets.length, 0);
  });
});
