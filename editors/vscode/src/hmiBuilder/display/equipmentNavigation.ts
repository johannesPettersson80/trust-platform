// Alarm details navigate to the equipment's existing page (§5.9), using read-only page data.
import type { HmiObjectModel } from "../drawing/model";
import type { DisplayHost, PageData } from "./runtimeDisplay";

function match(object: HmiObjectModel, equipment: string): number {
  if (object.instance === equipment || object.click?.instance === equipment) return 2;
  if (Object.values(object.bind).some((binding) => binding.path.startsWith(`${equipment}.`)))
    return object.click?.faceplate ? 2 : 1;
  return 0;
}

/** Finds only pages the current display can read, without commands or a second binding map. */
export async function equipmentDestination(host: DisplayHost, equipment: string): Promise<string | undefined> {
  if (!equipment) return undefined;
  const name = decodeURIComponent(host.location.pathname.split("/").pop() ?? "");
  const first = await host.fetchJson(`/api/hmi/page?name=${encodeURIComponent(name)}`);
  if (first.status !== 200) throw new Error("page lookup unavailable");
  const initial = first.body as PageData;
  const names = [
    ...new Set(
      [initial.page?.name, ...(initial.navigation ?? []).map((page) => page.name), ...initial.pages].filter(
        (value): value is string => Boolean(value),
      ),
    ),
  ];
  for (const pageName of names) {
    const answer =
      pageName === initial.page?.name
        ? first
        : await host.fetchJson(`/api/hmi/page?name=${encodeURIComponent(pageName)}`);
    if (answer.status !== 200) continue;
    const data = answer.body as PageData;
    if (data.bundle !== initial.bundle) throw new Error("the HMI changed during lookup");
    const objects = data.page?.model.object ?? [];
    const object =
      objects.find((object) => match(object, equipment) === 2) ??
      objects.find((object) => match(object, equipment) === 1);
    if (object) return `/hmi/${encodeURIComponent(pageName)}?focus=${encodeURIComponent(object.id)}`;
  }
  return undefined;
}

/** A brief, non-alarm outline on the destination, maintained across live redraws. */
export class EquipmentFocus {
  private readonly id: string | null;
  private readonly expires: number;
  private scrolled = false;

  constructor(
    private readonly host: DisplayHost,
    redraw: () => void,
  ) {
    this.id = new URLSearchParams(host.location.search ?? "").get("focus");
    this.expires = host.now() + 5000;
    if (this.id) host.later(5000, redraw);
  }

  apply(surface: HTMLElement): void {
    if (!this.id) return;
    surface.querySelectorAll("[data-focused]").forEach((node) => node.removeAttribute("data-focused"));
    if (this.host.now() >= this.expires) return;
    const target = Array.from(surface.querySelectorAll("[data-object]")).find(
      (node) => node.getAttribute("data-object") === this.id,
    );
    if (!target) return;
    target.setAttribute("data-focused", "true");
    if (!this.scrolled) {
      target.scrollIntoView?.({ block: "nearest", inline: "nearest" });
      this.scrolled = true;
    }
  }
}

export const EQUIPMENT_FOCUS_CSS = `.hmi-surface [data-focused="true"] { outline: 2px dashed var(--hmi-accent); outline-offset: 6px; }`;
