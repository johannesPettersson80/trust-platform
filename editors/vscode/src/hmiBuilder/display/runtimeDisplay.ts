import { screenCoordinates } from "../drawing/regionCoordinates";
// The runtime display in the browser (specification 39, §11.1): loads the page named in the
// address with its screen layout, keeps the live values the runtime streams, and redraws.

import { patchSurface, updateNode, type Drawn } from "./surfacePatch";
import { ValuePaintQueue } from "./valuePaintQueue";
import type { AdaptedPage } from "../drawing/adaptive";
import type { CustomSymbolModel } from "../drawing/symbols/custom";
import type { DrawFrame, HmiPageModel, PlacedFaceplateModel } from "../drawing/model";
import type { SvgNode } from "../drawing/svg";
import type { HmiColorsModel } from "../drawing/tokens";
import { AlarmMessage, AlarmView, loadRecords } from "./alarms";
import { timeOfDay } from "../drawing/format";
import { assetUrl, buildFrame, FRAME_CSS, showSimulation, type SimulationStatus } from "./frame";
import { loadHistory, LiveValueStore } from "./liveValues";
import { TREND_CSS, TrendActions } from "./trendActions";
import { MOTION_LAYER_CSS, MotionLayer } from "./motionLayer";
import { EquipmentFocus, EQUIPMENT_FOCUS_CSS } from "./equipmentNavigation";
import { OPERATE_CSS, OperateMessage, Operation } from "./operate";
import { PlacedFaceplates } from "./placedFaceplates";
import { SETTINGS_CSS } from "./settings";
import { POPUP_CSS, Popups } from "./popups";
import { designPoint, parseMessage, socketUrl, trendSpans } from "./shared";
import { displaySpeech, say, setSpeech, translate } from "../drawing/speech";

/** The top bar a screen layout turns on (§4.8). */
export interface TopBarModel {
  title: string;
  logo?: string;
  clock?: boolean;
  alarms?: boolean;
  /** Kept as the file says it; the session sits in every top bar (§4.8 item 8). */
  user?: boolean;
  page_title?: boolean;
}

/** A page of the navigation (§4.6). */
export interface NavigationEntry {
  name: string;
  title: string;
  icon?: string;
  group?: string;
}

/** What `/api/hmi/page` answers. */
export interface PageData {
  /** Shared read-only unit and precision defaults keyed by the folded IEC path (§5.9). */
  formats?: DrawFrame["formats"];
  screen: {
    size: [number, number];
    theme?: "light" | "dark";
    /** The symbol style (§4.1, §14.2): the two-dimensional symbols. */
    symbols?: "flat";
    colors?: HmiColorsModel;
    top_bar?: TopBarModel;
    navigation?: { style: "side" | "tabs" | "none"; position: "left" | "right" };
  } | null;
  /** The language of the texts, and the project's languages by their own names (§14.5). */
  language?: string;
  languages?: Array<{ tag: string; name: string }>;
  /** The screen's text table in that language. */
  texts?: Record<string, string>;
  pages: string[];
  /** The pages the navigation lists, in order (§4.6). */
  navigation?: NavigationEntry[];
  page: { name: string; model: HmiPageModel } | null;
  errors: Array<{ file: string; line: number; message: string }>;
  error?: string;
  /** The version of the HMI files the page is drawn from; commands carry it (§12.8). */
  bundle?: string;
  /** The project's own symbols the page places (§14.4). */
  symbols?: CustomSymbolModel[];
  /** The lowest role each control needs, none where no role may use it (§5.9 item 5). */
  roles?: Record<string, string | null>;
  /** The faceplates placed on the page, by object id, each bound to its instance and named as
   * its popup is, `null` where the project lacks the faceplate (§8.9). */
  faceplates?: Record<string, PlacedFaceplateData | null>;
}

/** A placed faceplate as the page answer gives it. */
export interface PlacedFaceplateData {
  name: string;
  model: PlacedFaceplateModel;
  roles?: Record<string, string | null>;
}

export interface SocketLike {
  onmessage: ((event: { data: unknown }) => void) | null;
  onclose: (() => void) | null;
  close(): void;
}

/** What the display needs from the browser; tests give their own. */
export interface DisplayHost {
  location: { pathname: string; host: string; protocol: string; search?: string };
  /** Resolves a bundled project image in an embedded display. */
  asset?(path: string): string;
  /** Keeps a choice of this browser, such as its language (§14.5); nothing when storage is off. */
  remember?(key: string, value: string): void;
  /** A choice this browser kept. */
  recall?(key: string): string | undefined;
  fetchJson(url: string): Promise<{ status: number; body: unknown }>;
  /** Sends a JSON body and reads the JSON answer; throws when the connection fails. */
  postJson(url: string, body: unknown): Promise<{ status: number; body: unknown }>;
  /** In the HMI Builder's Live mode: keeps settings that the PLC has in the project (§21.4 item 6);
   * absent in a browser. */
  keepSettings?(
    instance: string,
    settings: Record<string, boolean | number | string | null>,
  ): Promise<{ status: number; body: unknown }>;
  openSocket(url: string): SocketLike;
  now(): number;
  /** Runs a callback before the next paint. */
  schedule(callback: () => void): void;
  /** Runs a callback after a delay. */
  later(milliseconds: number, callback: () => void): void;
  /** Loads the page again, for a newer bundle (§16.2.6). */
  reload(): void;
  /** Goes to another address, for a page button (§4.9). */
  navigate(url: string): void;
  /** The alarm tones, where the display can play them (§5.9 item 2). */
  sounds?: import("./alarmSound").AlarmSounds;
  /** Gives the browser a file to save, such as a trend's CSV (§5.9 item 1). */
  download?(name: string, text: string): void;
}

/** The drawing module, as the runtime's `TrustHmiDrawing` global gives it. */
export interface DrawingApi {
  /** The page laid out for its page area (§4 item 5). */
  adaptPage(page: HmiPageModel, design: [number, number], area: [number, number]): AdaptedPage;
  /** Makes the project's own symbols known to the drawing (§14.4). */
  registerSymbols(models: CustomSymbolModel[]): void;
  drawPage(page: HmiPageModel, frame: DrawFrame): SvgNode;
  renderToString(node: SvgNode): string;
  tokensCss(colors?: HmiColorsModel, fontUrl?: string): string;
  /** An alarm priority's mark as an image of its own (§13.6, item 6). */
  priorityMarkImage(priority: string, size: number): SvgNode;
}

const FONT_URL = "/hmi/assets/fonts/InterVariable.woff2";
/** How long to wait before connecting again after the stream closes. */
const RECONNECT_MS = 2000;
/** The drawing is renewed at least this often, so stale ages and trends move on. */
const REDRAW_MS = 1000;

const DISPLAY_CSS = `
@keyframes hmi-blink { 50% { opacity: 0.25; } }
.hmi-blink { animation: hmi-blink 1s step-end infinite; }
.hmi-surface { width: 100%; height: 100%; }
.hmi-surface > svg { display: block; width: 100%; height: 100%; }
.hmi-surface.hmi-scroll { overflow: auto; }
.hmi-surface.hmi-scroll > svg { width: auto; height: auto; }
.hmi-message { box-sizing: border-box; min-height: 100%; display: grid; place-items: center; padding: 32px; }
.hmi-message section { max-width: 720px; padding: 28px 32px; border-radius: 16px; background: var(--hmi-surface); border: 1px solid var(--hmi-border); }
.hmi-message h1 { margin: 0 0 12px; font: 650 22px var(--hmi-font); color: var(--hmi-text); }
.hmi-message p { margin: 6px 0 0; font: 14px/1.5 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; color: var(--hmi-muted); }
`;

/** Starts the display in a container: loads the page, opens the value stream, and redraws. */
/** Where a browser keeps its choice of language (§14.5). */
const LANGUAGE_KEY = "trust-hmi-language";
/** Where a panel keeps its day or night choice (§18 phase 5 item 6). */
const THEME_KEY = "trust-hmi-theme";

export async function startDisplay(
  container: HTMLElement,
  host: DisplayHost,
  drawing: DrawingApi,
  signal?: AbortSignal,
): Promise<void> {
  if (signal?.aborted) return;
  const requested = pageNameOf(host.location.pathname);
  // The language the address fixes, else the one this browser chose; the runtime falls back to the
  // screen's first language when the project does not have it (§14.5).
  const pinned = new URLSearchParams(host.location.search ?? "").get("lang") ?? undefined;
  const asked = pinned ?? host.recall?.(LANGUAGE_KEY);
  const query = [
    requested !== undefined ? `name=${encodeURIComponent(requested)}` : undefined,
    asked ? `lang=${encodeURIComponent(asked)}` : undefined,
  ].filter((part): part is string => part !== undefined);
  const url = `/api/hmi/page${query.length > 0 ? `?${query.join("&")}` : ""}`;
  let answer: { status: number; body: unknown };
  try {
    answer = await host.fetchJson(url);
  } catch (error) {
    if (signal?.aborted) return;
    showMessage(container, drawing, say("The runtime cannot be reached"), [String(error)]);
    return;
  }
  if (signal?.aborted) return;
  const data = answer.body as Partial<PageData> | null;
  setSpeech(data?.texts ?? {}, data?.language ?? "en");
  // The page speaks its language: screen readers, and the CJK font of that language (§14.5).
  container.ownerDocument.documentElement.setAttribute("lang", data?.language ?? "en");
  if (answer.status !== 200 || !data?.screen || !data.page) {
    const lines = (data?.errors ?? []).map((error) => `${error.file}:${error.line}: ${error.message}`);
    const heading =
      data?.error ?? (lines.length > 0 ? say("This page does not load") : say("There is no page to show"));
    showMessage(container, drawing, heading, lines);
    return;
  }
  const { screen, page } = data;
  // The project's own symbols the page places (§14.4).
  drawing.registerSymbols(data.symbols ?? []);

  const document = container.ownerDocument;
  // Day or night as this panel last chose, else the project's theme (§18 phase 5 item 6).
  const kept = host.recall?.(THEME_KEY);
  let theme: "dark" | "light" = kept === "dark" || kept === "light" ? kept : (screen.theme ?? "light");
  container.classList.add(`hmi-theme-${theme}`);
  const style = document.createElement("style");
  // The frame's styles come after the operation's, which they refine (the session in the top bar).
  style.textContent =
    drawing.tokensCss(screen.colors ?? {}, FONT_URL) +
    DISPLAY_CSS +
    MOTION_LAYER_CSS +
    OPERATE_CSS +
    FRAME_CSS +
    POPUP_CSS +
    SETTINGS_CSS +
    TREND_CSS +
    EQUIPMENT_FOCUS_CSS;
  container.append(style);
  // The top bar and the navigation around the page area (§4.7).
  const languages = data.languages ?? [];
  const language =
    languages.length > 1 && pinned === undefined
      ? {
          current: data.language ?? languages[0].tag,
          languages,
          choose: (tag: string): void => {
            host.remember?.(LANGUAGE_KEY, tag);
            host.reload();
          },
        }
      : undefined;
  const frame = buildFrame(container, data, page.name, translate(page.model.title), language, host.asset, signal);
  if (frame.topBar) {
    const toggle = document.createElement("button");
    toggle.type = "button";
    toggle.className = "hmi-theme-switch";
    toggle.setAttribute("data-action", "theme");
    const offer = (): void => {
      const label = say(theme === "dark" ? "Day" : "Night");
      toggle.textContent = theme === "dark" ? "☀" : "☾";
      toggle.setAttribute("aria-label", label);
      toggle.title = label;
    };
    offer();
    toggle.addEventListener("click", () => {
      container.classList.remove(`hmi-theme-${theme}`);
      theme = theme === "dark" ? "light" : "dark";
      container.classList.add(`hmi-theme-${theme}`);
      host.remember?.(THEME_KEY, theme);
      offer();
    });
    frame.topBar.insertBefore(toggle, frame.sessionSlot ?? null);
  }
  const surface = document.createElement("div");
  surface.className = "hmi-surface";
  frame.main.append(surface);
  const motion = new MotionLayer(surface);

  // A placed panel's trends keep their history like the page's own (§8.9).
  const placedModels = Object.values(data.faceplates ?? {}).flatMap((placed) => (placed ? [placed.model] : []));
  const store = new LiveValueStore({ trendSpansMs: trendSpans(page.model, placedModels) });
  // The trends' ranges and live or paused windows (§5.9 item 1).
  const save = (name: string, text: string): void => host.download?.(name, text);
  const trends = new TrendActions(
    () => shown.model,
    store,
    (url) => host.fetchJson(url),
    surface,
    () => host.now(),
    {},
    save,
    (id) => expandTrend(id),
  );
  /** A trend shown over the page at its design size, sharing its view (§5.9 item 1). */
  let expanded:
    | {
        objectId: string;
        layer: HTMLElement;
        inner: HTMLElement;
        trends: TrendActions;
        at: (event: MouseEvent) => { x: number; y: number };
      }
    | undefined;
  const expandedModel = (objectId: string): HmiPageModel => {
    const object = shown.model.object.find((candidate) => candidate.id === objectId);
    return {
      title: "",
      object: object ? [{ ...object, at: [24, 24], size: [screen.size[0] - 48, screen.size[1] - 48] }] : [],
      pipe: [],
    };
  };
  function expandTrend(objectId: string): void {
    expanded?.layer.remove();
    const layer = document.createElement("div");
    layer.className = "hmi-trend-expanded";
    layer.setAttribute("data-part", "trend-expanded");
    const close = document.createElement("button");
    close.type = "button";
    close.className = "hmi-trend-close";
    close.setAttribute("data-action", "close");
    close.setAttribute("aria-label", say("Close"));
    close.textContent = "✕";
    const inner = document.createElement("div");
    inner.className = "hmi-trend-surface";
    layer.append(close, inner);
    container.append(layer);
    const local = new TrendActions(
      () => expandedModel(objectId),
      store,
      (url) => host.fetchJson(url),
      inner,
      () => host.now(),
      trends.views,
      save,
    );
    const atIn = (event: MouseEvent): { x: number; y: number } => designPoint(inner, screen.size, event);
    const idOf = (event: Event): string | undefined =>
      (event.target as Element | null)?.closest?.("[data-object]")?.getAttribute("data-object") ?? undefined;
    close.addEventListener("click", () => {
      layer.remove();
      expanded = undefined;
      redraw();
    });
    inner.addEventListener("click", (event) => void local.click(event.target as Element | null, idOf(event), redraw));
    inner.addEventListener("pointerdown", (event) => {
      const id = idOf(event);
      if (id && local.press(id, atIn(event as MouseEvent), event.target as Element | null)) {
        event.preventDefault();
        redraw();
      }
    });
    inner.addEventListener(
      "wheel",
      (event) => {
        const id = idOf(event);
        if (id && local.wheel(id, atIn(event as MouseEvent), (event as WheelEvent).deltaY)) {
          event.preventDefault();
          redraw();
        }
      },
      { passive: false },
    );
    expanded = { objectId, layer, inner, trends: local, at: atIn };
    redraw();
  }
  // The display sounds where its top bar shows the alarms, beside Silence (§5.9 item 2).
  const alarms = new AlarmView(frame.alarms ? host.sounds : undefined);
  frame.alarms?.silence.addEventListener("click", () => {
    alarms.silence();
    redraw();
  });
  const painting = {
    mark: (priority: string) => drawing.renderToString(drawing.priorityMarkImage(priority, 18)),
    time: timeOfDay,
  };
  const valuePaint = new ValuePaintQueue(
    (callback) => host.schedule(callback),
    (element, before, after) => updateNode(element, before, after, drawing.renderToString),
  );
  let pending = false;
  let lastRenderedAt = Number.NEGATIVE_INFINITY;
  /** The page as drawn last, so an update replaces only what changed (§19.6). */
  let drawn: Drawn | undefined;
  /** The page as laid out for its area (§4 item 5): what is drawn, clicked and dragged. */
  const shown = { name: page.name, model: page.model, roles: data.roles ?? {}, formats: data.formats };
  let layout: { size: [number, number]; fit: boolean } = { size: screen.size, fit: true };
  /** The page laid out for the area it was last drawn in: the layout does not depend on values. */
  let laidOut: { area: [number, number]; adapted: AdaptedPage } | undefined;
  const render = (): void => {
    if (signal?.aborted) return;
    pending = false;
    lastRenderedAt = host.now();
    // An area not yet measured shows the page as designed.
    const area: [number, number] =
      surface.clientWidth > 0 && surface.clientHeight > 0 ? [surface.clientWidth, surface.clientHeight] : screen.size;
    if (laidOut?.area[0] !== area[0] || laidOut.area[1] !== area[1])
      laidOut = { area, adapted: drawing.adaptPage(page.model, screen.size, area) };
    const adapted = laidOut.adapted;
    shown.model = screenCoordinates(adapted.page);
    layout = adapted;
    surface.classList.toggle("hmi-scroll", !adapted.fit);
    const drawFrame: DrawFrame = {
      currentPage: page.name,
      size: adapted.size,
      formats: data.formats,
      ...store.frame(host.now()),
      commands: operation.states(),
      readOnly: operation.readOnly,
      locked: operation.locked(),
      trendViews: trends.views,
      drafts: operation.drafts(),
      placed: placedFaceplates.frame(),
      asset: host.asset ?? assetUrl,
      ...displaySpeech(),
      alarms: alarms.frame(store.isStale(host.now())),
    };
    drawn = patchSurface(surface, drawing.drawPage(adapted.page, drawFrame), drawing.renderToString, drawn, valuePaint);
    valuePaint.flush();
    equipmentFocus.apply(surface);
    if (expanded)
      expanded.inner.innerHTML = drawing.renderToString(
        drawing.drawPage(expandedModel(expanded.objectId), { ...drawFrame, size: screen.size }),
      );
    motion.sync();
    if (frame.alarms) alarms.drawTopBar(frame.alarms, store.isStale(host.now()), painting);
  };
  const redraw = (): void => {
    if (signal?.aborted) return;
    popups?.redraw();
    if (!pending) {
      pending = true;
      host.schedule(render);
    }
  };
  const equipmentFocus = new EquipmentFocus(host, redraw);
  let socket: SocketLike | undefined;
  const cleanup: Array<() => void> = [];
  signal?.addEventListener(
    "abort",
    () => {
      valuePaint.dispose();
      if (socket) {
        socket.onmessage = null;
        socket.onclose = null;
        socket.close();
      }
      host.sounds?.stop();
      cleanup.forEach((remove) => remove());
    },
    { once: true },
  );
  const listen = (target: EventTarget | undefined, name: string, handler: EventListener): void => {
    target?.addEventListener(name, handler);
    cleanup.push(() => target?.removeEventListener(name, handler));
  };
  /** A new stream in place of the open one, which closes without connecting again itself. */
  const reconnect = (): void => {
    if (socket) {
      socket.onclose = null;
      socket.close();
    }
    connect();
  };
  const operation = new Operation(
    container,
    host,
    shown,
    data.bundle ?? "",
    () => redraw(),
    (path) => store.frame(host.now()).values[path],
    () => reconnect(),
    frame.sessionSlot,
    painting.mark,
  );
  /** A click's target: a page to go to, a popup to open (§4.9), or the faceplate of the object
   * clicked, named by where the object stands and its id (§8.7, item 5). */
  const follow = (target: string, object: string, source: string): void => {
    const [kind, name] = [target.slice(0, target.indexOf(":")), target.slice(target.indexOf(":") + 1)];
    if (kind === "page") host.navigate(`/hmi/${encodeURIComponent(name)}`);
    else if (kind === "popup") void popups?.open(name);
    else if (kind === "faceplate") void popups?.openFaceplate(`${source}/${object}`, name);
  };
  const popups: Popups | undefined = new Popups(container, host, drawing, operation, follow);
  cleanup.push(() => popups.dispose());
  // The faceplates placed on the page, each operated as its popup is (§8.9).
  const placedFaceplates = new PlacedFaceplates(
    data.faceplates ?? {},
    operation,
    () => redraw(),
    (path) => store.frame(host.now()).values[path],
    () => shown.model,
  );
  surface.addEventListener("click", (event) => {
    const clicked = event.target as Element | null;
    if (placedFaceplates.click(clicked, clicked?.closest?.("[data-option]")?.getAttribute("data-option") ?? undefined))
      return;
    const target = clicked?.closest?.("[data-object]");
    const id = target?.getAttribute("data-object");
    // A trend's chips (§5.9 item 1); the alarm list's tabs, pages, buttons and rows (§5.7, item 4).
    if (trends.click(clicked, id ?? undefined, redraw)) return;
    if (operation.alarms.click(clicked, id ?? undefined, alarms, redraw)) return;
    const option = clicked?.closest?.("[data-option]")?.getAttribute("data-option") ?? undefined;
    const goes = target?.getAttribute("data-click");
    if (goes && id) follow(goes, id, page.name);
    else if (id) operation.click(id, option);
  });
  // A slider is dragged from a press on it until the pointer is released anywhere (§5.8).
  const at = (event: MouseEvent): { x: number; y: number } => designPoint(surface, layout.size, event);
  surface.addEventListener("pointerdown", (event) => {
    operation.overlay.touched = (event as PointerEvent).pointerType === "touch";
    const id = (event.target as Element | null)?.closest?.("[data-object]")?.getAttribute("data-object");
    // A placed faceplate's slider (§8.9), a trend's box or pan (§5.9 item 1), else a slider.
    if (placedFaceplates.press(event.target as Element | null, at(event as MouseEvent))) event.preventDefault();
    else if (id && trends.press(id, at(event as MouseEvent), event.target as Element | null)) {
      event.preventDefault();
      redraw();
    } else if (id && operation.press(id, at(event as MouseEvent))) event.preventDefault();
  });
  surface.addEventListener(
    "wheel",
    (event) => {
      const id = (event.target as Element | null)?.closest?.("[data-object]")?.getAttribute("data-object");
      if (id && trends.wheel(id, at(event as MouseEvent), (event as WheelEvent).deltaY)) {
        event.preventDefault();
        redraw();
      }
    },
    { passive: false },
  );
  const view = document.defaultView ?? undefined;
  // A new page area lays the page out anew (§4 item 5).
  listen(view, "resize", () => redraw());
  const pointerTarget: EventTarget = view ?? document;
  listen(pointerTarget, "pointermove", (event) => {
    const over = (event.target as Element | null)?.closest?.("[data-object]")?.getAttribute("data-object") ?? undefined;
    const open = expanded;
    if (open) {
      const inside = open.inner.contains(event.target as Node | null);
      if (
        open.trends.drag(open.at(event as MouseEvent)) ||
        open.trends.hover(inside ? over : undefined, open.at(event as MouseEvent))
      )
        redraw();
    } else if (trends.drag(at(event as MouseEvent)) || trends.hover(over, at(event as MouseEvent))) redraw();
    operation.drag(at(event as MouseEvent));
    placedFaceplates.drag(at(event as MouseEvent));
  });
  listen(pointerTarget, "pointerup", (event) => {
    if (expanded?.trends.release(expanded.at(event as MouseEvent)) || trends.release(at(event as MouseEvent))) redraw();
    operation.release(at(event as MouseEvent));
    placedFaceplates.release(at(event as MouseEvent));
  });
  await operation.start();
  if (signal?.aborted) return;
  await loadRecords(host, page.model, alarms);
  if (signal?.aborted) return;
  await loadHistory((url) => host.fetchJson(url), trendSpans(page.model, placedModels), store);
  if (signal?.aborted) return;
  const connect = (): void => {
    if (signal?.aborted) return;
    const opened = host.openSocket(socketUrl(host.location, `page=${encodeURIComponent(page.name)}`));
    socket = opened;
    opened.onmessage = (event) => {
      if (signal?.aborted) return;
      const message = parseMessage(event.data);
      if (!message) return;
      // The session and commands are the operation's; values are the store's.
      if ("session" in message || "command" in message || "bundle" in message) {
        operation.receive(message as OperateMessage);
        return;
      }
      // The alarm list and records (specification 40, §5.1).
      if ("alarms" in message || "events" in message) {
        alarms.receive(message as AlarmMessage);
        redraw();
        return;
      }
      // The simulation the header shows, as the stream reports it (specification 46, §6.1).
      if ("simulation" in message) {
        showSimulation(frame.simulation, message.simulation as SimulationStatus | null | undefined);
        return;
      }
      store.receive(message, host.now());
      redraw();
    };
    opened.onclose = () => {
      if (signal?.aborted) return;
      store.disconnected(host.now());
      redraw();
      host.later(RECONNECT_MS, connect);
    };
  };
  const tick = (): void => {
    if (signal?.aborted) return;
    const now = host.now();
    frame.tick(now);
    // Fresh stream updates already advance the page. Avoid a second whole-page draw during
    // their pending card paints; still advance stale ages and quiet pages on every timer tick.
    if (store.isStale(now) || now - lastRenderedAt >= REDRAW_MS) redraw();
    host.later(REDRAW_MS, tick);
  };
  connect();
  frame.tick(host.now());
  host.later(REDRAW_MS, tick);
  redraw();
}

/** The page named in the address: `/hmi/<page>`, or none for `/hmi`. */
function pageNameOf(pathname: string): string | undefined {
  const name = pathname.replace(/^\/hmi\/?/, "").replace(/\/$/, "");
  return name === "" ? undefined : decodeURIComponent(name);
}

/** A message instead of the page, for example the errors of a page file that does not load. */
function showMessage(container: HTMLElement, drawing: DrawingApi, heading: string, lines: string[]): void {
  const document = container.ownerDocument;
  container.classList.add("hmi-theme-dark");
  const style = document.createElement("style");
  style.textContent = drawing.tokensCss({}, FONT_URL) + DISPLAY_CSS;
  const box = document.createElement("div");
  box.className = "hmi-message";
  const section = document.createElement("section");
  const title = document.createElement("h1");
  title.textContent = heading;
  section.append(title);
  for (const line of lines) {
    const paragraph = document.createElement("p");
    paragraph.textContent = line;
    section.append(paragraph);
  }
  box.append(section);
  container.append(style, box);
}
