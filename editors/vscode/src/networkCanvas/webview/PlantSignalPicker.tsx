import React from "react";

import type {
  PlantSignalCandidate,
  PlantSignalCatalog,
  PlantSignalDirection,
} from "../plant/signalCatalog";

/** Asks Rust to bind a port to an address (empty clears it); the reason it was refused, if it was. */
type BindPort = (direction: PlantSignalDirection, index: number, address: string) => Promise<string | undefined>;

/**
 * The plant's ports and the compiled PLC signals they bind to (specification 47 §8). Rust checks
 * each chosen or typed address and binds it in its compiled spelling.
 */
export function PlantSignalPicker({
  catalog,
  commandSources,
  feedbackTargets,
  labels,
  onBind,
}: {
  catalog: PlantSignalCatalog;
  commandSources: readonly string[];
  feedbackTargets: readonly string[];
  labels: { commands: string[]; feedbacks: string[] };
  onBind: BindPort;
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
      {labels.commands.map((label, index) => <SignalSelect
        key={`command-${index}`}
        name={`plant-command-source${index === 0 ? "" : `-${index}`}`}
        label={label}
        portIndex={index}
        direction="%Q → plant"
        signalDirection="command"
        candidates={catalog.commands}
        value={commandSources[index]}
        onBind={(address) => onBind("command", index, address)}
      />)}
      {labels.feedbacks.map((label, index) => <SignalSelect
        key={`feedback-${index}`}
        name={`plant-feedback-target${index === 0 ? "" : `-${index}`}`}
        label={label}
        portIndex={index}
        direction="plant → %I"
        signalDirection="feedback"
        candidates={catalog.feedbacks}
        value={feedbackTargets[index]}
        onBind={(address) => onBind("feedback", index, address)}
      />)}
    </div>
  );
}

function SignalSelect({
  name,
  label,
  direction,
  signalDirection,
  portIndex,
  candidates,
  value,
  onBind,
}: {
  name: string;
  label: string;
  direction: string;
  signalDirection: PlantSignalDirection;
  portIndex: number;
  candidates: readonly PlantSignalCandidate[];
  value?: string;
  onBind: (address: string) => Promise<string | undefined>;
}) {
  const [manualOpen, setManualOpen] = React.useState(false);
  const [manualAddress, setManualAddress] = React.useState(value ?? "");
  const [manualError, setManualError] = React.useState<string>();
  // The choice shows at once while Rust binds it.
  const [choosing, setChoosing] = React.useState<string>();
  const selectRef = React.useRef<HTMLSelectElement>(null);
  const manualInputRef = React.useRef<HTMLInputElement>(null);
  const selected = candidates.find((candidate) => candidate.address === value);
  // A bound address shows in its compiled spelling once Rust has bound it.
  React.useEffect(() => setManualAddress(value ?? ""), [value]);
  const bind = async (address: string, focus: () => HTMLElement | null) => {
    const refusal = await onBind(address);
    setManualError(refusal);
    if (refusal) requestAnimationFrame(() => focus()?.focus());
  };
  const applyManualAddress = () => void bind(manualAddress, () => manualInputRef.current);
  const applyPickerAddress = (address: string) => {
    setChoosing(address);
    void bind(address, () => selectRef.current).finally(() => setChoosing(undefined));
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
        data-plant-port-direction={signalDirection}
        data-plant-port-index={portIndex}
        value={choosing ?? value ?? ""}
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
              placeholder={signalDirection === "command" ? "%Q… or %M…" : "%I…"}
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
