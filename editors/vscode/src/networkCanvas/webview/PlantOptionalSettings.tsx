import React from "react";

import type { PlantDraft } from "../plant/addFlow";

export type PlantOptionalDestination = "actuator" | "signals" | "realism" | "scenarios";

interface PlantOptionalSettingsProps {
  draft: PlantDraft;
  active?: PlantOptionalDestination;
  onActiveChange: (destination?: PlantOptionalDestination) => void;
}

const DESTINATIONS: ReadonlyArray<{
  id: PlantOptionalDestination;
  title: string;
  description: string;
}> = [
  {
    id: "actuator",
    title: "Actuator & command",
    description: "Limits and response between the PLC command and the plant.",
  },
  {
    id: "signals",
    title: "Signal conversions",
    description: "Scale, offset, initial command, and simulation step settings.",
  },
  {
    id: "realism",
    title: "Realism",
    description: "Optional process and sensor imperfections, applied in their displayed order.",
  },
  {
    id: "scenarios",
    title: "Scenarios",
    description: "Scheduled changes and faults in simulation time.",
  },
];

export function PlantOptionalSettings({ draft, active, onActiveChange }: PlantOptionalSettingsProps) {
  return (
    <section className="trust-section" data-plant-optional-settings>
      <div className="trust-section__title">Optional settings</div>
      <p className="trust-help" style={{ marginTop: 0 }}>
        Open only the task you need. Configured settings stay summarized here when the editor is closed.
      </p>
      <div style={DESTINATION_GRID}>
        {DESTINATIONS.map((destination) => {
          const open = active === destination.id;
          return (
            <article
              key={destination.id}
              data-plant-optional-destination={destination.id}
              style={DESTINATION_CARD}
            >
              <div style={{ minWidth: 0 }}>
                <strong style={{ display: "block", fontSize: 12 }}>{destination.title}</strong>
                <span className="trust-help" style={{ display: "block", marginTop: 2 }}>
                  {destination.description}
                </span>
                <span
                  data-plant-optional-summary={destination.id}
                  style={{ display: "block", marginTop: 5, color: "var(--trust-text)", fontSize: 11 }}
                >
                  {summaryFor(destination.id, draft)}
                </span>
              </div>
              <button
                type="button"
                className="trust-button"
                data-plant-optional-open={destination.id}
                aria-expanded={open}
                onClick={() => onActiveChange(open ? undefined : destination.id)}
                style={{ alignSelf: "center", whiteSpace: "nowrap" }}
              >
                {open ? "Close" : "Edit"}
              </button>
            </article>
          );
        })}
      </div>
    </section>
  );
}

function summaryFor(destination: PlantOptionalDestination, draft: PlantDraft): string {
  switch (destination) {
    case "actuator": return actuatorSummary(draft);
    case "signals": return signalSummary(draft);
    case "realism": return realismSummary(draft);
    case "scenarios": return scenarioSummary(draft);
  }
}

function actuatorSummary(draft: PlantDraft): string {
  const actuator = draft.actuator ?? {};
  const configured = Object.entries(actuator).filter(([, value]) => value !== undefined);
  if (configured.length === 0) return "No actuator limits or response effects";
  const details: string[] = [];
  if (actuator.minimum !== undefined || actuator.maximum !== undefined) {
    if (actuator.minimum !== undefined && actuator.maximum !== undefined) {
      details.push(`Range ${formatNumber(actuator.minimum)}–${formatNumber(actuator.maximum)}`);
    } else if (actuator.minimum !== undefined) {
      details.push(`Minimum ${formatNumber(actuator.minimum)}`);
    } else if (actuator.maximum !== undefined) {
      details.push(`Maximum ${formatNumber(actuator.maximum)}`);
    }
  }
  details.push(`${configured.length} configured setting${configured.length === 1 ? "" : "s"}`);
  return details.join(" · ");
}

function signalSummary(draft: PlantDraft): string {
  const nonIdentity = [
    ...(draft.inputScale ?? []).filter((value) => value !== 1),
    ...(draft.inputOffset ?? []).filter((value) => value !== 0),
    ...(draft.outputScale ?? []).filter((value) => value !== 1),
    ...(draft.outputOffset ?? []).filter((value) => value !== 0),
  ].length;
  const initialInput = draft.initialInput ?? [];
  const nonZeroInitial = initialInput.filter((value) => value !== 0).length;
  const conversions = nonIdentity === 0
    ? "No signal scaling or offset"
    : `${nonIdentity} scaling or offset value${nonIdentity === 1 ? "" : "s"} changed`;
  const initial = nonZeroInitial === 0
    ? "initial command 0"
    : `${nonZeroInitial} nonzero initial command${nonZeroInitial === 1 ? "" : "s"}`;
  const step = draft.stepMs === undefined ? "project simulation step" : `${draft.stepMs} ms plant step`;
  return `${conversions} · ${initial} · ${step}`;
}

function realismSummary(draft: PlantDraft): string {
  const configured = draft.uncertainties ?? [];
  if (configured.length === 0) return "No process or sensor imperfections";
  const enabled = configured.filter((effect) => effect.enabled);
  const counts = { actuator: 0, process: 0, sensor: 0 };
  for (const effect of configured) {
    if (effect.target === "command") counts.actuator += 1;
    else if (effect.target === "measurement") counts.sensor += 1;
    else counts.process += 1;
  }
  return [
    `${configured.length} configured effect${configured.length === 1 ? "" : "s"}`,
    `${enabled.length} enabled effect${enabled.length === 1 ? "" : "s"}`,
    `Actuator ${counts.actuator}`,
    `Process ${counts.process}`,
    `Sensor ${counts.sensor}`,
  ].join(" · ");
}

function scenarioSummary(draft: PlantDraft): string {
  const configured = draft.scenarioEvents ?? [];
  if (configured.length === 0) return "No scheduled scenarios";
  const included = configured.filter((event) => event.enabled);
  const summary = `${configured.length} configured event${configured.length === 1 ? "" : "s"} · ${included.length} included event${included.length === 1 ? "" : "s"}`;
  if (included.length === 0) return summary;
  if (included.some((event) => !Number.isSafeInteger(event.atMs) || event.atMs < 0)) {
    return `${summary} · Correct event time in Scenarios`;
  }
  const firstAtSeconds = Math.min(...included.map((event) => event.atMs)) / 1_000;
  return `${summary} · first at ${formatNumber(firstAtSeconds)} s`;
}

function formatNumber(value: number): string {
  return Number.isInteger(value) ? String(value) : String(Number(value.toPrecision(8)));
}

const DESTINATION_GRID: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 220px), 1fr))",
  gap: 8,
};

const DESTINATION_CARD: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "minmax(0, 1fr) auto",
  gap: 10,
  alignItems: "start",
  minWidth: 0,
  padding: 9,
  border: "1px solid var(--trust-border)",
  borderRadius: 3,
  background: "var(--vscode-editorWidget-background, var(--trust-surface))",
};
