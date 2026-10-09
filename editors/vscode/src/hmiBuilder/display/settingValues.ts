// Settings in the form the runtime's settings route and the builder's inspector share
// (specification 39, §21.4; Plant §10): each setting with its plain title, its unit, its limits or
// choices, an optional help line, its kind of value and its value in its unit (a duration as a
// number in `ms`, `s`, `min` or `h`; a setting with choices as the choice's value). Nothing here is
// specific to one kind of equipment.

/** The kind of value a setting holds. */
export type SettingKind = "bool" | "integer" | "real" | "time" | "text" | "other";

/** A value a setting holds or one of its choices. */
export type SettingValue = boolean | number | string;

/** One allowed value of a setting, with its words. */
export interface SettingChoice {
  value: SettingValue;
  title: string;
}

/** A setting as the runtime and the inspector give it. */
export interface Setting {
  member: string;
  title: string;
  unit?: string;
  min?: number;
  max?: number;
  step?: number;
  choices?: SettingChoice[];
  help?: string;
  key?: boolean;
  kind: SettingKind;
  /** In the setting's unit; `null` when the runtime cannot read it. */
  value: SettingValue | null;
}

/** What the settings route answers: the settings, or why not, with the setting it is about and
 * whether the values changed since they were read. */
export interface SettingsReply {
  ok: boolean;
  instance?: string;
  settings?: Setting[];
  error?: string;
  field?: string;
  changed?: boolean;
}

/** A setting's field label: its title with its unit, "Start timeout (s)". */
export function settingLabel(setting: Setting): string {
  return setting.unit ? `${setting.title} (${setting.unit})` : setting.title;
}

/** The text a field shows for a value. */
export function settingText(value: Setting["value"]): string {
  return value === null ? "" : String(value);
}

/** A limit as a refusal says it, with the setting's unit. */
function limit(value: number, unit: string | undefined): string {
  return unit ? `${value} ${unit}` : String(value);
}

/**
 * The value entered in a setting's field as the runtime takes it, or an error naming the setting:
 * one of its choices' values when it has choices; TRUE or FALSE; a finite number within its
 * limits, in its unit, for a number or a duration (whole for an integer); otherwise the text.
 */
export function enteredValue(setting: Setting, entered: string | boolean): SettingValue {
  if (setting.choices?.length) {
    const choice = setting.choices.find((candidate) => String(candidate.value) === String(entered));
    if (!choice) throw new Error(`${setting.title}: choose one of its values.`);
    return choice.value;
  }
  if (setting.kind === "bool") return entered === true || entered === "true";
  if (setting.kind !== "integer" && setting.kind !== "real" && setting.kind !== "time") return String(entered);
  const text = String(entered).trim().replace(",", ".");
  const number = Number(text);
  if (text === "" || !Number.isFinite(number)) throw new Error(`${setting.title}: enter a number.`);
  if (setting.kind === "integer" && !Number.isInteger(number))
    throw new Error(`${setting.title}: enter a whole number.`);
  if (setting.min !== undefined && number < setting.min)
    throw new Error(`${setting.title}: ${number} is below the minimum ${limit(setting.min, setting.unit)}.`);
  if (setting.max !== undefined && number > setting.max)
    throw new Error(`${setting.title}: ${number} is above the maximum ${limit(setting.max, setting.unit)}.`);
  return number;
}

/** Whether a value differs from the setting's value. A field left as shown gives the same number,
 * text or flag back, so an unchanged setting is never sent. */
export function settingChanged(setting: Setting, value: SettingValue): boolean {
  return value !== setting.value;
}
