import { pipeRoutes } from "../drawing/pipes";
import { drawnBox } from "../editor/editing";
import type { HmiPageModel } from "../drawing/model";

/** Shared routes are inspected, never replaced. Crossings between pipes are intentional;
 * attached instruments and endpoint equipment are excluded from body collision findings. */
export function routingQuality(page: HmiPageModel, size: [number, number]): Array<Record<string, unknown>> {
  const findings: Array<Record<string, unknown>> = [];
  const routes = pipeRoutes(page, size);
  routes.forEach((points, index) => {
    const pipe = page.pipe[index],
      ends = [pipe.from.split(".")[0], pipe.to.split(".")[0]];
    if (!points) {
      findings.push({
        severity: "error",
        rule: "disconnected-port",
        pipe: index,
        objects: ends,
        message: `Connection ${pipe.from} → ${pipe.to} has an unavailable endpoint; reconnect it to a declared port.`,
      });
      return;
    }
    for (const object of page.object) {
      if (ends.includes(object.id) || object.attach === `pipe:${index}` || ["panel", "group"].includes(object.type))
        continue;
      const box = drawnBox(object);
      const intersects = points.slice(1).some(([x, y], i) => {
        const [a, b] = points[i];
        if (x === a)
          return (
            x > box.x + 2 &&
            x < box.x + box.width - 2 &&
            Math.max(y, b) > box.y + 2 &&
            Math.min(y, b) < box.y + box.height - 2
          );
        if (y === b)
          return (
            y > box.y + 2 &&
            y < box.y + box.height - 2 &&
            Math.max(x, a) > box.x + 2 &&
            Math.min(x, a) < box.x + box.width - 2
          );
        return false;
      });
      if (intersects)
        findings.push({
          severity: "warning",
          rule: "pipe-object-collision",
          pipe: index,
          object: object.id,
          objects: [object.id],
          rectangle: box,
          message: `Pipe ${pipe.from} → ${pipe.to} crosses ${object.id}; move the object or reroute the pipe outside its body.`,
        });
      if (findings.length >= 200) return;
    }
  });
  const repeated = new Map<string, typeof page.object>();
  for (const object of page.object)
    if (object.type === "faceplate") {
      const key = object.faceplate ?? object.type;
      repeated.set(key, [...(repeated.get(key) ?? []), object]);
    }
  for (const [name, objects] of repeated)
    if (objects.length > 1 && new Set(objects.map((object) => object.size.join("x"))).size > 1)
      findings.push({
        severity: "warning",
        rule: "repeated-equipment-size",
        objects: objects.map((object) => object.id),
        message: `Repeated ${name} cards have different sizes; use equal-size or record the intentional emphasis.`,
      });
  for (const [name, objects] of repeated) {
    if (objects.length < 3) continue;
    const row = [...objects].sort((a, b) => a.at[0] - b.at[0]);
    const top = Math.min(...row.map((object) => object.at[1]));
    const bottom = Math.max(...row.map((object) => object.at[1]));
    // Only infer a row when all cards are almost aligned and do not overlap horizontally.
    // Separate process rows or deliberately stacked cards are not layout errors.
    if (bottom - top > 16 || row.slice(1).some((object, index) => object.at[0] < row[index].at[0] + row[index].size[0]))
      continue;
    if (bottom !== top)
      findings.push({
        severity: "warning",
        rule: "repeated-equipment-alignment",
        objects: row.map((object) => object.id),
        measured: { top, bottom },
        message: `Repeated ${name} cards are nearly, but not exactly, aligned; align their top edges or record the intentional offset.`,
      });
    const gaps = row.slice(1).map((object, index) => object.at[0] - row[index].at[0] - row[index].size[0]);
    if (Math.max(...gaps) - Math.min(...gaps) > 8)
      findings.push({
        severity: "warning",
        rule: "repeated-equipment-spacing",
        objects: row.map((object) => object.id),
        measured: { gaps },
        message: `Repeated ${name} cards have uneven row spacing; distribute them evenly or record the intentional grouping.`,
      });
  }
  return findings.slice(0, 200);
}
