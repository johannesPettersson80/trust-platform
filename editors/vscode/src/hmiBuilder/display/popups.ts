// Popups on the runtime display (specification 39, §4.9): a popup opens over the page with its
// title and a close button, its own values and its own commands. It is modal until it is pinned,
// and it closes with its button or Escape.

import { patchSurface, updateNode, type Drawn } from "./surfacePatch";
import { mayChangeSettings } from "./settings";
import { ValuePaintQueue } from "./valuePaintQueue";
import type { CustomSymbolModel } from "../drawing/symbols/custom";
import { AlarmMessage, AlarmView, loadRecords } from "./alarms";
import type { DrawFrame, HmiPageModel } from "../drawing/model";
import type { CommandSurface } from "./commands";
import { assetUrl } from "./frame";
import { loadHistory, LiveValueStore } from "./liveValues";
import { TrendActions } from "./trendActions";
import type { Operation } from "./operate";
import type { DisplayHost, DrawingApi, SocketLike } from "./runtimeDisplay";
import { designPoint, parseMessage, socketUrl, trendSpans } from "./shared";
import { displaySpeech, say } from "../drawing/speech";

/** A popup as `/api/hmi/popup` gives it; a faceplate is titled "P101 — Pump" and may have a
 * description and settings (§8.4, §8.7). */
interface PopupModel extends HmiPageModel {
  size: [number, number];
  pinnable?: boolean;
  description?: string;
  settings_instance?: string;
}

/** How the runtime names a faceplate: by the object that opens it. */
const FACEPLATE = "faceplate:";

/** How long to wait before a popup's stream connects again. */
const RECONNECT_MS = 2000;

export const POPUP_CSS = `
.hmi-popup-backdrop { position: fixed; inset: 0; z-index: 15; display: grid; place-items: center; background: rgba(0, 0, 0, 0.5); backdrop-filter: blur(2px); }
.hmi-popup { box-sizing: border-box; display: flex; flex-direction: column; max-width: 92vw; border-radius: 16px; overflow: hidden; background: var(--hmi-surface); border: 1px solid var(--hmi-border); box-shadow: 0 24px 60px rgba(0, 0, 0, 0.5); }
.hmi-popup-header { display: flex; align-items: center; gap: 8px; padding: 10px 10px 10px 18px; border-bottom: 1px solid var(--hmi-border); }
.hmi-popup-heading { flex: 1; min-width: 0; }
.hmi-popup-header h2 { margin: 0; font: 650 16px var(--hmi-font); color: var(--hmi-text); }
.hmi-popup-header p { margin: 2px 0 0; font: 500 14px var(--hmi-font); color: var(--hmi-muted); }
.hmi-popup-header button { min-width: 36px; height: 36px; border-radius: 10px; cursor: pointer; border: 1px solid var(--hmi-border); background: var(--hmi-raised); color: var(--hmi-text); font: 600 14px var(--hmi-font); }
.hmi-popup-surface > svg { display: block; width: 100%; height: 100%; }
.hmi-pinned { position: fixed; right: 24px; bottom: 24px; z-index: 12; display: flex; flex-direction: column-reverse; gap: 12px; }
`;

export class Popups {
  private readonly windows: PopupWindow[] = [];
  private pinnedArea: HTMLElement | undefined;
  private disposed = false;
  private readonly keydown = (event: KeyboardEvent): void => {
    if (event.key === "Escape") this.windows[this.windows.length - 1]?.close();
  };

  constructor(
    private readonly root: HTMLElement,
    private readonly host: DisplayHost,
    private readonly drawing: DrawingApi,
    private readonly operation: Operation,
    /** What a click with a target in a popup does: a page, another popup, or a faceplate of the
     * clicked object, given where it stands. */
    private readonly follow: (target: string, object: string, source: string) => void,
  ) {
    root.ownerDocument.addEventListener("keydown", this.keydown);
  }

  /** Ends every popup with its owning display, including pending opens. */
  dispose(): void {
    this.disposed = true;
    this.root.ownerDocument.removeEventListener("keydown", this.keydown);
    [...this.windows].forEach((window) => window.close());
    this.pinnedArea?.remove();
  }

  /** Opens a popup by name, unless it is open already. */
  async open(name: string): Promise<void> {
    await this.show(name, `/api/hmi/popup?name=${encodeURIComponent(name)}`, name);
  }

  /** Opens the faceplate of an object, named `<page>/<object id>` (or `popup:<name>/<object id>`),
   * unless it is open already; `name` is the faceplate's, for a notice (§8.7, item 5). */
  async openFaceplate(source: string, name: string): Promise<void> {
    await this.show(`${FACEPLATE}${source}`, `/api/hmi/popup?faceplate=${encodeURIComponent(source)}`, name);
  }

  private async show(address: string, url: string, name: string): Promise<void> {
    if (this.disposed || this.windows.some((window) => window.name === address)) return;
    let answer: { status: number; body: unknown };
    try {
      answer = await this.host.fetchJson(url);
    } catch {
      if (this.disposed) return;
      this.operation.overlay.notice(
        say('The popup "{name}" cannot be opened: the runtime cannot be reached.', { name }),
      );
      return;
    }
    if (this.disposed) return;
    const body = answer.body as {
      popup?: { name: string; model: PopupModel; roles?: Record<string, string | null> } | null;
      symbols?: CustomSymbolModel[];
      formats?: DrawFrame["formats"];
    } | null;
    const popup = body?.popup;
    if (answer.status !== 200 || !popup) {
      this.operation.overlay.notice(say('The popup "{name}" cannot be opened.', { name }));
      return;
    }
    // The project's own symbols the popup places (§14.4).
    this.drawing.registerSymbols(body?.symbols ?? []);
    const window = new PopupWindow(this, popup.name, popup.model, popup.roles ?? {}, body?.formats);
    this.windows.push(window);
  }

  /** Every popup draws again, for stale ages and the session. */
  redraw(): void {
    this.windows.forEach((window) => window.redraw());
  }

  /** The area pinned popups stay in. */
  pinned(): HTMLElement {
    if (!this.pinnedArea) {
      this.pinnedArea = this.root.ownerDocument.createElement("div");
      this.pinnedArea.className = "hmi-pinned";
      this.root.append(this.pinnedArea);
    }
    return this.pinnedArea;
  }

  forget(window: PopupWindow): void {
    this.windows.splice(this.windows.indexOf(window), 1);
  }

  get context(): {
    root: HTMLElement;
    host: DisplayHost;
    drawing: DrawingApi;
    operation: Operation;
    follow: (target: string, object: string, source: string) => void;
  } {
    return { root: this.root, host: this.host, drawing: this.drawing, operation: this.operation, follow: this.follow };
  }
}

class PopupWindow {
  private readonly backdrop: HTMLElement;
  private readonly window: HTMLElement;
  private readonly header: HTMLElement;
  /** The header's Settings button, shown only in an Engineer session (§21.4 item 1). */
  private readonly settingsButton: HTMLButtonElement | undefined;
  private readonly surface: HTMLElement;
  private readonly store: LiveValueStore;
  private readonly valuePaint: ValuePaintQueue;
  private drawn: Drawn | undefined;
  /** The alarm list and records the popup's stream sends (specification 40, §5.1). */
  private readonly alarms = new AlarmView();
  private readonly commands: CommandSurface;
  /** The popup's trends' ranges and windows (§5.9 item 1). */
  private readonly trends: TrendActions;
  private socket: SocketLike | undefined;
  private pending = false;
  private closed = false;
  private dragging = false;
  private readonly cleanup: Array<() => void> = [];

  constructor(
    private readonly popups: Popups,
    readonly name: string,
    private readonly model: PopupModel,
    /** The role each control needs (§5.9 item 5). */
    roles: Record<string, string | null>,
    private readonly formats?: DrawFrame["formats"],
  ) {
    const { root, host, operation, drawing } = popups.context;
    this.valuePaint = new ValuePaintQueue(
      (callback) => host.schedule(callback),
      (element, before, after) => updateNode(element, before, after, drawing.renderToString),
    );
    const document = root.ownerDocument;
    this.backdrop = document.createElement("div");
    this.backdrop.className = "hmi-popup-backdrop";
    this.backdrop.setAttribute("data-part", "popup-backdrop");
    this.window = document.createElement("section");
    this.window.className = "hmi-popup";
    this.window.setAttribute("data-part", "popup");
    this.window.setAttribute("role", "dialog");
    // The runtime titles a faceplate by its equipment, "P101 — Pump" (§8.4 item 1).
    this.window.setAttribute("aria-label", model.title);
    this.window.style.width = `min(${model.size[0]}px, 92vw)`;
    const header = document.createElement("header");
    header.className = "hmi-popup-header";
    this.header = header;
    const heading = document.createElement("div");
    heading.className = "hmi-popup-heading";
    const title = document.createElement("h2");
    title.setAttribute("data-part", "popup-title");
    title.textContent = model.title;
    heading.append(title);
    if (model.description) {
      const description = document.createElement("p");
      description.setAttribute("data-part", "popup-description");
      description.textContent = model.description;
      heading.append(description);
    }
    header.append(heading);
    this.settingsButton = model.settings_instance
      ? this.button(say("Settings"), "settings", say("Equipment settings"), () => {
          void operation.settings.open(name, model.title);
        })
      : undefined;
    this.showSettingsButton();
    if (model.pinnable)
      header.append(this.button(say("Pin"), "pin", say("Keep open beside the page"), () => this.pin()));
    header.append(this.button("✕", "close", say("Close"), () => this.close()));
    this.surface = document.createElement("div");
    this.surface.className = "hmi-popup-surface";
    this.surface.style.aspectRatio = `${model.size[0]} / ${model.size[1]}`;
    this.window.append(header, this.surface);
    this.backdrop.append(this.window);
    root.append(this.backdrop);

    this.store = new LiveValueStore({ trendSpansMs: trendSpans(model) });
    this.trends = new TrendActions(
      () => model,
      this.store,
      (url) => host.fetchJson(url),
      this.surface,
      () => host.now(),
    );
    this.commands = operation.popupSurface(
      { name, model, roles, formats },
      () => this.redraw(),
      (path) => this.store.frame(host.now()).values[path],
    );
    this.surface.addEventListener("click", (event) => this.clicked(event));
    this.surface.addEventListener("pointerdown", (event) => {
      operation.overlay.touched = (event as PointerEvent).pointerType === "touch";
      const id = (event.target as Element | null)?.closest?.("[data-object]")?.getAttribute("data-object");
      // A trend's box or pan (§5.9 item 1), else a slider.
      if (id && this.trends.press(id, this.at(event as MouseEvent), event.target as Element | null)) {
        event.preventDefault();
        this.redraw();
        return;
      }
      this.dragging = id !== undefined && id !== null && this.commands.press(id, this.at(event as MouseEvent));
      if (this.dragging) event.preventDefault();
    });
    this.surface.addEventListener(
      "wheel",
      (event) => {
        const id = (event.target as Element | null)?.closest?.("[data-object]")?.getAttribute("data-object");
        if (id && this.trends.wheel(id, this.at(event as MouseEvent), (event as WheelEvent).deltaY)) {
          event.preventDefault();
          this.redraw();
        }
      },
      { passive: false },
    );
    const pointer: EventTarget = document.defaultView ?? document;
    const move: EventListener = (event) => {
      const over =
        (event.target as Element | null)?.closest?.("[data-object]")?.getAttribute("data-object") ?? undefined;
      if (this.trends.drag(this.at(event as MouseEvent)) || this.trends.hover(over, this.at(event as MouseEvent)))
        this.redraw();
      if (this.dragging) this.commands.drag(this.at(event as MouseEvent));
    };
    const release: EventListener = (event) => {
      if (this.trends.release(this.at(event as MouseEvent))) this.redraw();
      if (!this.dragging) return;
      this.dragging = false;
      this.commands.release(this.at(event as MouseEvent));
    };
    pointer.addEventListener("pointermove", move);
    pointer.addEventListener("pointerup", release);
    this.cleanup.push(
      () => pointer.removeEventListener("pointermove", move),
      () => pointer.removeEventListener("pointerup", release),
    );
    this.connect();
    this.redraw();
    void loadRecords(host, model, this.alarms).then(() => this.redraw());
    void loadHistory((url) => host.fetchJson(url), trendSpans(model), this.store).then(() => this.redraw());
  }

  /** Draws again before the next paint; the header follows the session. */
  redraw(): void {
    this.showSettingsButton();
    if (this.pending || this.closed) return;
    this.pending = true;
    const { host } = this.popups.context;
    host.schedule(() => {
      this.pending = false;
      if (!this.closed) this.render();
    });
  }

  /** The Settings button stands in the header, after the title, only while an Engineer is
   * signed in; Viewers and Operators do not see it (§21.4 item 1). */
  private showSettingsButton(): void {
    const button = this.settingsButton;
    if (!button) return;
    const shown = mayChangeSettings(this.popups.context.operation.session.role);
    const placed = button.parentElement === this.header;
    if (shown && !placed) this.header.querySelector(".hmi-popup-heading")?.after(button);
    else if (!shown && placed) button.remove();
  }

  /** Pinned, the popup stays open beside the page, which can be used again. */
  pin(): void {
    this.popups.pinned().append(this.window);
    this.backdrop.remove();
    this.window.setAttribute("data-pinned", "true");
    this.window.querySelector('[data-action="pin"]')?.remove();
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    this.valuePaint.dispose();
    this.dragging = false;
    this.cleanup.forEach((remove) => remove());
    if (this.socket) {
      this.socket.onmessage = null;
      this.socket.onclose = null;
      this.socket.close();
    }
    this.backdrop.remove();
    this.window.remove();
    this.popups.context.operation.closePopupSurface(this.commands);
    this.popups.forget(this);
  }

  private render(): void {
    const { host, drawing, operation } = this.popups.context;
    const frame = {
      size: this.model.size,
      formats: this.formats,
      ...this.store.frame(host.now()),
      commands: this.commands.states(),
      drafts: this.commands.drafts(),
      readOnly: operation.readOnly,
      locked: this.commands.locked(),
      trendViews: this.trends.views,
      asset: host.asset ?? assetUrl,
      ...displaySpeech(),
      alarms: this.alarms.frame(this.store.isStale(host.now())),
    };
    this.drawn = patchSurface(
      this.surface,
      drawing.drawPage(this.model, frame),
      drawing.renderToString,
      this.drawn,
      this.valuePaint,
    );
    this.valuePaint.flush();
  }

  private connect(): void {
    const { host } = this.popups.context;
    const query = this.name.startsWith(FACEPLATE)
      ? `faceplate=${encodeURIComponent(this.name.slice(FACEPLATE.length))}`
      : `popup=${encodeURIComponent(this.name)}`;
    const opened = host.openSocket(socketUrl(host.location, query));
    this.socket = opened;
    opened.onmessage = (event) => {
      const message = parseMessage(event.data);
      // The page's stream reports the session and the commands; a popup takes its values.
      if (!message || "session" in message || "command" in message || "bundle" in message || "simulation" in message)
        return;
      if ("alarms" in message || "events" in message) {
        this.alarms.receive(message as AlarmMessage);
        this.redraw();
        return;
      }
      this.store.receive(message, host.now());
      this.redraw();
    };
    opened.onclose = () => {
      this.store.disconnected(host.now());
      this.redraw();
      host.later(RECONNECT_MS, () => {
        if (!this.closed) this.connect();
      });
    };
  }

  private clicked(event: Event): void {
    const clicked = event.target as Element | null;
    const target = clicked?.closest?.("[data-object]");
    const id = target?.getAttribute("data-object");
    if (!id) return;
    if (this.trends.click(clicked, id, () => this.redraw())) return;
    if (this.popups.context.operation.alarms.click(clicked, id, this.alarms, () => this.redraw())) return;
    const follow = target?.getAttribute("data-click");
    if (follow) {
      // A faceplate opens for an object of a popup; a faceplate's own objects open none.
      this.popups.context.follow(follow, id, `popup:${this.name}`);
      return;
    }
    const option = clicked?.closest?.("[data-option]")?.getAttribute("data-option") ?? undefined;
    this.commands.click(id, option);
  }

  private at(event: MouseEvent): { x: number; y: number } {
    return designPoint(this.surface, this.model.size, event);
  }

  private button(text: string, action: string, label: string, run: () => void): HTMLButtonElement {
    const created = this.window.ownerDocument.createElement("button");
    created.type = "button";
    created.textContent = text;
    created.title = label;
    created.setAttribute("aria-label", label);
    created.setAttribute("data-action", action);
    created.addEventListener("click", run);
    return created;
  }
}
