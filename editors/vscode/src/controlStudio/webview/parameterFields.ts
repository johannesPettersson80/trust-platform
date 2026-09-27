import type { ParameterBinding } from "../parameterBinding";
import { displayEngineeringUnit } from "../unitDisplay";

export function parameterFieldLabel(symbol: string): string {
  const leaf = symbol.split(".").pop() ?? symbol;
  return leaf.replace(/([a-z])([A-Z])/g, "$1 $2").replace(/\bGain\b/g, "gain");
}

/** Projects the existing reviewed JSON record into ordinary numeric controls. */
export class ParameterFields {
  private binding: ParameterBinding | undefined;
  private readonly inputs = new Map<string, HTMLInputElement>();
  constructor(private readonly host: HTMLElement, private readonly values: HTMLTextAreaElement,
    private readonly changed: () => void) {}

  configure(binding: ParameterBinding): void {
    if (JSON.stringify(binding) === JSON.stringify(this.binding)) return;
    this.binding = binding;
    this.inputs.clear(); this.host.replaceChildren();
    for (const [index, parameter] of binding.parameters.entries()) {
      const field = document.createElement("div"); field.className = "field trust-field";
      const input = document.createElement("input"); input.className = "trust-input";
      input.id = `manual-parameter-${index}`; input.type = "number"; input.step = "any";
      input.min = String(parameter.minimum); input.max = String(parameter.maximum);
      input.dataset.parameterSymbol = parameter.symbol;
      input.title = `${parameter.symbol} (${parameter.iec_type}) · ${parameter.convention}`;
      const label = document.createElement("label"); label.htmlFor = input.id;
      label.textContent = parameterFieldLabel(parameter.symbol);
      const hint = document.createElement("span"); hint.id = `${input.id}-unit`; hint.className = "field-hint";
      hint.textContent = `${displayEngineeringUnit(parameter.unit)} · ${parameter.minimum}–${parameter.maximum}`;
      input.setAttribute("aria-describedby", hint.id);
      input.addEventListener("input", () => {
        let values: Record<string, unknown>;
        try { values = JSON.parse(this.values.value); } catch { values = {}; }
        if (!values || typeof values !== "object" || Array.isArray(values)) values = {};
        values[parameter.symbol] = Number.isFinite(input.valueAsNumber) ? input.valueAsNumber : null;
        this.values.value = JSON.stringify(values, null, 2);
        this.changed();
      });
      this.inputs.set(parameter.symbol, input);
      field.append(label, input, hint); this.host.append(field);
    }
    this.host.hidden = false;
  }

  sync(): void {
    try {
      const values = JSON.parse(this.values.value);
      for (const [symbol, input] of this.inputs) {
        input.value = typeof values?.[symbol] === "number" && Number.isFinite(values[symbol]) ? String(values[symbol]) : "";
      }
    } catch { /* Keep incomplete JSON available for correction and host rejection. */ }
  }

  describe(reason: string): string {
    for (const parameter of this.binding?.parameters ?? []) {
      if (reason === `${parameter.symbol} is outside its declared bounds after IEC rounding.`) {
        return `${parameterFieldLabel(parameter.symbol)} must be between ${parameter.minimum} and ${parameter.maximum}.`;
      }
    }
    return reason;
  }
}
