// The screen layout around a page on the runtime display (specification 39, §4.6–4.8): the top
// bar with title, logo, page title, alarm counter, clock and session, and the navigation as a
// side bar or tabs. The page area takes the space that remains.

import { timeOfDay } from "../drawing/format";
import type { AlarmArea } from "./alarms";
import { icon } from "./icons";
import type { NavigationEntry, PageData, TopBarModel } from "./runtimeDisplay";
import { say, translate } from "../drawing/speech";

export const FRAME_CSS = `
.hmi-frame { position: fixed; inset: 0; display: flex; flex-direction: column; background: var(--hmi-page); }
.hmi-topbar { box-sizing: border-box; height: 56px; flex: none; display: flex; align-items: center; gap: 18px; padding: 0 18px; background: var(--hmi-surface); border-bottom: 1px solid var(--hmi-border); font: 500 14px var(--hmi-font); color: var(--hmi-text); }
.hmi-topbar .hmi-brand { display: flex; align-items: center; gap: 10px; font: 650 17px var(--hmi-font); letter-spacing: -0.2px; white-space: nowrap; }
.hmi-topbar .hmi-brand img { height: 30px; width: auto; }
.hmi-topbar .hmi-page-title { flex: 1; text-align: center; color: var(--hmi-muted); font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.hmi-topbar .hmi-spacer { flex: 1; }
.hmi-language { position: relative; }
.hmi-language-button { display: inline-flex; align-items: center; gap: 8px; height: 44px; padding: 0 14px 0 10px; border-radius: 999px; border: 1px solid var(--hmi-border); background: var(--hmi-raised); color: var(--hmi-text); font: 600 14px var(--hmi-font); cursor: pointer; }
.hmi-language-button svg { color: var(--hmi-muted); }
.hmi-language-menu { position: fixed; z-index: 30; display: flex; flex-direction: column; min-width: 200px; padding: 6px; border-radius: 14px; border: 1px solid var(--hmi-border); background: var(--hmi-surface); box-shadow: 0 16px 40px rgba(0, 0, 0, 0.35); }
.hmi-language-menu[hidden] { display: none; }
.hmi-language-menu button { min-height: 44px; padding: 0 14px; border: 0; border-radius: 10px; background: transparent; color: var(--hmi-text); font: 500 15px var(--hmi-font); text-align: left; cursor: pointer; }
.hmi-language-menu button:hover { background: var(--hmi-raised); }
.hmi-language-menu button[aria-checked="true"] { color: var(--hmi-accent); font-weight: 650; }
.hmi-corner { position: fixed; top: 68px; right: 16px; z-index: 12; display: flex; align-items: center; gap: 8px; }
.hmi-topbar .hmi-clock { font: 600 15px var(--hmi-font); font-variant-numeric: tabular-nums; color: var(--hmi-text); }
.hmi-topbar .hmi-alarms { display: flex; align-items: center; gap: 14px; min-width: 0; }
.hmi-topbar .hmi-alarm-count { display: inline-flex; align-items: center; gap: 8px; padding: 4px 12px; border-radius: 999px; border: 1px solid var(--hmi-border); color: var(--hmi-muted); font-weight: 650; white-space: nowrap; }
.hmi-topbar .hmi-alarm-count[data-priority="high"] { color: var(--hmi-alarm-high); border-color: var(--hmi-alarm-high); background: color-mix(in srgb, var(--hmi-alarm-high) 14%, transparent); }
.hmi-topbar .hmi-alarm-count[data-priority="medium"] { color: var(--hmi-alarm-medium); border-color: var(--hmi-alarm-medium); background: color-mix(in srgb, var(--hmi-alarm-medium) 14%, transparent); }
.hmi-topbar .hmi-alarm-count[data-priority="low"] { color: var(--hmi-alarm-low); border-color: var(--hmi-alarm-low); background: color-mix(in srgb, var(--hmi-alarm-low) 12%, transparent); }
.hmi-topbar .hmi-alarm-count[data-blink="true"] .hmi-alarm-mark { animation: hmi-blink 1s step-end infinite; }
.hmi-topbar [data-stale="true"] { outline: 2px dashed var(--hmi-stale); outline-offset: 2px; opacity: 0.85; }
.hmi-topbar .hmi-alarm-newest { display: inline-flex; align-items: center; gap: 8px; min-width: 0; max-width: 420px; border-radius: 6px; }
.hmi-topbar .hmi-alarm-message { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; color: var(--hmi-text); }
.hmi-topbar .hmi-alarm-time { color: var(--hmi-muted); font-variant-numeric: tabular-nums; }
.hmi-topbar .hmi-alarm-hidden { color: var(--hmi-muted); font-size: 13px; white-space: nowrap; }
.hmi-topbar .hmi-alarm-silence { min-width: 44px; min-height: 44px; padding: 4px 14px; border-radius: 999px; border: 1px solid var(--hmi-alarm-high); background: color-mix(in srgb, var(--hmi-alarm-high) 18%, transparent); color: var(--hmi-text); font: 650 14px var(--hmi-font); cursor: pointer; }
.hmi-topbar .hmi-alarm-silence[hidden] { display: none; }
.hmi-topbar .hmi-theme-switch { flex: none; width: 44px; height: 44px; border-radius: 22px; border: 1px solid var(--hmi-border); background: transparent; color: var(--hmi-text); font-size: 17px; line-height: 1; cursor: pointer; }
.hmi-topbar .hmi-alarm-mark { display: inline-flex; flex: none; }
.hmi-simulation { flex: none; display: inline-flex; align-items: center; gap: 8px; box-sizing: border-box; height: 32px; padding: 0 12px; border-radius: 999px; border: 1px solid var(--hmi-accent-3); background: color-mix(in srgb, var(--hmi-accent-3) 18%, transparent); color: var(--hmi-text); font: 750 12px var(--hmi-font); letter-spacing: 0.6px; white-space: nowrap; font-variant-numeric: tabular-nums; }
.hmi-simulation[hidden] { display: none; }
.hmi-simulation::before { content: ""; flex: none; width: 7px; height: 7px; border-radius: 50%; background: var(--hmi-accent-3); }
@media (max-width: 720px) { .hmi-simulation { gap: 6px; height: 28px; padding: 0 9px; font-size: 11px; } }

.hmi-session.hmi-session.hmi-session-inline { position: static; box-shadow: none; padding: 3px 4px 3px 12px; }
.hmi-body { flex: 1; display: flex; min-height: 0; }
.hmi-side { box-sizing: border-box; width: 232px; flex: none; overflow-y: auto; padding: 14px 10px; display: flex; flex-direction: column; gap: 2px; background: var(--hmi-surface); border-right: 1px solid var(--hmi-border); }
.hmi-side[data-position="right"] { order: 2; border-right: none; border-left: 1px solid var(--hmi-border); }
.hmi-side .hmi-group { margin: 14px 12px 6px; font: 650 11px var(--hmi-font); letter-spacing: 0.8px; text-transform: uppercase; color: var(--hmi-muted); }
.hmi-nav-item { display: flex; align-items: center; gap: 12px; min-height: 44px; padding: 0 12px; border-radius: 10px; text-decoration: none; font: 550 15px var(--hmi-font); color: var(--hmi-muted); }
.hmi-nav-item:hover { background: var(--hmi-raised); color: var(--hmi-text); }
.hmi-nav-item[aria-current="page"] { background: color-mix(in srgb, var(--hmi-accent) 14%, transparent); color: var(--hmi-accent); font-weight: 700; box-shadow: inset 3px 0 0 var(--hmi-accent); }
.hmi-tabs { box-sizing: border-box; height: 48px; flex: none; display: flex; align-items: stretch; gap: 4px; padding: 0 12px; background: var(--hmi-surface); border-bottom: 1px solid var(--hmi-border); overflow-x: auto; }
.hmi-tabs .hmi-nav-item { border-radius: 0; box-shadow: none; min-height: 0; }
.hmi-tabs .hmi-nav-item[aria-current="page"] { background: transparent; box-shadow: inset 0 -3px 0 var(--hmi-accent); }
.hmi-tabs .hmi-divider { width: 1px; margin: 12px 6px; background: var(--hmi-border); }
.hmi-main { flex: 1; min-width: 0; min-height: 0; position: relative; }
`;

/** The parts of the frame the display fills in. */
export interface Frame {
  /** Where the page is drawn. */
  main: HTMLElement;
  /** Where the session bar goes, when the top bar shows the user. */
  sessionSlot?: HTMLElement;
  /** Shows the time in the top bar's clock, if it has one. */
  tick(nowMs: number): void;
  /** The top bar's alarm area, when it shows alarms (§13.6, item 7). */
  alarms?: AlarmArea;
  /** The top bar, when the screen has one. */
  topBar?: HTMLElement;
  /** The simulation badge: hidden until the runtime reports a simulation running (§8 item 8). */
  simulation: HTMLElement;
}

/** Builds the frame for a page: top bar, navigation and the page area. */
/** The languages a screen offers, the one shown, and what choosing one does (§14.5). */
export interface LanguageChoice {
  current: string;
  languages: Array<{ tag: string; name: string }>;
  choose(tag: string): void;
}

export function buildFrame(
  container: HTMLElement,
  data: Partial<PageData>,
  pageName: string,
  pageTitle: string,
  language?: LanguageChoice,
  asset: (path: string) => string = assetUrl,
  signal?: AbortSignal,
): Frame {
  const document = container.ownerDocument;
  const frame = document.createElement("div");
  frame.className = "hmi-frame";
  const topBar = data.screen?.top_bar;
  // A session that floats, when the top bar does not show the user, stands under the bar.
  if (topBar) container.classList.add("hmi-with-top-bar");
  const bar = topBar ? buildTopBar(document, topBar, pageTitle, language, container, asset, signal) : undefined;
  if (bar) frame.append(bar.element);
  const body = document.createElement("div");
  body.className = "hmi-body";
  const main = document.createElement("main");
  main.className = "hmi-main";
  const navigation = buildNavigation(document, data, pageName);
  if (navigation?.getAttribute("data-style") === "tabs") frame.append(navigation);
  else if (navigation) body.append(navigation);
  body.append(main);
  frame.append(body);
  // The simulation badge is in the top bar; without one, it stands in the top-right corner with the
  // language button, under the session (§8 item 8, §14.5).
  const simulation = bar?.simulation ?? buildSimulation(document);
  if (!bar) {
    const corner = document.createElement("div");
    corner.className = "hmi-corner";
    corner.append(simulation);
    if (language) corner.append(buildLanguage(document, language, container, signal));
    frame.append(corner);
  }
  container.append(frame);
  const clock = bar?.clock;
  const tick = (nowMs: number): void => {
    if (clock) clock.textContent = timeOfDay(nowMs);
  };
  return { main, sessionSlot: bar?.sessionSlot, tick, alarms: bar?.alarms, topBar: bar?.element, simulation };
}

function buildTopBar(
  document: Document,
  model: TopBarModel,
  pageTitle: string,
  language?: LanguageChoice,
  layer?: HTMLElement,
  asset: (path: string) => string = assetUrl,
  signal?: AbortSignal,
): {
  element: HTMLElement;
  clock?: HTMLElement;
  sessionSlot: HTMLElement;
  alarms?: AlarmArea;
  simulation: HTMLElement;
} {
  const bar = document.createElement("header");
  bar.className = "hmi-topbar";
  bar.setAttribute("data-part", "top-bar");
  const brand = document.createElement("div");
  brand.className = "hmi-brand";
  if (model.logo) {
    const logo = document.createElement("img");
    logo.src = asset(model.logo);
    logo.alt = "";
    brand.append(logo);
  }
  const title = document.createElement("span");
  title.textContent = translate(model.title);
  brand.append(title);
  bar.append(brand);
  // The simulation badge follows the brand, where the operator reads what the screen is (§8 item 8).
  const simulation = buildSimulation(document);
  bar.append(simulation);
  if (model.page_title) {
    const page = document.createElement("div");
    page.className = "hmi-page-title";
    page.setAttribute("data-part", "page-title");
    page.textContent = pageTitle;
    bar.append(page);
  } else {
    const spacer = document.createElement("div");
    spacer.className = "hmi-spacer";
    bar.append(spacer);
  }
  let alarms: AlarmArea | undefined;
  if (model.alarms) {
    const area = document.createElement("div");
    area.className = "hmi-alarms";
    area.setAttribute("data-part", "alarms");
    const part = (name: string, className: string): HTMLElement => {
      const element = document.createElement("div");
      element.className = className;
      element.setAttribute("data-part", name);
      area.append(element);
      return element;
    };
    const silence = document.createElement("button");
    silence.type = "button";
    silence.className = "hmi-alarm-silence";
    silence.setAttribute("data-action", "silence");
    silence.textContent = say("Silence");
    silence.hidden = true;
    alarms = {
      count: part("alarm-count", "hmi-alarm-count"),
      newest: part("alarm-newest", "hmi-alarm-newest"),
      hidden: part("alarm-hidden", "hmi-alarm-hidden"),
      silence,
    };
    area.insertBefore(silence, alarms.newest);
    alarms.count.textContent = say("No alarms");
    bar.append(area);
  }
  let clock: HTMLElement | undefined;
  if (model.clock) {
    clock = document.createElement("div");
    clock.className = "hmi-clock";
    clock.setAttribute("data-part", "clock");
    bar.append(clock);
  }
  if (language && layer) bar.append(buildLanguage(document, language, layer, signal));
  // The session sits at the top bar's right end in every top bar, whatever `user` says: it shows on every
  // page, and the user and role belong in the top bar (§12.4 item 1; the owner, 29 October 2026).
  const sessionSlot = document.createElement("div");
  sessionSlot.className = "hmi-session-slot";
  bar.append(sessionSlot);
  return { element: bar, clock, sessionSlot, alarms, simulation };
}

/**
 * The simulation the runtime reports on its stream (specification 46, §6.1; specification 39, §8 item 8).
 * The runtime decides the speed and its words; the page shows them and translates only the phrase
 * "{achieved} of {configured}".
 */
export interface SimulationStatus {
  enabled: boolean;
  /** The configured speed when it is above 1×, the simulation runs this many times real time; `null` at 1×. */
  speed_configured: number | null;
  /** The measured speed as text, to one decimal rounded down, when the runtime says it is behind; `null` otherwise. */
  speed_achieved: string | null;
}

/** The simulation badge, hidden until the runtime reports a simulation running (§8 item 8). */
function buildSimulation(document: Document): HTMLElement {
  const badge = document.createElement("div");
  badge.className = "hmi-simulation";
  badge.setAttribute("data-part", "simulation");
  badge.hidden = true;
  const label = document.createElement("span");
  label.textContent = say("SIMULATION");
  const speed = document.createElement("span");
  speed.setAttribute("data-part", "simulation-speed");
  speed.hidden = true;
  badge.append(label, " ", speed);
  return badge;
}

/**
 * Shows the badge while the runtime reports a simulation running, with its speed after the label,
 * and hides it otherwise (§8 item 8).
 */
export function showSimulation(badge: HTMLElement, status: SimulationStatus | null | undefined): void {
  badge.hidden = status?.enabled !== true;
  const text = status?.enabled === true ? simulationSpeed(status) : "";
  const speed = badge.querySelector<HTMLElement>('[data-part="simulation-speed"]');
  if (!speed) return;
  speed.textContent = text;
  speed.hidden = text === "";
}

/**
 * The speed the badge shows after its label (§8 item 8), from the words the runtime gives: nothing when it
 * gives no configured speed (speed 1); "×N" at the configured speed N; "×A of ×N" when it also gives the
 * achieved speed A (the runtime says it is behind).
 */
function simulationSpeed(status: SimulationStatus): string {
  const configured = status.speed_configured;
  if (typeof configured !== "number") return "";
  const achieved = status.speed_achieved;
  if (typeof achieved === "string") {
    return say("{achieved} of {configured}", { achieved: `×${achieved}`, configured: `×${configured}` });
  }
  return `×${configured}`;
}

/**
 * The language button: the globe and the language shown; a click lists the screen's languages by
 * their own names, and choosing one switches to it (§14.5). The list opens on the top layer, under
 * the button, so nothing floating covers it; Escape or a click elsewhere closes it.
 */
function buildLanguage(
  document: Document,
  choice: LanguageChoice,
  layer: HTMLElement,
  signal?: AbortSignal,
): HTMLElement {
  const box = document.createElement("div");
  box.className = "hmi-language";
  box.setAttribute("data-part", "language");
  const button = document.createElement("button");
  button.type = "button";
  button.className = "hmi-language-button";
  button.setAttribute("data-action", "language");
  button.setAttribute("aria-haspopup", "menu");
  button.setAttribute("aria-expanded", "false");
  button.setAttribute("aria-label", say("Language"));
  const globe = icon(document, "globe");
  if (globe) button.append(globe);
  const name = document.createElement("span");
  name.textContent = choice.languages.find((entry) => entry.tag === choice.current)?.name ?? choice.current;
  button.append(name);
  const menu = document.createElement("div");
  menu.className = "hmi-language-menu";
  menu.setAttribute("data-part", "language-menu");
  menu.setAttribute("role", "menu");
  menu.hidden = true;
  for (const entry of choice.languages) {
    const item = document.createElement("button");
    item.type = "button";
    item.setAttribute("role", "menuitemradio");
    item.setAttribute("data-language", entry.tag);
    item.setAttribute("aria-checked", String(entry.tag === choice.current));
    item.setAttribute("lang", entry.tag);
    item.textContent = entry.name;
    item.addEventListener("click", () => {
      menu.hidden = true;
      if (entry.tag !== choice.current) choice.choose(entry.tag);
    });
    menu.append(item);
  }
  const show = (open: boolean): void => {
    if (open) {
      const at = button.getBoundingClientRect();
      const width = document.defaultView?.innerWidth ?? at.right;
      menu.style.top = `${Math.round(at.bottom + 6)}px`;
      menu.style.right = `${Math.max(8, Math.round(width - at.right))}px`;
    }
    menu.hidden = !open;
    button.setAttribute("aria-expanded", String(open));
  };
  button.addEventListener("click", () => show(menu.hidden !== false));
  const outside = (event: MouseEvent): void => {
    const target = event.target as Node | null;
    if (!menu.hidden && target && !menu.contains(target) && !button.contains(target)) show(false);
  };
  const escape = (event: KeyboardEvent): void => {
    if (event.key === "Escape" && !menu.hidden) show(false);
  };
  document.addEventListener("click", outside);
  document.addEventListener("keydown", escape);
  signal?.addEventListener(
    "abort",
    () => {
      document.removeEventListener("click", outside);
      document.removeEventListener("keydown", escape);
    },
    { once: true },
  );
  box.append(button);
  layer.append(menu);
  return box;
}

/** The navigation when at least two pages are listed (§4.7): pages without a group first, then
 * each group's pages together, in the order of the list. */
function buildNavigation(document: Document, data: Partial<PageData>, pageName: string): HTMLElement | undefined {
  const style = data.screen?.navigation?.style ?? "side";
  const entries = data.navigation ?? [];
  if (style === "none" || entries.length < 2) return undefined;
  const ordered = [...entries.filter((entry) => !entry.group), ...entries.filter((entry) => entry.group)];
  const navigation = document.createElement("nav");
  navigation.className = style === "tabs" ? "hmi-tabs" : "hmi-side";
  navigation.setAttribute("data-part", "navigation");
  navigation.setAttribute("data-style", style);
  navigation.setAttribute("data-position", data.screen?.navigation?.position ?? "left");
  navigation.setAttribute("aria-label", say("Pages"));
  let group: string | undefined;
  for (const entry of ordered) {
    if (entry.group !== group) {
      group = entry.group;
      navigation.append(groupMark(document, style, group));
    }
    navigation.append(link(document, entry, entry.name === pageName));
  }
  return navigation;
}

function groupMark(document: Document, style: string, group: string | undefined): HTMLElement {
  const mark = document.createElement("div");
  if (style === "tabs") {
    mark.className = "hmi-divider";
  } else {
    mark.className = "hmi-group";
    mark.setAttribute("data-part", "group");
    mark.textContent = group !== undefined ? translate(group) : "";
  }
  return mark;
}

function link(document: Document, entry: NavigationEntry, current: boolean): HTMLAnchorElement {
  const anchor = document.createElement("a");
  anchor.className = "hmi-nav-item";
  anchor.href = `/hmi/${encodeURIComponent(entry.name)}`;
  if (current) anchor.setAttribute("aria-current", "page");
  const drawn = icon(document, entry.icon);
  if (drawn) anchor.append(drawn);
  const title = document.createElement("span");
  title.textContent = translate(entry.title);
  anchor.append(title);
  return anchor;
}

/** Where the display loads a project image (`hmi/assets/…`) from. */
export function assetUrl(path: string): string {
  return path.startsWith("hmi/assets/") ? `/hmi/project-assets/${path.slice("hmi/assets/".length)}` : path;
}
