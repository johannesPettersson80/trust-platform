// Exists because the Bode and Nichols plots showed gain and phase margin only as text in
// the design-row chip, with no 0 dB / -180 degree reference and no crossover marker on
// the plot itself (finding V-05). This module computes the plain-data geometry for those
// reference lines and markers so chart.ts only has to wire them into ECharts options.

import { formatEngineeringNumber } from "../chartContract";

export interface CrossoverPoint {
  frequencyRadPerSecond: number;
  magnitudeDb: number;
  phaseDegrees: number;
}

export interface MarginMarkerInput {
  gainMarginDb: number;
  phaseMarginDegrees: number;
  gainCrossover: CrossoverPoint;
  phaseCrossover: CrossoverPoint | undefined;
}

export interface MarkLineDatum {
  name: string;
  xAxis?: number;
  yAxis?: number;
}

export interface MarkPointDatum {
  name: string;
  coord: readonly [number, number];
  value: string;
}

/** A resolved gain margin is required before a "GM" marker means anything; an unbounded
 * margin (no phase crossover in the sampled range) draws only the 0 dB reference. */
function hasResolvedGainMargin(input: MarginMarkerInput): input is MarginMarkerInput & {
  phaseCrossover: CrossoverPoint;
} {
  return input.phaseCrossover !== undefined && Number.isFinite(input.gainMarginDb);
}

/** 0 dB reference plus (when resolved) a vertical line at the phase-crossover frequency
 * used to read gain margin, for the magnitude panel. */
export function magnitudeReferenceLines(input: MarginMarkerInput): MarkLineDatum[] {
  const lines: MarkLineDatum[] = [{ name: "0 dB", yAxis: 0 }];
  if (hasResolvedGainMargin(input)) {
    lines.push({ name: "Phase crossover", xAxis: input.phaseCrossover.frequencyRadPerSecond });
  }
  return lines;
}

/** The point on the candidate open-loop curve where gain margin is read: the phase
 * crossover frequency, at its own (negative-of-margin) magnitude. */
export function magnitudeCrossoverMarkers(input: MarginMarkerInput): MarkPointDatum[] {
  if (!hasResolvedGainMargin(input)) return [];
  return [{
    name: "Gain margin",
    coord: [input.phaseCrossover.frequencyRadPerSecond, input.phaseCrossover.magnitudeDb],
    value: `GM ${formatEngineeringNumber(input.gainMarginDb)} dB`,
  }];
}

/** -180 degree reference plus a vertical line at the gain-crossover frequency used to
 * read phase margin, for the phase panel. */
export function phaseReferenceLines(input: MarginMarkerInput): MarkLineDatum[] {
  return [
    { name: "−180°", yAxis: -180 },
    { name: "Gain crossover", xAxis: input.gainCrossover.frequencyRadPerSecond },
  ];
}

/** The point on the candidate open-loop curve where phase margin is read: the gain
 * crossover frequency (0 dB), at its own phase. */
export function phaseCrossoverMarkers(input: MarginMarkerInput): MarkPointDatum[] {
  return [{
    name: "Phase margin",
    coord: [input.gainCrossover.frequencyRadPerSecond, input.gainCrossover.phaseDegrees],
    value: `PM ${formatEngineeringNumber(input.phaseMarginDegrees)}°`,
  }];
}

/** 0 dB and -180 degree references for the Nichols plot (open-loop phase on x, magnitude
 * on y). */
export function nicholsReferenceLines(): MarkLineDatum[] {
  return [{ name: "0 dB", yAxis: 0 }, { name: "−180°", xAxis: -180 }];
}

/** Gain- and (when resolved) phase-margin points plotted directly on the Nichols curve. */
export function nicholsCrossoverMarkers(input: MarginMarkerInput): MarkPointDatum[] {
  const markers: MarkPointDatum[] = [{
    name: "Phase margin",
    coord: [input.gainCrossover.phaseDegrees, input.gainCrossover.magnitudeDb],
    value: `PM ${formatEngineeringNumber(input.phaseMarginDegrees)}°`,
  }];
  if (hasResolvedGainMargin(input)) {
    markers.push({
      name: "Gain margin",
      coord: [input.phaseCrossover.phaseDegrees, input.phaseCrossover.magnitudeDb],
      value: `GM ${formatEngineeringNumber(input.gainMarginDb)} dB`,
    });
  }
  return markers;
}
