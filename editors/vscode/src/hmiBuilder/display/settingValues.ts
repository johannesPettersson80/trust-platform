// Settings in the form the runtime's settings route and the builder's inspector share
// (specification 39, §21.4; Plant §10): each setting with its plain title, its unit, its limits or
// choices, an optional help line, its kind of value and its value in its unit (a duration as a
// number in `ms`, `s`, `min` or `h`; a setting with choices as the choice's value). Nothing here is
// specific to one kind of equipment. The runtime reads and checks what is entered (§21.4 item 4).

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
