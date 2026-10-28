/**
 * The model select of the Recorded-data fit (specification 35, B3.3). The engine offers the models with their words and says which of them the
 * page may enable (`modelChoices`); the page lists the options from those choices and keeps no table of model words, so a model the engine
 * offers is always listed, under the engine's label.
 */

/** One model of the select as the engine offers it: its value, whether the page may enable it, and its words. */
export interface ModelChoiceView {
  readonly model: string;
  readonly enabled: boolean;
  readonly label?: string | null;
}

/**
 * Lists `choices` as the options of `select`, each under the engine's label (the model's name when the engine gives none), and disables the
 * options the engine does not let the page enable. The model the select shows stays while the engine still offers it. An answer that offers no
 * choice leaves the options as they are and enables every one.
 */
export function showModelChoices(select: HTMLSelectElement, choices: readonly ModelChoiceView[]): void {
  if (choices.length === 0) {
    for (const option of Array.from(select.options)) option.disabled = false;
    return;
  }
  const words = (choice: ModelChoiceView): string => choice.label ?? choice.model;
  const listed = Array.from(select.options);
  const unchanged = listed.length === choices.length
    && listed.every((option, index) => option.value === choices[index].model && option.textContent === words(choices[index]));
  if (!unchanged) {
    const kept = select.value;
    select.replaceChildren(...choices.map(choice => {
      const option = select.ownerDocument.createElement("option");
      option.value = choice.model;
      option.textContent = words(choice);
      return option;
    }));
    if (choices.some(choice => choice.model === kept)) select.value = kept;
  }
  Array.from(select.options).forEach((option, index) => { option.disabled = !choices[index].enabled; });
}
