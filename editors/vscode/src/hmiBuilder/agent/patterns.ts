import type { CompositionRole } from "./layout";

export interface CompositionPattern {
  id: "process-overview" | "equipment-detail" | "multi-unit" | "investigation" | "settings";
  version: number;
  title: string;
  task: string;
  requiredInputs: string[];
  hierarchy: Array<{ region: CompositionRole; purpose: string }>;
  spacing: { grid: number; margin: number; gap: number };
  density: string;
  limits: string[];
  visualReview: "pending";
}
const spacing = { grid: 8, margin: 32, gap: 24 };
const limits = [
  "Explicit sizes and pinned equipment are preserved.",
  "Overfull regions require more space or another page; readable sizes are never reduced.",
  "Synthetic previews do not prove runtime operation.",
];

/** Design guidance for the five supported compose IDs. Visual approval is tracked separately;
 * the catalogue must not claim that accepting a pattern ID makes its result operator-ready. */
export const COMPOSITION_PATTERNS: readonly CompositionPattern[] = [
  {
    id: "process-overview",
    version: 1,
    title: "Process overview",
    task: "Understand process flow, equipment state and the measurements that need attention.",
    requiredInputs: [
      "equipment identities and readable labels",
      "confirmed port connections, including any branches",
      "measurement bindings and engineering units",
    ],
    hierarchy: [
      { region: "process", purpose: "Dominant process area; roughly two thirds of the upper width." },
      { region: "measurements", purpose: "Supporting readings in a consistent side column." },
      { region: "trends", purpose: "Optional lower history area." },
      { region: "alarms", purpose: "Optional lower alarm summary." },
    ],
    spacing,
    density:
      "Enlarge sparse process equipment up to 2.5 times its catalogue size. Start with 1–6 primary objects; use explicit regions for branches and instruments.",
    limits,
    visualReview: "pending",
  },
  {
    id: "equipment-detail",
    version: 1,
    title: "Equipment detail",
    task: "Diagnose one equipment unit, see its measurements and reach its existing faceplate controls.",
    requiredInputs: [
      "equipment instance",
      "typed measurement and state bindings",
      "existing faceplate or navigation target",
    ],
    hierarchy: [
      { region: "process", purpose: "One primary equipment symbol or placed faceplate." },
      { region: "measurements", purpose: "Current measurements and relevant operating limits." },
      { region: "controls", purpose: "Clearly separated control or faceplate entry points." },
      { region: "trends", purpose: "Optional diagnostic history." },
    ],
    spacing,
    density: "One primary unit. Keep controls grouped and readable; place secondary history below the primary state.",
    limits,
    visualReview: "pending",
  },
  {
    id: "multi-unit",
    version: 1,
    title: "Multi-unit overview",
    task: "Compare repeated equipment units with equal visual priority and distinguish mixed states.",
    requiredInputs: [
      "unit instances",
      "matching placed faceplate definitions and equal explicit card sizes",
      "per-unit state and measurement bindings",
    ],
    hierarchy: [
      { region: "process", purpose: "Equal card grid in stable equipment order." },
      { region: "trends", purpose: "Optional shared history beneath the cards." },
      { region: "alarms", purpose: "Optional shared alarm area." },
    ],
    spacing,
    density:
      "Use matching card sizes and gaps. Split the page when the full card grid cannot fit at its declared readable size.",
    limits,
    visualReview: "pending",
  },
  {
    id: "investigation",
    version: 1,
    title: "Trends and alarms",
    task: "Investigate a disturbance by comparing trends and alarms and navigating to the affected equipment.",
    requiredInputs: ["trend pens with units, ranges and time span", "alarm scope", "equipment navigation targets"],
    hierarchy: [
      { region: "trends", purpose: "Primary trend area with a readable time axis." },
      { region: "alarms", purpose: "Adjacent alarm investigation area with equipment navigation." },
    ],
    spacing,
    density:
      "Start with one trend and one alarm list. Explicit regions can give trends more width; preserve readable table rows and axes.",
    limits,
    visualReview: "pending",
  },
  {
    id: "settings",
    version: 1,
    title: "Settings and commissioning",
    task: "Review and change grouped commissioning settings with clear units, limits and write authority.",
    requiredInputs: [
      "writable paths and directions",
      "engineering units and permitted ranges",
      "operator labels and grouping",
    ],
    hierarchy: [
      { region: "controls", purpose: "Clearly labelled editable values with units and limits." },
      { region: "measurements", purpose: "Supporting readback for the same equipment." },
    ],
    spacing,
    density:
      "Use a readable vertical settings group. For larger forms, split by operator task instead of squeezing more controls into one page.",
    limits,
    visualReview: "pending",
  },
];

export function compositionPatterns(query = ""): CompositionPattern[] {
  const term = query.toLowerCase();
  return COMPOSITION_PATTERNS.filter((pattern) =>
    `${pattern.id} ${pattern.title} ${pattern.task}`.toLowerCase().includes(term),
  ).map((pattern) => JSON.parse(JSON.stringify(pattern)) as CompositionPattern);
}
