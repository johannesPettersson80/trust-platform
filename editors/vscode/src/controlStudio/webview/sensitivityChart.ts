import { engineeringLine } from "./chartPrimitives";

export interface SensitivityPoint { frequencyRadPerSecond: number; sensitivityMagnitudeDb: number; complementarySensitivityMagnitudeDb: number; controlSensitivityMagnitudeDb: number }
export interface SensitivityColors { sensitivity: string; complementary: string; control: string }

/** |S|, |T| and |CS| in dB against frequency, each with its own theme colour and stroke. */
export function sensitivitySeries(points: readonly SensitivityPoint[], colors: SensitivityColors): Record<string, unknown>[] {
  const line = (name: string, color: string, stroke: "solid" | "dashed" | "dotted", value: (point: SensitivityPoint) => number) => {
    const base = engineeringLine(name, 0, points.map(point => [point.frequencyRadPerSecond, value(point)]), stroke);
    return { ...base, lineStyle: { ...base.lineStyle, color }, itemStyle: { color } };
  };
  return [
    line("S", colors.sensitivity, "solid", point => point.sensitivityMagnitudeDb),
    line("T", colors.complementary, "dashed", point => point.complementarySensitivityMagnitudeDb),
    line("CS", colors.control, "dotted", point => point.controlSensitivityMagnitudeDb),
  ];
}
