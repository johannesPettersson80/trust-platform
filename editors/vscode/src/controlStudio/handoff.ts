export interface PlantControlStudioHandoff {
  schemaVersion: 1;
  handoffId: string;
  projectId: string;
  runtimeId: string;
  plantId: string;
  savedConfigurationRevision: string;
  activeConfigurationDigest?: string;
  loopId?: string;
  source: "Simulation";
  requestedContext: {
    mode: "Monitor";
    selection: "plant_summary";
  };
}
