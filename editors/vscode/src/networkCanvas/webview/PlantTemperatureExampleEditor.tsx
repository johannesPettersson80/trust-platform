import React, { useEffect, useState } from "react";
import type { PlantEditorDraft } from "../plant/editorModel";
import {
  temperatureExampleValues, updateTemperatureExampleField,
  type TemperatureExampleField,
} from "../plant/temperatureExample";

const FIELDS: ReadonlyArray<{ key: TemperatureExampleField; label: string; help: string }> = [
  { key: "resting_temperature", label: "Resting temperature (°C)", help: "The temperature approached with heater power at 0%." },
  { key: "full_power_rise", label: "Temperature rise at full power (°C)", help: "The change above the resting temperature at 100% power. A negative value describes cooling." },
  { key: "time_constant_s", label: "Response time (s)", help: "After the delay, about 63% of the remaining temperature change occurs in this time." },
  { key: "dead_time_s", label: "Delay before response (s)", help: "How long a new heater command takes to begin affecting temperature." },
];

/** A physical view of the same FOPDT draft; no independent simulation or model state. */
export function PlantTemperatureExampleEditor({ draft, onChange, onRawValidityChange, inputStyle }: {
  draft: PlantEditorDraft;
  onChange: (draft: PlantEditorDraft) => void;
  onRawValidityChange: (field: string, message?: string) => void;
  inputStyle: React.CSSProperties;
}) {
  const values = temperatureExampleValues(draft);
  const [raw, setRaw] = useState<Record<string, string>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  useEffect(() => {
    setRaw((current) => Object.fromEntries(Object.entries(current).filter(([key]) => errors[key])));
  }, [draft.parameters, errors]);
  return <div style={{ display: "grid", gap: 10, marginTop: 12 }}>
    <p className="trust-help" style={{ margin: 0 }}>Example settings — not calibrated to your equipment</p>
    {FIELDS.map((field) => {
      const errorId = `plant-temperature-error-${field.key}`;
      const helpId = `plant-temperature-help-${field.key}`;
      return <label key={field.key} style={{ display: "grid", gap: 4 }}>
        <span>{field.label}</span>
        <span id={helpId} className="trust-help">{field.help}</span>
        <input
          data-plant-temperature-field={field.key}
          data-plant-dynamics-field={`temperature-${field.key}`}
          name={`plant-temperature-${field.key}`}
          type="text" inputMode="decimal"
          value={raw[field.key] ?? String(values[field.key])}
          aria-invalid={Boolean(errors[field.key])}
          aria-describedby={`${helpId}${errors[field.key] ? ` ${errorId}` : ""}`}
          onChange={(event) => {
            const text = event.target.value;
            const value = Number(text);
            let message: string | undefined;
            if (!text.trim() || !Number.isFinite(value)) message = `${field.label} must be a finite number.`;
            else if (field.key === "time_constant_s" && value <= 0) message = "Response time must be greater than 0 s.";
            else if (field.key === "dead_time_s" && value < 0) message = "Delay must be 0 s or greater.";
            setRaw((current) => ({ ...current, [field.key]: text }));
            setErrors((current) => {
              const next = { ...current };
              if (message) next[field.key] = message;
              else delete next[field.key];
              return next;
            });
            onRawValidityChange(`temperature-${field.key}`, message);
            if (!message) onChange(updateTemperatureExampleField(draft, field.key, value));
          }}
          style={inputStyle}
        />
        {errors[field.key] && <span id={errorId} role="alert" style={{ color: "var(--trust-danger)" }}>{errors[field.key]}</span>}
      </label>;
    })}
  </div>;
}
