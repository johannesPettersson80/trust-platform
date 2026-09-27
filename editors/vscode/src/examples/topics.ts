const DOCUMENTATION_ROOT =
  "https://github.com/johannesPettersson80/trust-platform/blob/main/docs/public";

export const EXAMPLE_TOPICS = [
  { id: "tutorials", title: "PLC tutorials", description: "Structured Text, testing, simulation, deployment, safety, and CI/CD.", path: "examples/tutorials.md" },
  { id: "test-debug", title: "Test and debug", description: "Runtime values, process-image behaviour, and debugger workflows.", path: "examples/test-and-debug.md" },
  { id: "hmi", title: "HMI", description: "Live pages, bindings, trends, alarms, and safe writes.", path: "examples/hmi.md" },
  { id: "connectivity", title: "Connectivity", description: "ADS, Modbus, MQTT, OPC UA, EtherCAT, GPIO, and multi-driver projects.", path: "examples/connectivity.md" },
  { id: "runtime-cloud", title: "Runtime and cloud", description: "Profiles, deployment, remote access, federation, and observability.", path: "examples/runtime-cloud.md" },
  { id: "visual-editors", title: "Visual editors", description: "Ladder, SFC, statecharts, Blockly, and the hardware canvas.", path: "examples/visual-editors.md" },
  { id: "vendor", title: "Vendor migration", description: "Siemens, Mitsubishi, PLCopen XML, and vendor library adaptation.", path: "examples/vendor-profiles.md" },
  { id: "motion", title: "Libraries and motion", description: "PLCopen Motion, OSCAT, reusable libraries, and performance baselines.", path: "examples/libraries-and-motion.md" },
  { id: "capstones", title: "Capstones", description: "Complete plants, production lines, HMI, hardware, and multi-file systems.", path: "examples/capstones.md" },
  { id: "telemetry", title: "Telemetry", description: "OpenOT attribute authoring, historian data, and operations evidence.", path: "develop/openot-authoring.md" },
  { id: "runbooks", title: "Runbooks", description: "Site-specific operator and technician handoff templates.", path: "examples/runbooks.md" },
] as const;

export type ExampleTopicAction =
  | { type: "browseExamples"; topicId: string }
  | { type: "openExamplesDocumentation"; topicId: string };

/** Topic cards stay inside the runnable gallery; only an explicit Read guide leaves VS Code. */
export function exampleTopicAction(actionId: string): ExampleTopicAction {
  if (actionId.startsWith("read-guide:")) {
    return { type: "openExamplesDocumentation", topicId: actionId.slice("read-guide:".length) };
  }
  return { type: "browseExamples", topicId: actionId };
}

export function exampleDocumentationUrl(topicId: string | undefined): string {
  const path = EXAMPLE_TOPICS.find((topic) => topic.id === topicId)?.path ??
    "examples/index.md";
  return `${DOCUMENTATION_ROOT}/${path}`;
}
