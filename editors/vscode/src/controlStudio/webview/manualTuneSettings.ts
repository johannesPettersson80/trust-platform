import { displayEngineeringUnit } from "../chartContract";
import { openedOffers } from "./openedOffers";

export interface ManualTuneSettingsValue {
  /** Each entry as the engineer typed it: the engine reads it, and refuses an entry that no number reads under its row (B4.5). */
  values: Record<string, string>;
}

export interface ManualTuneSettingsControls {
  render(payload: Record<string, unknown>): void;
  read(): ManualTuneSettingsValue;
  /** Marks the row whose setting a refusal names (its `field`), and no other row. */
  refuse(field: string | undefined): void;
}

/** The settings a controller family takes, as the engine names them (`controllerOffer.familySettings`, C2); undefined when it names none. */
export type FamilySymbols = (family: string) => readonly string[] | undefined;

/**
 * Renders the editable settings of the family the candidate uses, beside the candidate, without writing runtime values. The engine answers
 * the rows (symbol, label, unit and bounds, B4.2) and the settings each family takes; the page shows the rows of the family and sends each
 * entry as the engineer typed it. The engine checks each entry against its bounds. Each edit notifies `changed` with the entries so far.
 */
export function initializeManualTuneSettings(root: HTMLElement, changed?: (value: ManualTuneSettingsValue) => void,
  familySymbols?: FamilySymbols): ManualTuneSettingsControls {
  const area = root.querySelector<HTMLElement>("[data-studio-tuning-result]")!;
  const rows = new Map<string, HTMLInputElement>();
  const snapshot = (): ManualTuneSettingsValue => ({
    values: Object.fromEntries([...rows].map(([symbol, input]) => [symbol, input.value.trim()])),
  });
  return {
    read: snapshot,
    refuse(field) {
      for (const [symbol, input] of rows) {
        const refused = symbol === field;
        if (refused) input.setAttribute("aria-invalid", "true");
        else input.removeAttribute("aria-invalid");
        input.classList.toggle("trust-input--error", refused);
      }
    },
    render(payload) {
      if (payload.status !== "candidate" || !payload.values || typeof payload.values !== "object") return;
      const candidate = payload.values as Record<string, number | string>;
      // The family is the engine's: the candidate's, or the one the Controller selector shows (B4.2). A payload that names none shows no row.
      const family = typeof payload.family === "string" ? payload.family : undefined;
      const relevant = new Set(family === undefined ? [] : familySymbols?.(family) ?? []);
      rows.clear();
      const fieldset = document.createElement("fieldset");
      fieldset.dataset.studioManualSettings = "true";
      fieldset.setAttribute("aria-label", "Manual controller settings");
      for (const row of openedOffers().controllerOffer?.settingRows ?? []) {
        if (!relevant.has(row.symbol)) continue;
        const wrapper = document.createElement("div");
        wrapper.className = "studio-field studio-manual-setting";
        const label = document.createElement("label");
        label.textContent = row.label;
        const input = document.createElement("input");
        // Text, so that an entry no number reads reaches the engine, which refuses it under this row (B4.5).
        input.type = "text";
        input.inputMode = "decimal";
        // An entry the engineer typed stays as typed; a number is shown as its number.
        const entry = candidate[row.symbol];
        input.value = typeof entry === "string" ? entry : typeof entry === "number" && Number.isFinite(entry) ? String(entry) : "";
        input.title = `${row.minimum} to ${row.maximum}`;
        input.className = "trust-input";
        input.dataset.studioManualValue = row.symbol;
        input.id = `manual-setting-${rows.size}`;
        label.htmlFor = input.id;
        const unit = document.createElement("span");
        unit.textContent = row.unit === "1" ? "" : displayEngineeringUnit(row.unit);
        input.addEventListener("input", () => changed?.(snapshot()));
        wrapper.append(label, input, unit);
        fieldset.append(wrapper);
        rows.set(row.symbol, input);
      }
      const title = document.createElement("p");
      title.textContent = family === undefined ? "" : `${family} controller`;
      area.replaceChildren(title, fieldset);
      area.hidden = false;
    },
  };
}
