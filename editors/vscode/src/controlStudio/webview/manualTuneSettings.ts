import { seededGainLabels } from "./threePageResults";
import { decodePidLaw, decodeProportionalLaw } from "../controllerLaw";
import type { ControlLoopDefinition } from "../loopDefinition";
import { displayEngineeringUnit } from "../chartContract";
import { groupSettingSymbols, ownedParameterLabel, PID_GAIN_LABELS } from "../controllerGroupPresentation";

export interface ManualTuneSettingsValue {
  values: Record<string, number>;
  pinnedValues: Record<string, number>;
}

export interface ManualTuneSettingsControls {
  render(payload: Record<string, unknown>): void;
  read(): ManualTuneSettingsValue;
}

/** Renders editable declared controller settings beside a candidate without writing runtime values. */
export function initializeManualTuneSettings(root: HTMLElement, loop: ControlLoopDefinition,
  changed?: (value: ManualTuneSettingsValue) => void): ManualTuneSettingsControls {
  const area = root.querySelector<HTMLElement>("[data-studio-tuning-result]")!;
  const rows = new Map<string, { value: HTMLInputElement; pin: HTMLInputElement }>();
  let values: Record<string, number> = {};
  let pinnedValues: Record<string, number> = {};

  const symbols = (family = "PID"): Set<string> => {
    const group = groupSettingSymbols(loop);
    if (group) return group;
    const pid = decodePidLaw(loop.controller.law, loop.parameterBinding);
    if (pid) return new Set([
      pid.proportionalGainSymbol,
      ...(["PI", "PID"].includes(family) ? [pid.integralGainSymbol] : []),
      ...(["PD", "PID"].includes(family) ? [pid.derivativeGainSymbol] : []),
      ...(["PI", "PID"].includes(family) && pid.antiWindup ? [pid.antiWindup.trackingGainSymbol] : []),
      ...(["PI", "PID"].includes(family) && pid.proportionalSetpointWeightSymbol ? [pid.proportionalSetpointWeightSymbol] : []),
      ...(["PD", "PID"].includes(family) && pid.derivativeSetpointWeightSymbol ? [pid.derivativeSetpointWeightSymbol] : []),
      ...(pid.measurementFilterTimeSymbol ? [pid.measurementFilterTimeSymbol] : []),
    ]);
    const proportional = decodeProportionalLaw(loop.controller.law, loop.parameterBinding);
    return proportional ? new Set([proportional.gainSymbol]) : new Set(loop.parameterBinding?.parameters.map(parameter => parameter.symbol) ?? []);
  };
  const snapshot = (): ManualTuneSettingsValue => {
    const next: Record<string, number> = loop.controller.group ? { ...values } : {};
    const pins: Record<string, number> = loop.controller.group ? { ...pinnedValues } : {};
    for (const [symbol, row] of rows) {
      const value = row.value.valueAsNumber;
      const parameter = loop.parameterBinding?.parameters.find(candidate => candidate.symbol === symbol);
      if (!parameter || !Number.isFinite(value) || value < parameter.minimum || value > parameter.maximum) {
        row.value.setAttribute("aria-invalid", "true"); row.value.classList.add("trust-input--error"); row.value.focus();
        const label = row.value.labels?.[0]?.textContent;
        // Lower-case only the sentence position, so symbols such as (Kp) or (Tf) keep their case.
        const name = label ? label.charAt(0).toLowerCase() + label.slice(1) : symbol;
        throw new Error(`Enter ${name} between ${parameter?.minimum ?? "the declared minimum"} and ${parameter?.maximum ?? "the declared maximum"}.`);
      }
      row.value.removeAttribute("aria-invalid"); row.value.classList.remove("trust-input--error");
      next[symbol] = value;
      if (row.pin.checked) pins[symbol] = value;
      else delete pins[symbol];
    }
    return { values: next, pinnedValues: pins };
  };
  const notify = (): void => { if (!changed) return; try { const next = snapshot(); values = next.values; pinnedValues = next.pinnedValues; changed(next); } catch { /* Keep incomplete edits for explicit validation on Tune. */ } };
  return {
    read: snapshot,
    render(payload) {
      if (payload.status !== "candidate" || !payload.values || typeof payload.values !== "object") return;
      const candidate = payload.values as Record<string, number>;
      const saved = payload.settingsPins && typeof payload.settingsPins === "object" ? payload.settingsPins as { values?: Record<string, number> } : undefined;
      const relevant = symbols(typeof payload.family === "string" ? payload.family : "PID");
      values = loop.controller.group ? { ...candidate } : Object.fromEntries([...relevant].filter(symbol => Number.isFinite(candidate[symbol])).map(symbol => [symbol, candidate[symbol]]));
      pinnedValues = saved?.values ?? {};
      rows.clear();
      const fieldset = document.createElement("fieldset");
      fieldset.dataset.studioManualSettings = "true";
      fieldset.setAttribute("aria-label", "Manual controller settings");
      for (const parameter of loop.parameterBinding?.parameters ?? []) {
        if (!relevant.has(parameter.symbol)) continue;
        const row = document.createElement("div"); row.className = "studio-field studio-manual-setting";
        const label = document.createElement("label"); label.textContent = loop.controller.law?.kind === "pid"
          ? parameter.symbol === loop.controller.law.proportionalGainSymbol ? PID_GAIN_LABELS.ProportionalGain
            : parameter.symbol === loop.controller.law.integralGainSymbol ? PID_GAIN_LABELS.IntegralGain
              : parameter.symbol === loop.controller.law.derivativeGainSymbol ? PID_GAIN_LABELS.DerivativeGain
                : parameter.symbol === loop.controller.law.proportionalSetpointWeightSymbol ? "Setpoint weight (β)"
                  : parameter.symbol === loop.controller.law.derivativeSetpointWeightSymbol ? "Derivative setpoint weight (γ)"
                  : parameter.symbol === loop.controller.law.measurementFilterTimeSymbol ? "Measurement filter time (Tf)" : parameter.convention
          : seededGainLabels(loop)?.get(parameter.symbol) ?? (parameter.convention || parameter.symbol);
        label.textContent = ownedParameterLabel(loop, parameter.symbol, label.textContent ?? parameter.symbol);
        const input = document.createElement("input"); input.type = "number"; input.step = "any"; input.min = String(parameter.minimum); input.max = String(parameter.maximum); input.value = values[parameter.symbol] === undefined ? "" : String(values[parameter.symbol]); input.className = "trust-input"; input.dataset.studioManualValue = parameter.symbol;
        input.id = `manual-setting-${rows.size}`; label.htmlFor = input.id;
        const unit = document.createElement("span"); unit.textContent = parameter.unit === "1" ? "" : displayEngineeringUnit(parameter.unit);
        const pinLabel = document.createElement("label"); const pin = document.createElement("input"); pin.type = "checkbox"; pin.checked = Number.isFinite(pinnedValues[parameter.symbol]); pin.dataset.studioManualPin = parameter.symbol; pinLabel.append(pin, " Pin");
        input.addEventListener("input", notify); pin.addEventListener("change", notify);
        row.append(label, input, unit, pinLabel); fieldset.append(row); rows.set(parameter.symbol, { value: input, pin });
      }
      const title = document.createElement("p"); title.textContent = `${payload.family} controller · ${loop.controller.type}`;
      area.replaceChildren(title, fieldset); area.hidden = false;
    },
  };
}
