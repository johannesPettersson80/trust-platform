// Group the comm.schema protocol catalog by its own `category` so the Add picker reads as three
// deliberate shelves — Field devices / Supervisory services / Peer links — instead of one flat wall.
// Pure + framework-free so it is unit-testable without a DOM (see network-canvas.test.ts).

export const CATEGORY_ORDER: ReadonlyArray<{ key: string; label: string }> = [
  { key: "field_device", label: "Field devices" },
  { key: "supervisory_service", label: "Supervisory services" },
  { key: "peer_link", label: "Peer links" },
];

const CATEGORY_KEYS = new Set(CATEGORY_ORDER.map((c) => c.key));

export interface CategoryGroup<T> {
  key: string;
  label: string;
  items: T[];
}

/**
 * Group `items` by their `category`, preserving input order within each group and the canonical
 * Field → Supervisory → Peer order across groups. Anything with a missing/unknown category lands in a
 * trailing "Other" group so nothing is ever silently dropped. Empty groups are omitted.
 */
export function groupByCategory<T extends { category?: string | null }>(items: T[]): CategoryGroup<T>[] {
  const byCat = new Map<string, T[]>();
  for (const item of items) {
    const key = item.category && CATEGORY_KEYS.has(item.category) ? item.category : "other";
    const list = byCat.get(key);
    if (list) list.push(item);
    else byCat.set(key, [item]);
  }
  const ordered: CategoryGroup<T>[] = CATEGORY_ORDER.map((c) => ({
    key: c.key,
    label: c.label,
    items: byCat.get(c.key) ?? [],
  }));
  const other = byCat.get("other") ?? [];
  if (other.length) ordered.push({ key: "other", label: "Other", items: other });
  return ordered.filter((g) => g.items.length > 0);
}
