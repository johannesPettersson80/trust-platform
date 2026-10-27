import React, { useEffect, useRef, useState } from "react";

import {
  numberText,
  type PlantEditorView,
  type PlantTemperatureField,
} from "../plant/editorSession";

/**
 * The temperature example's physical values, an exact view of the same first-order draft
 * (specification 47 §7.1). Rust maps each typed value to the model parameters or refuses it; a
 * refused text stays as typed with Rust's reason.
 */
export function PlantTemperatureExampleEditor({ temperature, onSet, onRawValidityChange, inputStyle }: {
  temperature: NonNullable<PlantEditorView["temperature"]>;
  /** Asks Rust to apply the typed text; the reason it was refused, if it was. */
  onSet: (field: PlantTemperatureField, text: string) => Promise<string | undefined>;
  onRawValidityChange: (field: string, message?: string) => void;
  inputStyle: React.CSSProperties;
}) {
  const [texts, setTexts] = useState<Record<string, string>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const latest = useRef<Record<string, number>>({});
  // The field being typed in keeps its text; any other field shows Rust's value once that value is
  // another number, as after an edit of another field. A refused text stays as typed with its reason.
  const [focused, setFocused] = useState<string>();
  useEffect(() => {
    setTexts((current) => {
      const kept = Object.entries(current).filter(([field, text]) => field === focused ||
        errors[field] || Number(text) === temperature.values[field as PlantTemperatureField]);
      return kept.length === Object.keys(current).length ? current : Object.fromEntries(kept);
    });
  }, [temperature.values, errors, focused]);
  const enter = async (field: PlantTemperatureField, text: string) => {
    setTexts((current) => ({ ...current, [field]: text }));
    const sequence = (latest.current[field] ?? 0) + 1;
    latest.current[field] = sequence;
    const message = await onSet(field, text);
    if (latest.current[field] !== sequence) return;
    setErrors((current) => {
      const next = { ...current };
      if (message) next[field] = message;
      else delete next[field];
      return next;
    });
    onRawValidityChange(`temperature-${field}`, message);
  };
  return <div style={{ display: "grid", gap: 10, marginTop: 12 }}>
    <p className="trust-help" style={{ margin: 0 }}>Example settings — not calibrated to your equipment</p>
    {temperature.fields.map((field) => {
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
          value={texts[field.key] ?? numberText(temperature.values[field.key])}
          aria-invalid={Boolean(errors[field.key])}
          aria-describedby={`${helpId}${errors[field.key] ? ` ${errorId}` : ""}`}
          onChange={(event) => void enter(field.key, event.target.value)}
          onFocus={() => setFocused(field.key)}
          onBlur={() => setFocused((current) => (current === field.key ? undefined : current))}
          style={inputStyle}
        />
        {errors[field.key] && <span id={errorId} role="alert" style={{ color: "var(--trust-danger)" }}>{errors[field.key]}</span>}
      </label>;
    })}
  </div>;
}
