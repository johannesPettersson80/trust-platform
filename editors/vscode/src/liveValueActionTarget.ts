export type LiveValueActionTarget =
  | { readonly kind: "global"; readonly name: string }
  | { readonly kind: "io"; readonly address: string };

const GLOBAL_PREFIX = "global:";

export function liveValueActionTarget(address: string): LiveValueActionTarget {
  if (address.startsWith(GLOBAL_PREFIX) && address.length > GLOBAL_PREFIX.length) {
    return { kind: "global", name: address.slice(GLOBAL_PREFIX.length) };
  }
  return { kind: "io", address };
}
