import { formatEngineeringNumber } from "../chartContract";

export interface DesignComparisonPinView {
  revision: number;
  structure: string;
  proportionalGain: number;
  integralGain?: number;
  derivativeGain?: number;
  source: string;
  validation: string;
  available: boolean;
  reason?: string;
}

export function stableComparisonKey(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stableComparisonKey).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.entries(value as Record<string, unknown>).sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${stableComparisonKey(item)}`).join(",")}}`;
  }
  return JSON.stringify(value) ?? "null";
}

export function renderDesignComparisonPins(_host: HTMLElement, _pins: readonly DesignComparisonPinView[], _selectedRevision: number | undefined,
  _select: (revision: number | undefined) => void, _remove: (revision: number) => void): void {
  const host = _host;
  host.replaceChildren(..._pins.map(pin => {
    const item = host.ownerDocument.createElement("span");
    item.className = "design-chip";
    item.dataset.pinnedRevision = String(pin.revision);
    const gains = `Kp ${formatEngineeringNumber(pin.proportionalGain)}` +
      (pin.integralGain === undefined ? "" : ` · Ki ${formatEngineeringNumber(pin.integralGain)}`) +
      (pin.derivativeGain === undefined ? "" : ` · Kd ${formatEngineeringNumber(pin.derivativeGain)}`);
    item.textContent = pin.available
      ? `Pinned ${pin.revision} · ${pin.structure} · ${gains} · ${pin.source} · ${pin.validation}`
      : `Pinned ${pin.revision} · Unavailable · ${pin.reason ?? "Comparison context changed."}`;
    const select = host.ownerDocument.createElement("button");
    select.type = "button";
    select.className = "secondary";
    select.dataset.pinnedSelect = String(pin.revision);
    select.textContent = "Select";
    select.setAttribute("aria-label", `Select pinned ${pin.structure} design ${pin.revision} for comparison`);
    select.disabled = !pin.available;
    select.setAttribute("aria-pressed", String(_selectedRevision === pin.revision));
    select.addEventListener("click", () => _select(pin.revision));
    const remove = host.ownerDocument.createElement("button");
    remove.type = "button";
    remove.className = "secondary";
    remove.dataset.pinnedRemove = String(pin.revision);
    remove.textContent = "Remove";
    remove.setAttribute("aria-label", `Remove pinned ${pin.structure} design ${pin.revision}`);
    remove.addEventListener("click", () => _remove(pin.revision));
    item.append(select, remove);
    return item;
  }));
  if (_selectedRevision !== undefined) {
    const current = host.ownerDocument.createElement("button");
    current.type = "button";
    current.className = "secondary";
    current.dataset.pinnedReturn = "current";
    current.textContent = "Return to current";
    current.setAttribute("aria-label", "Return to current candidate comparison");
    current.addEventListener("click", () => _select(undefined));
    host.append(current);
  }
}

export function historyShortcutAction(event: KeyboardEvent): "undo" | "redo" | undefined {
  if (event.defaultPrevented || !(event.ctrlKey || event.metaKey) || event.altKey || event.key.toLowerCase() !== "z") return undefined;
  let target = event.target as (Element & { parentElement?: Element | null }) | null;
  if (target?.matches?.("input, textarea, select")) return undefined;
  while (target) {
    const editable = target.getAttribute?.("contenteditable");
    if (editable !== null && editable !== "false") return undefined;
    target = target.parentElement ?? null;
  }
  return event.shiftKey ? "redo" : "undo";
}
