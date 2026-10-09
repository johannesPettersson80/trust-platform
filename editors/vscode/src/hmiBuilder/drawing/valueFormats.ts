// Read-only display defaults (§5.9): the source model and command constraints never change.
import type { DrawFrame, HmiObjectModel } from "./model";

/** IEC path matching follows the runtime's ASCII case-insensitive member lookup. */
export function formatPath(path: string): string {
  return path.replace(/[A-Z]/g, (letter) => letter.toLowerCase());
}

/** A drawing copy with inherited settings; explicit object properties always win. */
export function withValueFormat(
  object: HmiObjectModel,
  frame: Pick<DrawFrame, "formats"> & Partial<Pick<DrawFrame, "values">>,
  property = "value",
): HmiObjectModel {
  const binding = object.bind[property];
  if (!binding) return object;
  const format = frame.formats?.[formatPath(binding.path)];
  const equipment = frame.values?.[binding.path]?.presentation;
  const unit = object.unit ?? binding.unit ?? equipment?.unit ?? format?.unit;
  const decimals = object.decimals ?? format?.decimals;
  // Only a single-value scale inherits engineering endpoints; controls keep authored constraints.
  const range = object.type === "bar" || object.type === "gauge" ? equipment?.range : undefined;
  const min = object.min ?? range?.[0],
    max = object.max ?? range?.[1];
  if (unit === object.unit && decimals === object.decimals && min === object.min && max === object.max) return object;
  return {
    ...object,
    ...(unit !== undefined ? { unit } : {}),
    ...(decimals !== undefined ? { decimals } : {}),
    ...(min !== undefined ? { min } : {}),
    ...(max !== undefined ? { max } : {}),
  };
}
