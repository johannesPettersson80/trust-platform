import React from "react";

import type {
  PlantSignalCandidate,
  PlantSignalCatalog,
  PlantSignalDirection,
} from "../plant/signalCatalog";
import { validateManualPlantSignalAddress } from "../plant/signalCatalog";

export function PlantSignalPicker({
  catalog,
  usedFeedbackTargets,
  commandSource,
  feedbackTarget,
  onCommandSource,
  onFeedbackTarget,
}: {
  catalog: PlantSignalCatalog;
  usedFeedbackTargets: readonly string[];
  commandSource?: string;
  feedbackTarget?: string;
  onCommandSource: (address: string) => void;
  onFeedbackTarget: (address: string) => void;
}) {
  if (catalog.status === "compile_error") {
    return (
      <p role="alert" className="trust-help" style={{ marginTop: 7 }}>
        PLC signals unavailable. {catalog.error}
      </p>
    );
  }
  return (
    <div style={{ display: "grid", gap: 9, marginTop: 7 }}>
      <div className="trust-help">%Q command → plant → %I feedback</div>
      <SignalSelect
        name="plant-command-source"
        label="Command / PLC output"
        direction="%Q → plant"
        signalDirection="command"
        candidates={catalog.commands}
        catalog={catalog}
        usedFeedbackTargets={usedFeedbackTargets}
        value={commandSource}
        onChange={onCommandSource}
      />
      <SignalSelect
        name="plant-feedback-target"
        label="Feedback / PLC input"
        direction="plant → %I"
        signalDirection="feedback"
        candidates={catalog.feedbacks}
        catalog={catalog}
        usedFeedbackTargets={usedFeedbackTargets}
        value={feedbackTarget}
        onChange={onFeedbackTarget}
      />
    </div>
  );
}

function SignalSelect({
  name,
  label,
  direction,
  signalDirection,
  candidates,
  catalog,
  usedFeedbackTargets,
  value,
  onChange,
}: {
  name: string;
  label: string;
  direction: string;
  signalDirection: PlantSignalDirection;
  candidates: readonly PlantSignalCandidate[];
  catalog: PlantSignalCatalog;
  usedFeedbackTargets: readonly string[];
  value?: string;
  onChange: (address: string) => void;
}) {
  const [manualOpen, setManualOpen] = React.useState(false);
  const [manualAddress, setManualAddress] = React.useState(value ?? "");
  const [manualError, setManualError] = React.useState<string>();
  const selectRef = React.useRef<HTMLSelectElement>(null);
  const manualInputRef = React.useRef<HTMLInputElement>(null);
  const selected = candidates.find((candidate) => candidate.address === value);
  const explainError = (address: string, message: string) =>
    `${label} ${address.trim().toUpperCase()}: ${message}`;
  const applyManualAddress = () => {
    const result = validateManualPlantSignalAddress(
      catalog,
      signalDirection,
      manualAddress,
      { usedFeedbackTargets }
    );
    if (!result.ok) {
      setManualError(explainError(manualAddress, result.message));
      requestAnimationFrame(() => manualInputRef.current?.focus());
      return;
    }
    setManualAddress(result.address);
    setManualError(undefined);
    onChange(result.address);
  };
  const applyPickerAddress = (address: string) => {
    if (!address) {
      setManualError(undefined);
      onChange("");
      return;
    }
    const result = validateManualPlantSignalAddress(
      catalog,
      signalDirection,
      address,
      { usedFeedbackTargets }
    );
    if (!result.ok) {
      setManualError(explainError(address, result.message));
      requestAnimationFrame(() => selectRef.current?.focus());
      return;
    }
    setManualError(undefined);
    onChange(result.address);
  };
  return (
    <div style={{ display: "grid", gap: 4 }}>
      <label
        htmlFor={`${name}-select`}
        style={{ display: "flex", justifyContent: "space-between", gap: 8 }}
      >
        <span style={{ color: "var(--trust-text-muted)", fontSize: 10.5, fontWeight: 600 }}>{label}</span>
        <span className="trust-help">{direction}</span>
      </label>
      <select
        ref={selectRef}
        id={`${name}-select`}
        name={name}
        value={value ?? ""}
        aria-invalid={Boolean(manualError)}
        aria-describedby={manualError ? `${name}-error` : undefined}
        onChange={(event) => applyPickerAddress(event.target.value)}
        style={SELECT}
      >
        <option value="">Select a signal…</option>
        {candidates.map((candidate) => (
          <option key={candidate.address} value={candidate.address}>
            {candidate.symbol} · {candidate.address} · {candidate.dataType}
            {candidate.currentValue !== undefined ? ` · now ${candidate.currentValue}` : ""}
          </option>
        ))}
      </select>
      {selected?.currentValue !== undefined && (
        <span className="trust-help">Current value {selected.currentValue}</span>
      )}
      <button
        type="button"
        className="trust-button"
        data-plant-manual-address-toggle={name}
        aria-expanded={manualOpen}
        onClick={() => {
          setManualOpen((open) => {
            if (!open) {
              setManualAddress(value ?? "");
              setManualError(undefined);
            }
            return !open;
          });
        }}
        style={{ justifySelf: "start" }}
      >
        {manualOpen ? "Hide manual entry" : "Enter address manually…"}
      </button>
      {manualOpen && (
        <div style={{ display: "grid", gap: 4 }}>
          <div style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: 6 }}>
            <input
              ref={manualInputRef}
              name={`${name}-manual`}
              data-plant-manual-address-input={name}
              aria-label={`Manual ${label.toLowerCase()} address`}
              aria-invalid={Boolean(manualError)}
              aria-describedby={manualError ? `${name}-error` : undefined}
              value={manualAddress}
              onChange={(event) => {
                setManualAddress(event.target.value);
                setManualError(undefined);
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter") applyManualAddress();
              }}
              placeholder={signalDirection === "command" ? "%Q…" : "%I…"}
              style={SELECT}
            />
            <button
              type="button"
              className="trust-button"
              data-plant-manual-address-apply={name}
              onClick={applyManualAddress}
            >
              Use
            </button>
          </div>
        </div>
      )}
      {manualError && (
        <span
          id={`${name}-error`}
          role="alert"
          data-plant-signal-error={name}
          style={{ color: "var(--vscode-errorForeground)", fontSize: 11 }}
        >
          {manualError}
        </span>
      )}
    </div>
  );
}

const SELECT: React.CSSProperties = {
  boxSizing: "border-box",
  width: "100%",
  border: "1px solid var(--vscode-input-border, var(--trust-border))",
  borderRadius: 2,
  background: "var(--vscode-dropdown-background)",
  color: "var(--vscode-dropdown-foreground)",
  font: "inherit",
  fontSize: 11.5,
  padding: "5px 7px",
};
