// The editor's quality checks (specification 39, §14.1 item 4): warnings about a page's objects,
// about a project's own colours, and about a program that does not serve its screens (§18
// Phase 5). They never stop a deployment.

import { drawPage } from "../drawing";
import { collectCuts } from "../drawing/cuts";
import { DrawnText, drawnTexts, oklabDistance, textContrast } from "../drawing/contrast";
import type { HmiObjectModel, HmiPageModel } from "../drawing/model";
import { HmiColorsModel, TOKEN_THEMES } from "../drawing/tokens";
import type { Catalogue } from "./catalogue";
import { objectName } from "./objectName";
import { designValues } from "./designSamples";
import type { VariableNode } from "./messages";
import { specimenDrawings } from "./specimen";

export interface QualityWarning {
  kind: "touch-target" | "unit" | "unbound" | "cut-text" | "contrast" | "look-alike";
  /** The object it is about. */
  object?: string;
  /** The project colour it is about. */
  colour?: { theme: "dark" | "light"; token: string };
  message: string;
}

/** A project language with its text table: the project's texts over truST's words (§14.5). */
export interface LanguageTable {
  tag: string;
  name: string;
  texts: Record<string, string>;
}

/** The least width and height of a touch target (§3.5 item 2). */
const TOUCH = 44;
/** The components that show a number, which needs its unit. */
const NUMBERS = new Set(["value", "gauge", "bar", "split-bar", "slider", "numeric-input", "setpoint"]);
/** The warning and alarm colours, which data colours must not resemble (§14.1 item 3). */
const ALARM_TOKENS = ["warning", "alarm", "alarm-high", "alarm-medium", "alarm-low"];
/** Closer than this in OKLab, two colours look alike. */
const ALIKE = 0.1;
const HEX = /^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;

/** How a warning names an object: its label or text, else its id. */
function nameOf(object: HmiObjectModel): string {
  return object.label || object.text || objectName(object);
}

/** The declared type of a variable path, as the editor's variable list has it. */
export function variableType(variables: VariableNode[], path: string): string | undefined {
  if (path.includes(":")) {
    const [runtime, member] = path.split(":");
    return variables.find((node) => {
      const [owner, local] = node.path.split(":");
      return owner === runtime && local?.toLowerCase() === member.toLowerCase();
    })?.type;
  }
  let level = variables;
  let node: VariableNode | undefined;
  for (const segment of path.split(".")) {
    const match = /^([^[\]]+)((?:\[[^\]]*\])*)$/.exec(segment.trim());
    if (!match) return undefined;
    node = level.find((candidate) => candidate.name.toLowerCase() === match[1].toLowerCase());
    for (let index = (match[2].match(/\[/g) ?? []).length; node && index > 0; index -= 1) node = node.array?.element;
    if (!node) return undefined;
    level = node.children ?? [];
  }
  return node?.type;
}

/**
 * The warnings about texts the drawing cuts to fit, by object id: in each of the project's
 * languages, or as written without them (§14.1 item 4, §14.5 item 6). One warning per object and
 * language, naming the language when the project has more than one.
 */
function cutTexts(
  page: HmiPageModel,
  catalogue: Catalogue,
  languages: LanguageTable[] | undefined,
): Map<string, QualityWarning[]> {
  const right = Math.max(1, ...page.object.map((object) => object.at[0] + object.size[0]));
  const bottom = Math.max(1, ...page.object.map((object) => object.at[1] + object.size[1]));
  const values = designValues(page, catalogue);
  const tables = languages && languages.length > 0 ? languages : [undefined];
  const warned = new Map<string, QualityWarning[]>();
  for (const table of tables) {
    const frame = {
      size: [right, bottom] as [number, number],
      values,
      ...(table ? { texts: table.texts, language: table.tag } : {}),
    };
    const { cuts } = collectCuts(() => drawPage(page, frame));
    const where = table && tables.length > 1 ? ` in ${table.name}` : "";
    for (const cut of cuts) {
      const known = warned.get(cut.object) ?? [];
      if (known.some((warning) => warning.message.includes(`cut to fit${where}:`))) continue;
      const object = page.object.find((candidate) => candidate.id === cut.object);
      const shown = (words: string | undefined): string | undefined =>
        words === undefined ? undefined : (table?.texts[words] ?? words);
      const field = shown(object?.label) === cut.text ? "label" : shown(object?.text) === cut.text ? "text" : "word";
      known.push({
        kind: "cut-text",
        object: cut.object,
        message: `The ${field} "${cut.text}" is cut to fit${where}: make the object wider or the ${field} shorter.`,
      });
      warned.set(cut.object, known);
    }
  }
  return warned;
}

/** The warnings about a page's objects. A P&ID is a drawing that never runs: only its cut texts
 * are warned of (§18 phase 4 item 1). */
export function objectWarnings(
  page: HmiPageModel,
  catalogue: Catalogue,
  variables: VariableNode[],
  languages?: LanguageTable[],
  view: "page" | "pid" = "page",
): QualityWarning[] {
  const warnings: QualityWarning[] = [];
  const cut = cutTexts(page, catalogue, languages);
  for (const object of page.object) {
    if (view === "pid") {
      warnings.push(...(cut.get(object.id) ?? []));
      continue;
    }
    const component = catalogue.components.find((candidate) => candidate.type === object.type);
    const [width, height] = object.size;
    const name = nameOf(object);
    const touch = component?.category === "control" || object.type === "page-button" || object.click !== undefined;
    if (touch && (width < TOUCH || height < TOUCH)) {
      warnings.push({
        kind: "touch-target",
        object: object.id,
        message: `${name} is ${width} × ${height} px; a touch target needs at least ${TOUCH} × ${TOUCH} px.`,
      });
    }
    const bindings = Object.values(object.bind);
    const unit = object.unit !== undefined || bindings.some((binding) => binding.unit !== undefined);
    if (
      NUMBERS.has(object.type) &&
      !unit &&
      bindings.some((binding) => /^L?REAL$/i.test(variableType(variables, binding.path) ?? ""))
    ) {
      warnings.push({
        kind: "unit",
        object: object.id,
        message: `${name} shows a REAL value without a unit. Add its unit, or choose No unit if the number has none.`,
      });
    }
    const takes = Object.keys(component?.properties ?? {}).length > 0 || component?.additional !== undefined;
    const shownText = component?.category === "display" && Boolean(object.text);
    // A process symbol without bindings is a drawing, as a tank or a hand valve often is; a
    // display or control exists to show or send a value.
    const drawing = component?.category === "equipment" && object.instance === undefined;
    if (component && takes && bindings.length === 0 && !shownText && !drawing) {
      warnings.push({
        kind: "unbound",
        object: object.id,
        message: `${name} is not bound to a variable, so it shows nothing in the runtime.`,
      });
    }
    warnings.push(...(cut.get(object.id) ?? []));
  }
  return warnings;
}

/** Each text of the specimen with what lies behind it, measured once per catalogue. */
const specimenTexts = new WeakMap<Catalogue, DrawnText[]>();

function textsOfSpecimen(catalogue: Catalogue): DrawnText[] {
  let texts = specimenTexts.get(catalogue);
  if (!texts) {
    // The controls of a read-only page are inactive and need no contrast (WCAG 2.1, 1.4.3).
    texts = specimenDrawings(catalogue)
      .filter((drawing) => drawing.state !== "read-only")
      .flatMap((drawing) => drawnTexts(drawing.svg));
    specimenTexts.set(catalogue, texts);
  }
  return texts;
}

/** The warnings about a project's own colours, each at its theme and token. */
export function colourWarnings(colors: HmiColorsModel, catalogue: Catalogue): QualityWarning[] {
  const warnings: QualityWarning[] = [];
  for (const theme of ["dark", "light"] as const) {
    const defaults = TOKEN_THEMES[theme];
    const own = Object.fromEntries(
      Object.entries(colors[theme] ?? {}).filter(
        ([token, colour]) => token !== "font" && token in defaults && HEX.test(colour),
      ),
    );
    if (Object.keys(own).length === 0) continue;
    const palette = { ...defaults, ...own };
    // Low contrast: the worst text each project colour takes part in.
    const worst = new Map<string, { drawn: DrawnText; ratio: number; needed: number }>();
    for (const drawn of textsOfSpecimen(catalogue)) {
      const { ratio, needed } = textContrast(drawn, palette);
      if (ratio >= needed) continue;
      const involved = new Set([
        drawn.colour,
        ...drawn.behind.flat().flatMap((backing) => [backing.token, ...(backing.mix ? [backing.mix.token] : [])]),
      ]);
      for (const token of involved) {
        if (!(token in own)) continue;
        const known = worst.get(token);
        if (!known || ratio / needed < known.ratio / known.needed) worst.set(token, { drawn, ratio, needed });
      }
    }
    for (const [token, { drawn, ratio, needed }] of worst) {
      const what = drawn.sign ? `the sign "${drawn.text}" in "${drawn.colour}"` : `text in "${drawn.colour}"`;
      const behind = `"${drawn.behind[0]?.[0]?.token ?? "page"}"`;
      warnings.push({
        kind: "contrast",
        colour: { theme, token },
        message: `With this colour, ${what} on ${behind} has a contrast of ${ratio.toFixed(2)}:1 in the ${theme} theme; it needs ${needed}:1.`,
      });
    }
    // Colours that look alike: data in the warning's or an alarm's colour.
    for (const [token, colour] of Object.entries(own)) {
      if (ALARM_TOKENS.includes(token) || token === "stale") continue;
      const nearest = ALARM_TOKENS.map((alarm) => ({ alarm, distance: oklabDistance(colour, palette[alarm]) })).sort(
        (a, b) => a.distance - b.distance,
      )[0];
      if (nearest && nearest.distance < ALIKE) {
        const named =
          nearest.alarm === "warning"
            ? "warning"
            : nearest.alarm === "alarm"
              ? "alarm"
              : `${nearest.alarm.replace("alarm-", "")}-priority alarm`;
        warnings.push({
          kind: "look-alike",
          colour: { theme, token },
          message: `${token} looks like the ${named} colour in the ${theme} theme (${nearest.distance.toFixed(2)} apart), so data could be taken for an alarm.`,
        });
      }
    }
  }
  return warnings;
}

/** The block a program calls in a task so that the screens get their values (§18 Phase 5). */
const DISPLAY_BLOCK = "hmi.display";

/** A warning when the program has no `Hmi.Display` block, so the screens would show no values;
 * none while the program has no variables, as when it does not compile. */
export function displayWarning(variables: VariableNode[]): string | undefined {
  if (variables.length === 0) return undefined;
  const found = (nodes: VariableNode[]): boolean =>
    nodes.some((node) => node.type.toLowerCase() === DISPLAY_BLOCK || found(node.children ?? []));
  return found(variables)
    ? undefined
    : "No program calls Hmi.Display: call it in a task to show live values on the screens.";
}
