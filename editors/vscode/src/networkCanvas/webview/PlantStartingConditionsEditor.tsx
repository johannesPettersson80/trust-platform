import React, { useState } from "react";

import { numberText, typedNumber } from "../plant/editorSession";

/**
 * One starting state. The text the user types stays as typed and goes into the draft as the value
 * it spells; Rust's check of the initial state is shown as `issue`.
 */
export function PlantStartingConditionsEditor({ value, onChange, issue, inputStyle, stateIndex, label = "Starting value", description = "The output value when simulation starts or resets, in the model's output units." }: {
  value: unknown;
  onChange: (value: number) => void;
  issue?: string;
  inputStyle: React.CSSProperties;
  label?: string;
  description?: string;
  stateIndex?: number;
}) {
  const [text, setText] = useState<string>();
  const field = stateIndex === undefined ? "starting_value" : `starting_state.${stateIndex}`;
  const identity = stateIndex === undefined ? "plant-starting-value" : `plant-starting-state-${stateIndex}`;

  return (
    <label style={{ display: "grid", gap: 6 }}>
      <span>{label}</span>
      <input
        name={stateIndex === undefined ? "plant-dynamics-starting-value" : `plant-dynamics-starting-state-${stateIndex}`}
        data-plant-dynamics-field={field}
        data-plant-starting-state={stateIndex}
        type="text"
        inputMode="decimal"
        value={text ?? numberText(value)}
        aria-invalid={Boolean(issue)}
        aria-describedby={issue ? `${identity}-error` : `${identity}-help`}
        onChange={(event) => {
          setText(event.target.value);
          onChange(typedNumber(event.target.value));
        }}
        style={inputStyle}
      />
      <span id={`${identity}-help`} className="trust-help">
        {description}
      </span>
      {issue && <span id={`${identity}-error`} role="alert" style={{ color: "var(--trust-danger)" }}>{issue}</span>}
    </label>
  );
}
