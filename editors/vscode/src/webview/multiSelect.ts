let triggerSequence = 0;

export interface MultiSelectControls {
  readonly trigger: HTMLButtonElement;
  setDisabled(disabled: boolean): void;
}

/** Wraps a native multi-select with an keyboard-accessible checkbox dropdown. */
export function initializeMultiSelect(select: HTMLSelectElement): MultiSelectControls {
  const document = select.ownerDocument;
  const trigger = document.createElement("button");
  trigger.type = "button";
  trigger.className = "trust-button trust-multi-select-trigger";
  trigger.dataset.multiSelectTrigger = "true";
  trigger.id = `${select.id || "trust-multi-select"}-trigger-${++triggerSequence}`;
  trigger.setAttribute("aria-haspopup", "dialog");
  trigger.setAttribute("aria-expanded", "false");
  trigger.setAttribute("aria-label", select.getAttribute("aria-label") ?? "Select signals");
  const associatedLabel = select.labels?.[0];
  if (associatedLabel) associatedLabel.htmlFor = trigger.id;
  select.hidden = true;
  select.classList.add("trust-multi-select-native");
  select.tabIndex = -1;
  select.setAttribute("aria-hidden", "true");
  select.insertAdjacentElement("afterend", trigger);

  const dialog = document.createElement("dialog");
  dialog.className = "trust-multi-select-dialog";
  dialog.dataset.multiSelectDialog = "true";
  dialog.setAttribute("aria-label", select.getAttribute("aria-label") ?? "Select signals");
  const options = document.createElement("div");
  options.className = "trust-multi-select-options";
  dialog.append(options); document.body.append(dialog);
  let disabled = false;

  const optionText = (option: HTMLOptionElement): string => {
    const unit = option.dataset.unit;
    return unit ? `${option.textContent?.trim() ?? option.value} (${unit})` : option.textContent?.trim() ?? option.value;
  };
  const renderTrigger = (): void => {
    const selected = Array.from(select.options).filter(option => option.selected).map(optionText);
    trigger.textContent = selected.length ? selected.join(", ") : "Select signals";
    trigger.title = selected.join(", ");
    trigger.setAttribute("aria-label", `${select.getAttribute("aria-label") ?? "Select signals"}: ${selected.join(", ") || "None selected"}`);
    trigger.disabled = disabled || select.disabled;
  };
  const close = (restoreFocus: boolean): void => {
    if (!dialog.open) return;
    if (typeof dialog.close === "function") dialog.close(); else dialog.open = false;
    trigger.setAttribute("aria-expanded", "false");
    if (restoreFocus) trigger.focus();
  };
  const positionDialog = (): void => {
    const bounds = trigger.getBoundingClientRect();
    const view = document.defaultView;
    if (view) {
      dialog.style.width = `${Math.min(Math.max(bounds.width, 260), view.innerWidth - 24)}px`;
      dialog.style.left = `${Math.max(12, Math.min(bounds.left, view.innerWidth - Math.min(Math.max(bounds.width, 260), view.innerWidth - 24) - 12))}px`;
      dialog.style.top = `${Math.max(12, Math.min(bounds.bottom + 4, view.innerHeight - 320))}px`;
    }
  };
  const open = (): void => {
    if (disabled || select.disabled) return;
    options.replaceChildren(...Array.from(select.options).map(option => {
      const label = document.createElement("label"); label.className = "trust-field trust-multi-select-option";
      const checkbox = document.createElement("input"); checkbox.type = "checkbox"; checkbox.checked = option.selected; checkbox.disabled = option.disabled; checkbox.dataset.multiSelectOption = option.value;
      checkbox.addEventListener("change", () => {
        if (disabled || select.disabled) { close(false); return; }
        option.selected = checkbox.checked;
        select.dispatchEvent(new (document.defaultView?.Event ?? Event)("change", { bubbles: true }));
        renderTrigger();
      });
      checkbox.addEventListener("keydown", event => {
        if (event.key === " ") { event.preventDefault(); checkbox.click(); }
      });
      label.append(checkbox, document.createTextNode(optionText(option))); return label;
    }));
    positionDialog();
    if (typeof dialog.show === "function") dialog.show(); else dialog.open = true;
    trigger.setAttribute("aria-expanded", "true");
    (options.querySelector<HTMLInputElement>("input:not(:disabled)") ?? trigger).focus();
  };
  trigger.addEventListener("click", () => dialog.open ? close(true) : open());
  trigger.addEventListener("keydown", event => {
    if ([" ", "ArrowDown", "ArrowUp"].includes(event.key)) { event.preventDefault(); open(); }
    if (event.key === "Escape") { event.preventDefault(); close(true); }
  });
  dialog.addEventListener("keydown", event => {
    if (event.key === "Escape") { event.preventDefault(); close(true); }
    if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      const choices = Array.from(options.querySelectorAll<HTMLInputElement>("input:not(:disabled)"));
      const current = choices.indexOf(document.activeElement as HTMLInputElement);
      const next = event.key === "Home" ? 0 : event.key === "End" ? choices.length - 1 : (current + (event.key === "ArrowDown" ? 1 : -1) + choices.length) % choices.length;
      choices[next]?.focus();
    }
  });
  dialog.addEventListener("cancel", event => { event.preventDefault(); close(true); });
  const outside = (event: Event) => {
    const target = event.target as Node | null;
    if (target && !dialog.contains(target) && !trigger.contains(target)) close(false);
  };
  document.addEventListener("pointerdown", outside);
  document.addEventListener("focusin", outside);
  select.addEventListener("change", renderTrigger);
  document.defaultView?.addEventListener("resize", () => { if (dialog.open) positionDialog(); });
  const Observer = document.defaultView?.MutationObserver;
  if (Observer) new Observer(() => {
    if ((disabled || select.disabled) && dialog.open) close(false);
    renderTrigger();
  }).observe(select, { attributes: true, childList: true, subtree: true });
  renderTrigger();

  return {
    trigger,
    setDisabled(value: boolean): void {
      disabled = value; trigger.disabled = value;
      for (const control of dialog.querySelectorAll<HTMLButtonElement | HTMLInputElement>("button,input")) control.disabled = value;
      if (value && dialog.open) close(false);
    },
  };
}
