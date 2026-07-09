import React from "react";

export function SymbolSelectionCheckbox({
  checked,
  label,
  onCheckedChange,
}: {
  checked: boolean;
  label: string;
  onCheckedChange: (checked: boolean) => void;
}) {
  return (
    <input
      type="checkbox"
      data-role="symbol-selection"
      aria-label={label}
      checked={checked}
      onChange={(event) => onCheckedChange(event.currentTarget.checked)}
      style={{ flex: "none" }}
    />
  );
}
