/** Manual parameter controls share the host's candidate and persist only explicit pins. */
export function controllerPinControls(section: HTMLElement) {
  const container = section.querySelector<HTMLElement>("[data-engineer-setting-pins]")!;
  let pins: Record<string, number> = {};
  const rows = new Map<string, { selected: HTMLInputElement; value: HTMLInputElement }>();
  return {
    read(): Record<string, number> | null {
      const values: Record<string, number> = {};
      for (const [symbol, row] of rows) if (row.selected.checked) {
        if (!Number.isFinite(row.value.valueAsNumber)) throw new Error(`Enter a finite pinned value for ${symbol}.`);
        values[symbol] = row.value.valueAsNumber;
      }
      return Object.keys(values).length ? values : null;
    },
    render(payload: Record<string, unknown>): void {
      const saved = payload.settingsPins as { values?: Record<string, number> } | undefined;
      if ("settingsPins" in payload) pins = saved?.values ?? {};
      const candidate = payload.values as Record<string, number> | undefined;
      if (!candidate && !saved) return;
      container.replaceChildren(); rows.clear();
      for (const [symbol, value] of Object.entries(candidate ?? pins)) {
        if (!Number.isFinite(value)) continue;
        const label = document.createElement("label");
        const selected = document.createElement("input"); selected.type = "checkbox"; selected.checked = symbol in pins;
        selected.setAttribute("aria-label", `Pin ${symbol}`);
        const name = document.createElement("span"); name.textContent = symbol;
        const input = document.createElement("input"); input.type = "number"; input.step = "any"; input.value = String(pins[symbol] ?? value);
        input.setAttribute("aria-label", `${symbol} pinned value`);
        input.disabled = !selected.checked;
        selected.addEventListener("change", () => { input.disabled = !selected.checked; });
        label.append(selected, name, input); container.append(label);
        rows.set(symbol, { selected, value: input });
      }
    },
  };
}
