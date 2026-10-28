import React from "react";

import type { PlantEditorView } from "../plant/editorSession";

export type PlantOptionalDestination = "actuator" | "signals" | "realism" | "scenarios";

interface PlantOptionalSettingsProps {
  /** Rust's summary of each optional section. */
  summaries: PlantEditorView["summaries"];
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

export function PlantOptionalSettings({ summaries, active, onActiveChange }: PlantOptionalSettingsProps) {
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
                  {summaries[destination.id]}
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
