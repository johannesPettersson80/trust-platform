import type { ControlAuxiliarySignalRole, ControlLoopDefinition } from "./loopDefinition";

export interface RecordedChannel {
  id: string;
  channel: string;
  role: ControlAuxiliarySignalRole;
  unit: string;
  value: number | null;
  quality: "good" | "forced" | "bad";
}

export function captureAdditionalChannels(
  loop: ControlLoopDefinition,
  value: (id: string) => number | null,
  forced: (id: string) => boolean
): RecordedChannel[] {
  const bindings = [
    ...(loop.signals.actuator ? [{ ...loop.signals.actuator, id: "actuator", channel: "primary", role: "actuator" as const }] : []),
    ...(loop.signals.additional ?? []),
  ];
  return bindings.map(binding => {
    const measured = value(binding.id);
    const badChannel = bindings.some(flag => flag.role === "quality" && flag.channel === binding.channel && value(flag.id) !== 1);
    return { id: binding.id, channel: binding.channel, role: binding.role, unit: binding.unit, value: measured,
      quality: forced(binding.id) ? "forced" : measured === null || badChannel ? "bad" : "good" };
  });
}

export function validRecordedChannels(value: unknown): value is readonly RecordedChannel[] {
  if (!Array.isArray(value) || value.length > 32) return false;
  const ids = new Set<string>();
  return value.every(item => {
    if (!item || typeof item !== "object" || Array.isArray(item)) return false;
    const channel = item as RecordedChannel;
    if (typeof channel.id !== "string" || !channel.id || ids.has(channel.id)) return false;
    ids.add(channel.id);
    return typeof channel.channel === "string" && !!channel.channel && typeof channel.unit === "string"
      && ["SP", "PV", "MV", "actuator", "disturbance", "state", "quality", "schedule"].includes(channel.role)
      && (channel.value === null || typeof channel.value === "number" && Number.isFinite(channel.value))
      && ["good", "forced", "bad"].includes(channel.quality);
  });
}

export function recordedChannelIdentity(channels: readonly RecordedChannel[] | undefined): string {
  return JSON.stringify((channels ?? []).map(channel => [channel.id, channel.channel, channel.role, channel.unit])
    .sort((left, right) => left[0] < right[0] ? -1 : left[0] > right[0] ? 1 : 0));
}
