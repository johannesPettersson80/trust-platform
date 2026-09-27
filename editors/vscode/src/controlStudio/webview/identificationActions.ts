export type IdentificationActionState = "initial" | "imported" | "qualified" | "running" | "preview" | "comparison" | "acceptance" | "accepted";

/** Keep the Identify pane's action row focused on one next step. */
export function setIdentificationActionState(state: IdentificationActionState): void {
  const actions = {
    import: document.querySelector<HTMLButtonElement>("[data-identify-import]"),
    check: document.querySelector<HTMLButtonElement>("[data-identify-run]"),
    fit: document.querySelector<HTMLButtonElement>("[data-identify-fit]"),
    validation: document.querySelector<HTMLButtonElement>("[data-identify-record-validation]"),
    accept: document.querySelector<HTMLButtonElement>("[data-identify-accept]"),
  };
  if (Object.values(actions).some(button => !button)) return;
  const cancel = document.querySelector<HTMLButtonElement>("[data-identify-cancel]");
  const returnFocus = cancel && document.activeElement === cancel && state !== "running";
  if (cancel) { cancel.hidden = state !== "running"; cancel.disabled = false; }

  actions.import!.hidden = false;
  setEmphasis(actions.import!, state === "initial");

  actions.check!.hidden = false;
  actions.check!.disabled = state === "initial" || state === "running";
  setEmphasis(actions.check!, state === "imported");

  actions.fit!.hidden = state !== "qualified";
  setEmphasis(actions.fit!, state === "qualified");

  actions.validation!.hidden = state !== "preview";
  setEmphasis(actions.validation!, state === "preview");

  actions.accept!.hidden = state !== "acceptance";
  setEmphasis(actions.accept!, state === "acceptance");
  if (returnFocus) (state === "qualified" ? actions.fit : actions.check)?.focus();
}

function setEmphasis(button: HTMLButtonElement, primary: boolean): void {
  button.classList.toggle("primary", primary);
  button.classList.toggle("secondary", !primary);
}
