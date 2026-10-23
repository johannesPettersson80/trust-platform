import React from "react";

export function SymbolSelectionCheckbox({
  checked,
  disabled = false,
  label,
  onCheckedChange,
}: {
  checked: boolean;
  disabled?: boolean;
  label: string;
  onCheckedChange: (checked: boolean) => void;
}) {
  return (
    <input
      type="checkbox"
      data-role="symbol-selection"
      aria-label={label}
      checked={checked}
      disabled={disabled}
      onChange={(event) => onCheckedChange(event.currentTarget.checked)}
      style={{ flex: "none" }}
    />
  );
}
