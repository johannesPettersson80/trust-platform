// The component catalogue as the language server gives it (`trust-lsp.hmiCatalogue`, specification
// 39, §5.2, §5.6): one definition for the Components panel and the binding checks.

export interface PropertyDefinition {
  accepts: string[];
  directions: string[];
}

/** A setting's metadata from a component definition (specification 39, §8.5). */
export interface SettingDefinition {
  member: string;
  title: string;
  /** Its unit; `$Member` takes the unit from that member of the instance. */
  unit?: string;
  min?: number;
  max?: number;
  step?: number;
  choices?: Array<{ value: boolean | number | string; title: string }>;
  /** Shown in the inspector. */
  key?: boolean;
  help?: string;
}

/** A hardware terminal of a component definition. */
export interface TerminalDefinition {
  member: string;
  title: string;
  direction: "input" | "output";
  required: boolean;
  required_when?: string;
}

/** An instance a component depends on. */
export interface DependencyDefinition {
  member: string;
  title: string;
  interface: string;
  required: boolean;
  drives?: string;
}

/** One equipment definition as the language server reads it from `hmi/pairs/` (§8.5, §21.3). */
export interface EquipmentDefinition {
  block: string;
  title: string;
  symbol: string;
  panel: string;
  variants: string[];
  bindings: Record<string, string>;
  quality?: Record<string, string>;
  fields: Record<string, string>;
  settings: SettingDefinition[];
  io: TerminalDefinition[];
  dependencies: DependencyDefinition[];
  /** Where Create PLC code puts its call. */
  position?: "measurements" | "controllers" | "actuators" | null;
  /** Proposed for one of the project's own blocks: its files are written when it is first
   * placed (§21.9). */
  proposed?: boolean;
  /** For one of the project's own blocks: listed under My equipment (§21.9). */
  mine?: boolean;
}

export interface ComponentDefinition {
  equipment?: EquipmentDefinition;
  type: string;
  category: string;
  phase: number;
  size: [number, number];
  variants?: string[];
  properties?: Record<string, PropertyDefinition>;
  /** Extra bindings under names of the user's choosing, such as a trend's pens. */
  additional?: PropertyDefinition;
  /** The ports its pipes connect to (§6, item 1). */
  ports?: string[];
  /** It attaches to the equipment or the pipe it measures, as an instrument does (§6, item 5). */
  attaches?: boolean;
  /** For equipment, the heading of §5.2 it is listed under in the Components panel. */
  group?: string;
}

export interface Catalogue {
  equipment?: EquipmentDefinition[];
  format: number;
  components: ComponentDefinition[];
}
