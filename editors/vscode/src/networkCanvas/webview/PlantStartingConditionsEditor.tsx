import React, { useEffect, useState } from "react";

/** Scalar starting state; raw invalid text is retained independently of the last valid draft. */
export function PlantStartingConditionsEditor({ value, onChange, onValidityChange, inputStyle, stateIndex, label = "Starting value", description = "The output value when simulation starts or resets, in the model's output units." }: {
  value: number | undefined;
  onChange: (value: number) => void;
  onValidityChange: (message?: string) => void;
  inputStyle: React.CSSProperties;
  label?: string;
  description?: string;
  stateIndex?: number;
}) {
  const [raw, setRaw] = useState<string>();
  const [error, setError] = useState<string>();
  const field = stateIndex === undefined ? "starting_value" : `starting_state.${stateIndex}`;
  const identity = stateIndex === undefined ? "plant-starting-value" : `plant-starting-state-${stateIndex}`;
  useEffect(() => {
    if (!error) setRaw(undefined);
  }, [value, error]);

  return (
    <label style={{ display: "grid", gap: 6 }}>
      <span>{label}</span>
      <input
        name={stateIndex === undefined ? "plant-dynamics-starting-value" : `plant-dynamics-starting-state-${stateIndex}`}
        data-plant-dynamics-field={field}
        data-plant-starting-state={stateIndex}
        type="text"
        inputMode="decimal"
        value={raw ?? String(value ?? "")}
        aria-invalid={Boolean(error)}
        aria-describedby={error ? `${identity}-error` : `${identity}-help`}
        onChange={(event) => {
          const text = event.target.value;
          setRaw(text);
          const number = Number(text);
          const issue = text.trim() && Number.isFinite(number)
            ? undefined : `${stateIndex === undefined ? "Starting value" : label} must be a finite number.`;
          setError(issue);
          onValidityChange(issue);
          if (!issue) onChange(number);
        }}
        style={inputStyle}
      />
      <span id={`${identity}-help`} className="trust-help">
        {description}
      </span>
      {error && <span id={`${identity}-error`} role="alert" style={{ color: "var(--trust-danger)" }}>{error}</span>}
    </label>
  );
}
